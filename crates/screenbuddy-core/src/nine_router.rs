//! 9Router integration.
//!
//! [9Router](https://github.com/decolua/9router) is an OpenAI-compatible router
//! that fronts many providers with automatic fallback. ScreenBuddy talks to it
//! over the same wire format it already uses for OpenAI, so the interesting
//! parts are the things that differ from a plain OpenAI call:
//!
//! - the default base URL (`http://localhost:20128/v1`),
//! - model discovery through `/v1/models`, which is how the user finds out what
//!   their router actually offers,
//! - auth being optional, since 9Router may run with or without it.
//!
//! Everything lives here rather than inline in `ai.rs` so the routing,
//! discovery and error handling for one upstream stay in one place.

use serde::Deserialize;

/// Where 9Router listens by default. Matches its documented port.
pub const DEFAULT_BASE_URL: &str = "http://localhost:20128/v1";

/// A model as 9Router reports it.
#[derive(Debug, Clone, Deserialize)]
pub struct RouterModel {
    pub id: String,
    #[serde(default)]
    pub owned_by: Option<String>,
}

impl RouterModel {
    /// 9Router names models `provider/model`, e.g. `cc/claude-opus-4-6`. The
    /// prefix is useful, but the model name on its own is what reads well in a
    /// picker, so both are offered.
    pub fn provider(&self) -> Option<&str> {
        self.id.split_once('/').map(|(p, _)| p)
    }

    pub fn model_name(&self) -> &str {
        self.id.split_once('/').map(|(_, m)| m).unwrap_or(&self.id)
    }
}

/// Resolve the base URL to use, falling back to the documented default.
pub(crate) fn base_url(configured: Option<&str>) -> String {
    configured
        .map(|s| s.trim().trim_end_matches('/').to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_BASE_URL.to_string())
}

/// Full URL for one of 9Router's OpenAI-compatible endpoints.
pub(crate) fn endpoint(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

pub(crate) fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_the_documented_port() {
        assert_eq!(base_url(None), "http://localhost:20128/v1");
        assert_eq!(base_url(Some("")), "http://localhost:20128/v1");
        assert_eq!(base_url(Some("   ")), "http://localhost:20128/v1");
    }

    #[test]
    fn a_configured_url_wins_and_loses_its_slash() {
        assert_eq!(
            base_url(Some("http://10.0.0.5:9000/v1")),
            "http://10.0.0.5:9000/v1"
        );
        assert_eq!(
            base_url(Some("http://10.0.0.5:9000/v1/")),
            "http://10.0.0.5:9000/v1"
        );
        // Whitespace from a settings box should not corrupt the URL.
        assert_eq!(base_url(Some("  http://x/v1  ")), "http://x/v1");
    }

    #[test]
    fn endpoints_join_without_doubling_slashes() {
        assert_eq!(
            endpoint("http://h:1/v1", "chat/completions"),
            "http://h:1/v1/chat/completions"
        );
        assert_eq!(
            endpoint("http://h:1/v1/", "/chat/completions"),
            "http://h:1/v1/chat/completions"
        );
        assert_eq!(endpoint("http://h:1/v1", "models"), "http://h:1/v1/models");
    }

    #[test]
    fn provider_and_model_split_on_the_prefix() {
        let m = RouterModel {
            id: "cc/claude-opus-4-6".into(),
            owned_by: None,
        };
        assert_eq!(m.provider(), Some("cc"));
        assert_eq!(m.model_name(), "claude-opus-4-6");
    }

    #[test]
    fn an_unprefixed_model_still_works() {
        let m = RouterModel {
            id: "gpt-4o".into(),
            owned_by: None,
        };
        assert_eq!(m.provider(), None);
        assert_eq!(m.model_name(), "gpt-4o", "falls back to the whole id");
    }

    #[test]
    fn model_parses_from_an_openai_shaped_entry() {
        let json = serde_json::json!({"id": "openai/gpt-4o", "owned_by": "openai"});
        let m: RouterModel = serde_json::from_value(json).expect("parse");
        assert_eq!(m.id, "openai/gpt-4o");
        assert_eq!(m.owned_by.as_deref(), Some("openai"));
    }

    #[test]
    fn a_model_without_optional_fields_still_parses() {
        let json = serde_json::json!({"id": "free/llama"});
        let m: RouterModel = serde_json::from_value(json).expect("parse");
        assert!(m.owned_by.is_none());
    }

    #[test]
    fn long_errors_are_truncated_with_an_ellipsis() {
        let long = "x".repeat(500);
        let out = truncate(&long, 400);
        assert!(out.chars().count() <= 401);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn short_errors_are_left_alone() {
        assert_eq!(truncate("boom", 400), "boom");
    }
}
