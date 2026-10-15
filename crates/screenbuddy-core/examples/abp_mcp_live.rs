//! Live check: the Rust MCP client against AgenticBotPlatform's real stdio
//! MCP server.
//!
//! Run with the ABP checkout and its venv in place:
//!
//! ```text
//! ABP_DIR=C:/Projects/AgenticBotPlatform \
//! ABP_PYTHON=C:/Projects/AgenticBotPlatform/.venv/Scripts/python.exe \
//! DASHBOARD_TOKEN=<token> \
//! cargo run -p screenbuddy-core --example abp_mcp_live
//! ```
//!
//! Proves the whole Pattern E chain in Rust: spawn -> initialize ->
//! tools/list -> tools/call, against the same server the Python side already
//! verified. Skipped (not failed) when ABP is not configured.

use screenbuddy_core::mcp_client::McpClient;
use std::env;

fn main() {
    let abp_dir = match env::var("ABP_DIR") {
        Ok(d) => d,
        Err(_) => {
            println!("SKIP  set ABP_DIR to the AgenticBotPlatform checkout to run this");
            return;
        }
    };
    let python = env::var("ABP_PYTHON").unwrap_or_else(|_| "python".into());
    let script = format!("{abp_dir}/bot/mcp_server.py");
    if !std::path::Path::new(&script).exists() {
        println!("SKIP  {script} not found");
        return;
    }
    let token = env::var("DASHBOARD_TOKEN").unwrap_or_default();
    if token.is_empty() {
        println!("SKIP  set DASHBOARD_TOKEN to the ABP dashboard token");
        return;
    }

    let mut client = match McpClient::spawn(
        &python,
        &[&script],
        &[
            ("DASHBOARD_TOKEN", &token),
            ("ABP_HOME", &abp_dir),
            // mcp_server.py does `from bot import ...`, which needs the repo
            // root on sys.path — running it as a script path only puts bot/
            // itself there.
            ("PYTHONPATH", &abp_dir),
        ],
    ) {
        Ok(c) => c,
        Err(e) => {
            println!("FAIL  could not spawn ABP's MCP server: {e}");
            std::process::exit(1);
        }
    };

    let info = match client.initialize() {
        Ok(i) => i,
        Err(e) => {
            println!("FAIL  handshake: {e}");
            std::process::exit(1);
        }
    };
    println!("initialize: OK  server={} v{}", info.name, info.version);

    let tools = match client.list_tools() {
        Ok(t) => t,
        Err(e) => {
            println!("FAIL  tools/list: {e}");
            std::process::exit(1);
        }
    };
    println!("tools/list: {} tools", tools.len());
    for t in tools.iter().take(10) {
        println!("  - {t}", t = t.name);
    }

    // One real call: ABP's own status tool, no arguments.
    match client.call_tool("get_status", serde_json::json!({})) {
        Ok(text) => println!("tools/call get_status: OK\n{text}"),
        Err(e) => {
            println!("FAIL  tools/call: {e}");
            std::process::exit(1);
        }
    }
    println!("LIVE_OK");
}
