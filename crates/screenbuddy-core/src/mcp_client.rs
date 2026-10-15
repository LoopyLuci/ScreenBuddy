//! MCP client: consume an external Model Context Protocol server over stdio.
//!
//! ScreenBuddy is an MCP *server* today (`mcp/server.py`); this module is the
//! other direction — the creature reaching out to a server such as
//! AgenticBotPlatform's (`bot/mcp_server.py`). It speaks the stdio transport
//! the MCP spec defines: newline-delimited JSON-RPC 2.0, one message per
//! line, no embedded newlines.
//!
//! The flow is the same handshake every MCP client performs:
//! `initialize` -> `notifications/initialized` -> `tools/list` ->
//! `tools/call`. Requests carry an incrementing id and the reader loops
//! until the matching response arrives, so interleaved notifications never
//! desynchronise the stream.
//!
//! Synchronous by design: one client, one conversation, request in order.
//! Callers on the async side wrap it (`spawn_blocking`) — the protocol is
//! strictly sequential, so there is nothing to gain from a concurrent one.

use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

/// A tool an MCP server advertises, mirroring the schema the model sees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// Who answered the handshake.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// One live connection to an MCP server subprocess.
pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    /// Stderr of the child, kept so a crashed server can be diagnosed.
    stderr: Option<std::thread::JoinHandle<String>>,
}

