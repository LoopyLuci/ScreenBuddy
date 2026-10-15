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

/// How to (re)spawn the connection: kept on the client so a dead server
/// can be replaced without the caller knowing how it was started.
#[derive(Debug, Clone)]
pub struct McpConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

/// One live connection to an MCP server subprocess.
pub struct McpClient {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
    /// Stderr of the child, kept so a crashed server can be diagnosed.
    stderr: Option<std::thread::JoinHandle<String>>,
    /// How to start this server again — the basis of `reconnect`.
    config: McpConfig,
}

impl McpClient {
    /// Spawn `command args...` with `env` applied and speak MCP to it.
    ///
    /// The child's stderr is drained on a thread (a full stderr pipe would
    /// deadlock a chatty server) and kept for error reporting.
    pub fn spawn(command: &str, args: &[&str], env: &[(&str, &str)]) -> Result<Self> {
        let config = McpConfig {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };
        Self::spawn_with_config(config)
    }

    /// Start a fresh child from a stored spawn config.
    fn spawn_with_config(config: McpConfig) -> Result<Self> {
        let mut cmd = Command::new(&config.command);
        for a in &config.args {
            cmd.arg(a);
        }
        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in &config.env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(|e| {
            Error::Network(format!(
                "could not start MCP server {command:?}: {e}",
                command = config.command
            ))
        })?;
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
            config,
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

    /// Call a tool; a dead connection is replaced and the call retried once.
    ///
    /// An ABP that restarts mid-session would otherwise leave the creature
    /// with dead tools until the app restarts. A tool-level error is an
    /// answer, so only transport failures reconnect — retrying an `isError`
    /// would just spend the failure twice.
    pub fn call_tool_reliable(
        &mut self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<String> {
        match self.call_tool(name, arguments.clone()) {
            Err(ref e) if is_transport_error(e) => {
                eprintln!("[MCP] connection lost on {name}; reconnecting");
                self.reconnect()?;
                self.call_tool(name, arguments)
            }
            other => other,
        }
    }

    /// Cheap liveness probe: a `tools/list` round trip.
    ///
    /// On a transport failure the connection is replaced and probed once
    /// more — a restarted ABP is expected to come back within seconds. If it
    /// doesn't, the error stands.
    pub fn heartbeat(&mut self) -> Result<()> {
        match self.list_tools() {
            Ok(_) => Ok(()),
            Err(e) if is_transport_error(&e) => {
                eprintln!("[MCP] heartbeat failed; reconnecting");
                self.reconnect()?;
                self.list_tools().map(|_| ())
            }
            Err(e) => Err(e),
        }
    }

    /// Kill the child and start a fresh one from the stored spawn config,
    /// handshaking anew. A dead server becomes a new session, ids included.
    pub fn reconnect(&mut self) -> Result<()> {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(h) = self.stderr.take() {
            if let Ok(detail) = h.join() {
                if !detail.is_empty() {
                    eprintln!("[MCP] server said before dying: {detail}");
                }
            }
        }
        let fresh = Self::spawn_with_config(self.config.clone())?;
        *self = fresh;
        self.initialize().map(|_| ())
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
    register_tools_into(client, tools, runtime)
}

/// Like `register_server_tools`, but only the named tools (unknown names are
/// ignored — a server that doesn't advertise them simply registers fewer).
///
/// This is how a small model's prompt stays sane: an 0.5B model cannot
/// usefully choose among 72 tool schemas, so the caller passes the curated
/// allowlist it can actually hold.
pub fn register_server_tools_filtered(
    client: McpClient,
    runtime: &crate::agent::AgentRuntime,
    allow: &[&str],
) -> Result<usize> {
    let mut client = client;
    client.initialize()?;
    let tools: Vec<McpTool> = client
        .list_tools()?
        .into_iter()
        .filter(|t| allow.contains(&t.name.as_str()))
        .collect();
    let count = tools.len();
    register_tools_into(client, tools, runtime)?;
    Ok(count)
}

/// The shared registration loop behind both entry points.
fn register_tools_into(
    client: McpClient,
    tools: Vec<McpTool>,
    runtime: &crate::agent::AgentRuntime,
) -> Result<usize> {
    let count = tools.len();
    let shared = std::sync::Arc::new(std::sync::Mutex::new(client));
    for tool in tools {
        let name = tool.name.clone();
        let conn = shared.clone();
        let handler: crate::agent::ToolHandler = std::sync::Arc::new(move |args| {
            let conn = conn.clone();
            let name = name.clone();
            Box::pin(async move {
                // The client speaks synchronous stdio; keep it off the async
                // worker. A poisoned lock means a panic happened while the
                // connection was held — recover the client (a dead connection
                // reconnects on the reliable call below) instead of failing.
                tokio::task::spawn_blocking(move || {
                    let mut conn = conn.lock().unwrap_or_else(|e| e.into_inner());
                    conn.call_tool_reliable(&name, args)
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

/// Which errors mean the connection itself is broken, versus the server
/// answering. A tool error (`isError`) or a JSON-RPC error is an answer —
/// only a dead pipe, a reset connection or unparseable garbage is transport.
pub fn is_transport_error(err: &Error) -> bool {
    match err {
        Error::Network(m) => {
            m.starts_with("could not write to MCP server")
                || m.starts_with("MCP server stopped responding")
                || m.starts_with("MCP server closed the connection")
                || m.starts_with("MCP server sent invalid JSON")
        }
        _ => false,
    }
}

/// The read-only fleet tools worth holding in a small model's prompt.
///
/// ABP advertises ~72 tools; a 0.5B model cannot usefully choose among all
/// of them, so the tiny tier registers this curated set instead. Every name
/// is read-only — no state changes, no estop, no config writes — and every
/// one is a real ABP MCP tool (verified against `bot/mcp_server.py`).
pub const TINY_ABP_TOOLS: &[&str] = &[
    "get_status",
    "list_jobs",
    "list_snapshots",
    "hot_reload_status",
    "get_setup_status",
    "ask_instance",
    "list_external_mcp_servers",
    "get_model_info",
    "get_model_usage",
    "memory_search",
    "read_project_context",
];

/// Which tools to register for an ABP connection.
///
/// An explicit allowlist (env `ABP_TOOLS`, comma-separated) wins; the tiny
/// tier gets the curated read-only set; `None` means register everything.
pub fn select_abp_tools(explicit: Option<&str>, tiny: bool) -> Option<Vec<String>> {
    if let Some(list) = explicit {
        let names: Vec<String> = list
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        if !names.is_empty() {
            return Some(names);
        }
    }
    if tiny {
        return Some(TINY_ABP_TOOLS.iter().map(|s| s.to_string()).collect());
    }
    None
}

/// Connect to AgenticBotPlatform's MCP server and expose its tools to the
/// creature's agent loop — Pattern E, the creature reaching into the fleet.
///
/// Entirely opt-in: with no `ABP_DIR` (and `DASHBOARD_TOKEN`) in the
/// environment this is a no-op returning `None`, so an install without ABP
/// behaves exactly as before. `ABP_PYTHON` picks the interpreter (default
/// `python`); use ABP's own venv so its dependencies resolve.
///
/// Tool selection: `ABP_TOOLS` (comma-separated allowlist) registers exactly
/// those; `ABP_TINY_TOOLS=1` registers the curated read-only set for a small
/// model; with neither, all tools are registered.
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
    let explicit = std::env::var("ABP_TOOLS").ok();
    let tiny = std::env::var("ABP_TINY_TOOLS")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let allow = select_abp_tools(explicit.as_deref(), tiny);
    let names: Option<Vec<String>> = allow;
    let registered = match names.as_deref() {
        Some(list) => {
            let refs: Vec<&str> = list.iter().map(|s| s.as_str()).collect();
            register_server_tools_filtered(client, runtime, &refs)
        }
        None => register_server_tools(client, runtime),
    };
    match registered {
        Ok(n) => {
            let mode = if names.is_some() { " (filtered)" } else { "" };
            println!("[MCP] {n} AgenticBotPlatform tool(s) available to the agent{mode}");
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
                {"name": "die", "description": "exits the server", "input_schema": {"type": "object"}},
            ]}})
        elif method == "tools/call":
            name = msg["params"]["name"]
            if name == "die":
                sys.exit(0)
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
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[1].name, "boom");
        assert_eq!(tools[2].name, "die");
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
        assert_eq!(registered, 3, "all stub tools should be registered");
        assert_eq!(runtime.tool_count(), 3);

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

    #[test]
    fn is_transport_error_tells_a_dead_connection_from_an_answer() {
        let dead = Error::Network("MCP server closed the connection".into());
        assert!(is_transport_error(&dead));
        let tool_err = Error::Network("tool boom failed: boom went boom".into());
        assert!(
            !is_transport_error(&tool_err),
            "an isError answer is not transport"
        );
        let rpc_err = Error::Network("MCP error on no/such/method: no such method".into());
        assert!(
            !is_transport_error(&rpc_err),
            "a JSON-RPC error is an answer, not transport"
        );
    }

    #[test]
    fn a_dead_connection_recovers_by_reconnecting() {
        // The stub exits on "die": the connection is now dead. The reliable
        // call must replace it and succeed on the fresh server.
        let mut client = stub_client();
        client.initialize().unwrap();
        let err = client.call_tool("die", serde_json::json!({})).unwrap_err();
        assert!(
            is_transport_error(&err),
            "a dead server must surface as transport, got: {err}"
        );
        let out = client
            .call_tool_reliable("echo", serde_json::json!({"text": "back"}))
            .unwrap();
        assert_eq!(out, "echo: back");
    }

    #[test]
    fn heartbeat_reconnects_after_the_server_dies() {
        let mut client = stub_client();
        client.initialize().unwrap();
        client.call_tool("die", serde_json::json!({})).unwrap_err();
        // Probe, find the pipe dead, reconnect, probe again.
        client.heartbeat().unwrap();
        // And the connection is genuinely usable afterwards.
        let out = client
            .call_tool("echo", serde_json::json!({"text": "alive"}))
            .unwrap();
        assert_eq!(out, "echo: alive");
    }

    #[test]
    fn filtered_registration_registers_only_the_allowlist() {
        let runtime = crate::agent::AgentRuntime::new(std::sync::Arc::new(
            crate::ai::AiEngine::new(crate::ai::AiConfig::default()),
        ));
        let path = write_stub_server();
        let client = McpClient::spawn("python", &[path.to_str().unwrap()], &[]).unwrap();
        let registered = register_server_tools_filtered(client, &runtime, &["echo"]).unwrap();
        assert_eq!(registered, 1, "only the allowlisted tool registers");
        assert_eq!(runtime.tool_count(), 1);
        assert!(runtime.get_tool("echo").is_some());
        assert!(
            runtime.get_tool("boom").is_none(),
            "boom is not in the allowlist"
        );
    }

    #[test]
    fn filtered_registration_ignores_names_the_server_does_not_advertise() {
        let runtime = crate::agent::AgentRuntime::new(std::sync::Arc::new(
            crate::ai::AiEngine::new(crate::ai::AiConfig::default()),
        ));
        let path = write_stub_server();
        let client = McpClient::spawn("python", &[path.to_str().unwrap()], &[]).unwrap();
        let registered =
            register_server_tools_filtered(client, &runtime, &["echo", "not-a-real-tool"]).unwrap();
        assert_eq!(registered, 1, "unknown names are ignored, not fatal");
    }

    #[test]
    fn select_abp_tools_prefers_the_explicit_list() {
        let tools = select_abp_tools(Some("get_status, list_jobs ,"), false).unwrap();
        assert_eq!(tools, vec!["get_status", "list_jobs"]);
    }

    #[test]
    fn select_abp_tools_tiny_returns_the_curated_set() {
        let tools = select_abp_tools(None, true).unwrap();
        assert!(tools.contains(&"get_status".to_string()));
        assert!(tools.contains(&"ask_instance".to_string()));
        assert!(tools.len() >= 8, "a tiny set should still be useful");
    }

    #[test]
    fn select_abp_tools_with_neither_registers_everything() {
        assert!(select_abp_tools(None, false).is_none());
        assert!(select_abp_tools(Some(""), false).is_none());
    }

    #[test]
    fn the_tiny_tool_set_is_read_only() {
        // A small model must never hold a state-changing tool — these are
        // ABP's mutation verbs, and the policy must not rot with new tools.
        let dangerous = [
            "set_",
            "delete_",
            "remove_",
            "restore_",
            "engage_",
            "disengage_",
            "create_",
            "update_",
            "enable_",
            "disable_",
            "start_",
            "stop_",
            "restart_",
            "trigger_",
            "dispatch_",
            "run_swarm",
            "write_",
        ];
        for tool in TINY_ABP_TOOLS {
            for prefix in dangerous {
                assert!(
                    !tool.starts_with(prefix),
                    "tiny set must be read-only; {tool} starts with {prefix}"
                );
            }
        }
    }
}
