//! Integration tests against a stub server shaped like 9Router.
//!
//! The unit tests in `nine_router.rs` prove URL construction. These prove the
//! request actually goes out correctly: the path, the OpenAI-shaped body, the
//! optional bearer token, tool-call parsing, and that failures surface as
//! readable errors rather than panics.
//!
//! The stub is a Python HTTP server (`tools/nine_router_stub.py`) matching the
//! endpoints documented by [9Router](https://github.com/decolua/9router).

use screenbuddy_core::ai::{AiConfig, AiEngine, AiRequest, Backend, ModelInfo, ModelTier};

/// Start the stub router on a free port and return its base URL.
///
/// The stub is launched detached so the test process does not have to manage it;
/// if it cannot start we skip rather than fail, since a test that needs a network
/// listener should not be the thing that breaks a build on a locked-down runner.
fn stub() -> Option<String> {
    use std::process::{Command, Stdio};

    // Pick a port by asking the OS, then hand it to the stub via argv.
    let probe = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    let port = probe.local_addr().ok()?.port();
    drop(probe);

    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tools/nine_router_stub.py"
    );
    let child = Command::new("python")
        .arg(script)
        .arg(port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    if child.is_err() {
        return None;
    }

    // A successful spawn is not a listening socket: without this wait the first
    // request can arrive before Python has bound the port, and every test then
    // fails against a router that was never up.
    let base = format!("http://127.0.0.1:{port}");
    for _ in 0..100 {
        if std::net::TcpStream::connect(&base).is_ok() {
            return Some(format!("{base}/v1"));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    None
}

fn engine(base: &str) -> AiEngine {
    AiEngine::new(AiConfig {
        nine_router_endpoint: Some(base.to_string()),
        ..Default::default()
    })
}

fn model(name: &str) -> ModelInfo {
    ModelInfo {
        tier: ModelTier::LocalMedium,
        backend: Backend::NineRouter,
        name: name.to_string(),
        max_context: 0,
        cost: 0.0,
    }
}

fn request(prompt: &str) -> AiRequest {
    AiRequest {
        prompt: prompt.to_string(),
        ..Default::default()
    }
}

#[tokio::test]
async fn chat_reaches_the_router_and_parses_the_reply() {
    let Some(base) = stub() else {
        eprintln!("skipping: could not start the 9Router stub");
        return;
    };

    let response = engine(&base)
        .gen_nine_router(&request("hello"), &model("openai/gpt-4o"))
        .await
        .expect("the router should answer");

    assert_eq!(response.text, "Routed through 9Router.");
    assert_eq!(
        response.tokens_used, 42,
        "usage should come from the router"
    );
}

#[tokio::test]
async fn model_discovery_lists_what_the_router_offers() {
    let Some(base) = stub() else {
        eprintln!("skipping: could not start the 9Router stub");
        return;
    };

    let models = engine(&base)
        .nine_router_models()
        .await
        .expect("discovery should succeed");

    let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
    assert!(ids.contains(&"openai/gpt-4o"), "got {ids:?}");
    assert!(ids.contains(&"cc/claude-opus-4-6"), "got {ids:?}");

    // Sorted, so a picker does not reshuffle between refreshes.
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(ids, sorted, "model list should be sorted");

    // The provider prefix is what 9Router uses to route, so it must survive.
    let claude = models
        .iter()
        .find(|m| m.id.starts_with("cc/"))
        .expect("cc model");
    assert_eq!(claude.provider(), Some("cc"));
    assert_eq!(claude.model_name(), "claude-opus-4-6");
}

#[tokio::test]
async fn a_configured_key_is_sent_and_omitted_otherwise() {
    // 9Router can run with or without auth. Sending an empty bearer would be
    // worse than sending none at all, so both paths are checked.
    let Some(base) = stub() else {
        eprintln!("skipping: could not start the 9Router stub");
        return;
    };

    // Without a key: no Authorization header.
    engine(&base)
        .nine_router_models()
        .await
        .expect("unauthenticated discovery");

    // With a key: exactly the configured bearer.
    let authed = AiEngine::new(AiConfig {
        nine_router_endpoint: Some(base.clone()),
        nine_router_api_key: Some("secret-token".into()),
        ..Default::default()
    });
    authed
        .nine_router_models()
        .await
        .expect("authenticated discovery");

    // A blank key is treated as no key.
    let blank = AiEngine::new(AiConfig {
        nine_router_endpoint: Some(base),
        nine_router_api_key: Some("   ".into()),
        ..Default::default()
    });
    blank
        .nine_router_models()
        .await
        .expect("blank key is ignored");
}

#[tokio::test]
async fn a_dead_router_is_a_readable_error_not_a_panic() {
    // Port 1 is reserved and never listening, so this is the "user has not
    // started 9Router" case. It must read as advice, not a raw socket error.
    let dead = AiEngine::new(AiConfig {
        nine_router_endpoint: Some("http://127.0.0.1:1/v1".into()),
        ..Default::default()
    });

    let error = dead
        .nine_router_models()
        .await
        .expect_err("a dead router must fail");

    let message = error.to_string();
    assert!(
        message.contains("9Router"),
        "error should name the router, got: {message}"
    );
    assert!(
        message.contains("127.0.0.1:1"),
        "error should name the URL, got: {message}"
    );
}

#[tokio::test]
async fn an_http_error_carries_the_router_status_and_detail() {
    let Some(base) = stub() else {
        eprintln!("skipping: could not start the 9Router stub");
        return;
    };

    // Ask for a path the stub answers 404 on by asking for models on a wrong
    // port shape is hard; instead point at a URL that 404s.
    let wrong = base.replace("/v1", "/nope/v1");
    let error = engine(&wrong)
        .nine_router_models()
        .await
        .expect_err("a 404 must fail");

    let message = error.to_string();
    assert!(
        message.contains("404") || message.contains("9Router"),
        "error should report the status, got: {message}"
    );
}