impl McpClient {
    /// Spawn `command args...` with `env` applied and speak MCP to it.
    ///
    /// The child's stderr is drained on a thread (a full stderr pipe would
    /// deadlock a chatty server) and kept for error reporting.
    pub fn spawn(command: &str, args: &[&str], env: &[(&str, &str)]) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| Error::Network(format!("could not start MCP server {command:?}: {e}")))?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| Error::Network("MCP server has no stdin".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Network("MCP server has no stdout".into()))?;
        let stderr_pipe = child.stderr.take();
        let stderr = stderr_pipe.map(|pipe| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut buf = String::new();
                let _ = pipe.take(4096).read_to_string(&mut buf);
                buf
            })
        });
        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
            stderr,
        })
    }

    /// The handshake: `initialize`, then the `initialized` notification.
    pub fn initialize(&mut self) -> Result<ServerInfo> {
        let result = self.request(
            "initialize",
            serde_json::json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "screenbuddy", "version": env!("CARGO_PKG_VERSION")},
            }),
        )?;
        self.notify("notifications/initialized")?;
        let info = &result["serverInfo"];
        Ok(ServerInfo {
            name: info["name"].as_str().unwrap_or("").to_string(),
            version: info["version"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Every tool the server offers.
    pub fn list_tools(&mut self) -> Result<Vec<McpTool>> {
        let result = self.request("tools/list", serde_json::json!({}))?;
        let tools = result["tools"].as_array().cloned().unwrap_or_default();
        serde_json::from_value(serde_json::Value::Array(tools))
            .map_err(|e| Error::Network(format!("bad tools/list payload: {e}")))
    }

    /// Call one tool and return its text content.
    ///
    /// A tool-level failure (`isError: true` in the result) is an error, not
    /// an empty answer — the same honest-failure rule the rest of the codebase
    /// follows.
    pub fn call_tool(&mut self, name: &str, arguments: serde_json::Value) -> Result<String> {
        let result = self.request(
            "tools/call",
            serde_json::json!({"name": name, "arguments": arguments}),
        )?;
        if result["isError"].as_bool() == Some(true) {
            let text = result["content"]
                .as_array()
                .map(|c| Self::join_content(c))
                .unwrap_or_default();
            return Err(Error::Network(format!("tool {name} failed: {text}")));
        }
        let content = result["content"].as_array().cloned().unwrap_or_default();
        Ok(Self::join_content(&content))
    }

    fn join_content(content: &[serde_json::Value]) -> String {
        content
            .iter()
            .filter_map(|c| c["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Send one request and read until its response arrives.
    fn request(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;
        let msg =
            serde_json::json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send(&msg)?;
        loop {
            let line = self.read_line()?;
            let parsed: serde_json::Value = serde_json::from_str(&line)
                .map_err(|e| Error::Network(format!("MCP server sent invalid JSON: {e}")))?;
            if parsed.get("id").and_then(|i| i.as_u64()) != Some(id) {
                continue; // a notification or another id: keep reading
            }
            if let Some(err) = parsed.get("error") {
                return Err(Error::Network(format!(
                    "MCP error on {method}: {}",
                    err["message"].as_str().unwrap_or("unknown")
                )));
            }
            return Ok(parsed["result"].clone());
        }
    }

    /// A notification: same frame shape, no id, no response expected.
    fn notify(&mut self, method: &str) -> Result<()> {
        let msg = serde_json::json!({"jsonrpc": "2.0", "method": method});
        self.send(&msg)
    }

    fn send(&mut self, msg: &serde_json::Value) -> Result<()> {
        let line = serde_json::to_string(msg)
            .map_err(|e| Error::Network(format!("could not encode MCP message: {e}")))?;
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|e| Error::Network(format!("could not write to MCP server: {e}")))?;
        Ok(())
    }

    fn read_line(&mut self) -> Result<String> {
        let mut line = String::new();
        let n = self
            .stdout
            .read_line(&mut line)
            .map_err(|e| Error::Network(format!("MCP server stopped responding: {e}")))?;
        if n == 0 {
            let detail = self
                .stderr
                .take()
                .and_then(|h| h.join().ok())
                .unwrap_or_default();
            return Err(Error::Network(format!(
                "MCP server closed the connection{}",
                if detail.is_empty() {
                    String::new()
                } else {
                    format!(": {detail}")
                }
            )));
        }
        Ok(line.trim().to_string())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Register every tool an MCP server offers into an agent runtime.
///
/// The creature's `AgentRuntime::register_tool` is the extension point: each
/// MCP tool becomes a runtime tool whose handler calls back through this one
/// long-lived connection. The client is shared behind a mutex — the protocol
/// is strictly sequential, so one connection serves every tool.
///
/// Returns how many tools were registered.
pub fn register_server_tools(
    client: McpClient,
    runtime: &crate::agent::AgentRuntime,
) -> Result<usize> {
    let mut client = client;
    client.initialize()?;
    let tools = client.list_tools()?;
    let shared = std::sync::Arc::new(std::sync::Mutex::new(client));
    let count = tools.len();
    for tool in tools {
        let name = tool.name.clone();
        let conn = shared.clone();
        let handler: crate::agent::ToolHandler = std::sync::Arc::new(move |args| {
            let conn = conn.clone();
            let name = name.clone();
            Box::pin(async move {
                // The client speaks synchronous stdio; keep it off the async
                // worker. A poisoned lock means the connection is gone, which
                // is an error to report, not to swallow.
                tokio::task::spawn_blocking(move || {
                    let mut conn = conn
                        .lock()
                        .map_err(|_| Error::Network("MCP connection is closed".into()))?;
                    conn.call_tool(&name, args)
                })
                .await
                .map_err(|e| Error::Network(format!("MCP call task failed: {e}")))?
            })
        });
        runtime.register_tool(
            &tool.name.clone(),
            crate::agent::Tool {
                name: tool.name,
                description: tool.description,
                input_schema: tool.input_schema,
                handler,
            },
        );
    }
    Ok(count)
}

/// Connect to AgenticBotPlatform's MCP server and expose its tools to the
/// creature's agent loop — Pattern E, the creature reaching into the fleet.
///
/// Entirely opt-in: with no `ABP_DIR` (and `DASHBOARD_TOKEN`) in the
/// environment this is a no-op returning `None`, so an install without ABP
/// behaves exactly as before. `ABP_PYTHON` picks the interpreter (default
/// `python`); use ABP's own venv so its dependencies resolve.
pub fn register_abp_tools(runtime: &crate::agent::AgentRuntime) -> Option<usize> {
    let abp_dir = std::env::var("ABP_DIR").ok()?;
    let script = format!("{abp_dir}/bot/mcp_server.py");
    if !std::path::Path::new(&script).exists() {
        eprintln!("[MCP] ABP_DIR is set but {script} does not exist; skipping");
        return None;
    }
    let token = std::env::var("DASHBOARD_TOKEN").unwrap_or_default();
    if token.is_empty() {
        eprintln!("[MCP] DASHBOARD_TOKEN is not set; cannot authenticate to ABP");
        return None;
    }
    let python = std::env::var("ABP_PYTHON").unwrap_or_else(|_| "python".into());
    let client = match McpClient::spawn(
        &python,
        &[&script],
        &[
            ("DASHBOARD_TOKEN", &token),
            ("ABP_HOME", &abp_dir),
            ("PYTHONPATH", &abp_dir),
        ],
    ) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[MCP] could not start ABP's MCP server: {e}");
            return None;
        }
    };
    match register_server_tools(client, runtime) {
        Ok(n) => {
            println!("[MCP] {n} AgenticBotPlatform tool(s) available to the agent");
            Some(n)
        }
        Err(e) => {
            eprintln!("[MCP] ABP handshake failed: {e}");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal MCP server: the handshake, two tools, and a call that echoes
    /// its arguments. Written to a temp file and spawned as the child.
    fn write_stub_server() -> std::path::PathBuf {
        let stub = r#"#!/usr/bin/env python3
import json, sys

def send(obj):
    sys.stdout.write(json.dumps(obj) + "\n")
    sys.stdout.flush()

for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    method = msg.get("method", "")
    if "id" in msg:
        if method == "initialize":
            send({"jsonrpc": "2.0", "id": msg["id"], "result": {
                "protocolVersion": "2025-03-26",
                "serverInfo": {"name": "stub", "version": "9.9.9"},
                "capabilities": {}}})
        elif method == "tools/list":
            send({"jsonrpc": "2.0", "id": msg["id"], "result": {"tools": [
                {"name": "echo", "description": "echoes text", "input_schema": {"type": "object"}},
                {"name": "boom", "description": "always fails", "input_schema": {"type": "object"}},
            ]}})
        elif method == "tools/call":
            name = msg["params"]["name"]
            if name == "boom":
                send({"jsonrpc": "2.0", "id": msg["id"], "result": {
                    "isError": True,
                    "content": [{"type": "text", "text": "boom went boom"}]}})
            else:
                text = msg["params"]["arguments"].get("text", "")
                send({"jsonrpc": "2.0", "id": msg["id"], "result": {
                    "content": [{"type": "text", "text": f"echo: {text}"}]}})
        else:
            send({"jsonrpc": "2.0", "id": msg["id"], "error": {
                "code": -32601, "message": f"no such method {method}"}})
    # notifications carry no id and expect no reply
"#;
        let path = std::env::temp_dir().join("screenbuddy_mcp_stub.py");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(stub.as_bytes()).unwrap();
        path
    }

    fn stub_client() -> McpClient {
        let path = write_stub_server();
        McpClient::spawn("python", &[path.to_str().unwrap()], &[]).unwrap()
    }

    #[test]
    fn the_handshake_reports_the_server() {
        let mut client = stub_client();
        let info = client.initialize().unwrap();
        assert_eq!(info.name, "stub");
        assert_eq!(info.version, "9.9.9");
    }

    #[test]
    fn list_tools_returns_what_the_server_advertises() {
        let mut client = stub_client();
        client.initialize().unwrap();
        let tools = client.list_tools().unwrap();
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[1].name, "boom");
    }

    #[test]
    fn call_tool_returns_the_text_content() {
        let mut client = stub_client();
        client.initialize().unwrap();
        let out = client
            .call_tool("echo", serde_json::json!({"text": "hi"}))
            .unwrap();
        assert_eq!(out, "echo: hi");
    }

    #[test]
    fn a_tool_error_is_an_error_not_an_empty_answer() {
        // The honest-failure rule: isError must surface, never a blank reply.
        let mut client = stub_client();
        client.initialize().unwrap();
        let err = client.call_tool("boom", serde_json::json!({})).unwrap_err();
        assert!(
            matches!(err, Error::Network(ref m) if m.contains("boom went boom")),
            "got: {err}"
        );
    }

    #[test]
    fn an_unknown_method_is_a_jsonrpc_error() {
        let mut client = stub_client();
        client.initialize().unwrap();
        let err = client
            .request("no/such/method", serde_json::json!({}))
            .unwrap_err();
        assert!(
            matches!(err, Error::Network(ref m) if m.contains("no such method")),
            "got: {err}"
        );
    }

    #[test]
    fn spawning_a_missing_command_is_a_clear_error() {
        // unwrap_err would demand Debug on the Ok type; a match keeps the
        // assertion about the error itself.
        let err = match McpClient::spawn("definitely-not-a-real-binary-xyz", &[], &[]) {
            Err(e) => e,
            Ok(_) => panic!("spawning a missing binary should have failed"),
        };
        assert!(
            matches!(err, Error::Network(ref m) if m.contains("could not start")),
            "got: {err}"
        );
    }

    #[test]
    fn registering_server_tools_makes_them_callable_through_the_runtime() {
        // The wiring test: the stub's tools land in a real AgentRuntime and a
        // handler call goes back through the connection and returns the text.
        let runtime = crate::agent::AgentRuntime::new(std::sync::Arc::new(
            crate::ai::AiEngine::new(crate::ai::AiConfig::default()),
        ));
        let path = write_stub_server();
        let client = McpClient::spawn("python", &[path.to_str().unwrap()], &[]).unwrap();
        let registered = register_server_tools(client, &runtime).unwrap();
        assert_eq!(registered, 2, "both stub tools should be registered");
        assert_eq!(runtime.tool_count(), 2);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let out = rt.block_on(async {
            let echo = runtime.get_tool("echo").expect("echo is registered");
            (echo.handler)(serde_json::json!({"text": "wired"}))
                .await
                .unwrap()
        });
        assert_eq!(out, "echo: wired");
    }
}
