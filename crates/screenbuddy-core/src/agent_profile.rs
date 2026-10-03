//! Saved agent profiles: the thing a user actually edits in the GUI.
//!
//! An *agent* ties together everything that makes one companion feel like
//! itself: which model answers, which creature is on screen, and how it behaves
//! and speaks. Before this, all of that was hardcoded -- the seven creatures
//! were a literal tuple array in `main.rs`, and `AgentConfig` had only
//! `max_iterations`, `timeout_secs` and a single system prompt.
//!
//! Everything here persists to `%APPDATA%\ScreenBuddy\agents.json`, mirroring
//! how [`crate::session::SessionStore`] handles chat. The design rules:
//!
//! - A profile is user data, so it round-trips losslessly and survives a restart.
//! - Built-in presets are code, not data, and are always available even if the
//!   user deletes their copies.
//! - Resolution is total: asking for an agent always yields something usable,
//!   falling back to a default rather than erroring on a dangling reference.

use crate::agent::AgentConfig;
use crate::creature_catalogue::creature_catalogue;
use crate::error::Result;
use crate::screen_physics::CreaturePersonality;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Which provider answers for an agent.
///
/// Kept separate from the model's own backend because a user picks "use
/// OpenRouter", not "use `Backend::OpenAi` with tier 2".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AgentProvider {
    /// Let the engine choose from the configured model list. The default, so an
    /// omitted provider means "choose for me".
    #[default]
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "ollama")]
    Ollama,
    #[serde(rename = "llama_cpp")]
    LlamaCpp,
    // Spelled out rather than derived: snake_case would make this "open_ai",
    // which disagreed with as_str() and could not be read back.
    #[serde(rename = "openai")]
    OpenAi,
    #[serde(rename = "anthropic")]
    Anthropic,
    /// [9Router](https://github.com/decolua/9router): an OpenAI-compatible
    /// router that fronts many providers with automatic fallback.
    #[serde(rename = "nine_router")]
    NineRouter,
    #[serde(rename = "custom")]
    Custom,
}

impl AgentProvider {
    pub const ALL: [AgentProvider; 7] = [
        AgentProvider::Auto,
        AgentProvider::Ollama,
        AgentProvider::LlamaCpp,
        AgentProvider::OpenAi,
        AgentProvider::Anthropic,
        AgentProvider::NineRouter,
        AgentProvider::Custom,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AgentProvider::Auto => "Automatic",
            AgentProvider::Ollama => "Ollama (local)",
            AgentProvider::LlamaCpp => "llama.cpp (local)",
            AgentProvider::OpenAi => "OpenAI",
            AgentProvider::Anthropic => "Anthropic",
            AgentProvider::NineRouter => "9Router",
            AgentProvider::Custom => "Custom endpoint",
        }
    }

    /// Parse from the wire string, falling back to [`AgentProvider::Auto`].
    ///
    /// Deliberately never fails: a profile written by a newer build must still
    /// load, and an unknown provider simply means "let the engine choose".
    pub fn parse(value: &str) -> Self {
        match value {
            "ollama" => AgentProvider::Ollama,
            "llama_cpp" => AgentProvider::LlamaCpp,
            "openai" => AgentProvider::OpenAi,
            "anthropic" => AgentProvider::Anthropic,
            "nine_router" => AgentProvider::NineRouter,
            "custom" => AgentProvider::Custom,
            _ => AgentProvider::Auto,
        }
    }
}

/// Parsing never fails: an unknown provider simply means "let the engine choose".
#[derive(Debug, Clone, Copy)]
pub struct NeverFailed;

impl std::fmt::Display for NeverFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("parsing never fails")
    }
}

impl std::str::FromStr for AgentProvider {
    type Err = NeverFailed;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(AgentProvider::parse(value))
    }
}

impl AgentProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentProvider::Auto => "auto",
            AgentProvider::Ollama => "ollama",
            AgentProvider::LlamaCpp => "llama_cpp",
            AgentProvider::OpenAi => "openai",
            AgentProvider::Anthropic => "anthropic",
            AgentProvider::NineRouter => "nine_router",
            AgentProvider::Custom => "custom",
        }
    }
}

/// How talkative and formal the agent is.
///
/// A persona is what makes a companion feel distinct; a bare system prompt does
/// not carry that. These become system-prompt text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Persona {
    /// Plain and helpful. The default, so an omitted persona is never surprising.
    #[default]
    Assistant,
    /// Warm, encouraging, a bit playful.
    Friendly,
    /// Blunt and economical. Good for power users.
    Terse,
    /// Detailed and explanatory.
    Explainer,
    /// Dry humour. Deliberately not the default.
    Wry,
    /// Formal and precise.
    Professional,
}

impl Persona {
    pub const ALL: [Persona; 6] = [
        Persona::Assistant,
        Persona::Friendly,
        Persona::Terse,
        Persona::Explainer,
        Persona::Wry,
        Persona::Professional,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Persona::Assistant => "Assistant",
            Persona::Friendly => "Friendly",
            Persona::Terse => "Terse",
            Persona::Explainer => "Explainer",
            Persona::Wry => "Wry",
            Persona::Professional => "Professional",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Persona::Assistant => "assistant",
            Persona::Friendly => "friendly",
            Persona::Terse => "terse",
            Persona::Explainer => "explainer",
            Persona::Wry => "wry",
            Persona::Professional => "professional",
        }
    }

    /// Parse from the wire string, falling back to [`Persona::Assistant`].
    pub fn parse(value: &str) -> Self {
        match value {
            "friendly" => Persona::Friendly,
            "terse" => Persona::Terse,
            "explainer" => Persona::Explainer,
            "wry" => Persona::Wry,
            "professional" => Persona::Professional,
            _ => Persona::Assistant,
        }
    }

    /// The instruction this persona contributes to the system prompt.
    ///
    /// Phrased as guidance to the model rather than a description of the
    /// persona, which reads as more reliable than "you are friendly".
    pub fn guidance(self) -> &'static str {
        match self {
            Persona::Assistant => "Be helpful and direct.",
            Persona::Friendly => {
                "Be warm and encouraging. Keep replies brief and upbeat, like a friend chatting on a desktop."
            }
            Persona::Terse => {
                "Answer in as few words as possible. No preamble, no restating the question, no summary of what you just said."
            }
            Persona::Explainer => {
                "Explain your reasoning as you go. Prefer detail over brevity unless asked otherwise."
            }
            Persona::Wry => {
                "Be dry and witty. A little humour is fine, but never at the expense of being correct."
            }
            Persona::Professional => {
                "Be formal and precise. Avoid contractions and filler."
            }
        }
    }
}

impl std::str::FromStr for Persona {
    type Err = NeverFailed;

    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        Ok(Persona::parse(value))
    }
}

/// One saved agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentProfile {
    /// Stable identifier. Used as the lookup key, so it is not the display name.
    pub id: String,
    /// What the user sees in the list.
    pub name: String,
    /// Which provider answers.
    pub provider: AgentProvider,
    /// The provider's model name, e.g. `cc/claude-opus-4-6`.
    ///
    /// Empty means "let the engine choose", which keeps a profile usable when a
    /// configured model disappears.
    pub model: String,
    /// Which creature represents this agent on screen.
    pub creature: String,
    /// Movement behaviour.
    pub personality: CreaturePersonality,
    /// Speaking style.
    pub persona: Persona,
    /// Extra instructions appended to the system prompt.
    ///
    /// This is the escape hatch: everything not covered by the enum fields goes
    /// here, so a user is never blocked by a missing setting.
    pub system_prompt: String,
    /// Whether this agent may use tools.
    pub tools_enabled: bool,
    /// Tool-call budget per request.
    pub max_iterations: usize,
    /// Per-request timeout.
    pub timeout_secs: u64,
    /// Sampling temperature.
    pub temperature: f32,
    /// Built-ins cannot be deleted or edited in place; they are always present.
    pub builtin: bool,
}

impl Default for AgentProfile {
    fn default() -> Self {
        // A blank profile, used when a payload omits fields. The id is a
        // placeholder; the store's sanitise step assigns a usable one.
        Self {
            id: String::new(),
            name: String::new(),
            provider: AgentProvider::Auto,
            model: String::new(),
            creature: String::new(),
            personality: CreaturePersonality::Neutral,
            persona: Persona::Assistant,
            system_prompt: String::new(),
            tools_enabled: true,
            max_iterations: 10,
            timeout_secs: 120,
            temperature: 0.7,
            builtin: false,
        }
    }
}

impl AgentProfile {
    /// A blank profile, ready to edit.
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            provider: AgentProvider::default(),
            model: String::new(),
            creature: String::new(),
            personality: CreaturePersonality::Neutral,
            persona: Persona::default(),
            system_prompt: String::new(),
            tools_enabled: true,
            max_iterations: 10,
            timeout_secs: 120,
            temperature: 0.7,
            builtin: false,
        }
    }

    /// The built-in presets, defined in code so they exist on a fresh install.
    pub fn presets() -> Vec<AgentProfile> {
        vec![
            AgentProfile {
                id: "builtin-assistant".into(),
                name: "Assistant".into(),
                provider: AgentProvider::Auto,
                model: String::new(),
                creature: "robo-cat-01".into(),
                personality: CreaturePersonality::Neutral,
                persona: Persona::Assistant,
                system_prompt: "You are ScreenBuddy, a helpful desktop AI companion.".into(),
                tools_enabled: true,
                max_iterations: 10,
                timeout_secs: 120,
                temperature: 0.7,
                builtin: true,
            },
            AgentProfile {
                id: "builtin-study-buddy".into(),
                name: "Study Buddy".into(),
                provider: AgentProvider::Auto,
                model: String::new(),
                creature: "companion-bird-01".into(),
                personality: CreaturePersonality::Curious,
                persona: Persona::Explainer,
                system_prompt:
                    "You are a patient tutor. Break problems down into steps and check understanding."
                        .into(),
                tools_enabled: true,
                max_iterations: 12,
                timeout_secs: 180,
                temperature: 0.6,
                builtin: true,
            },
            AgentProfile {
                id: "builtin-quick-ask".into(),
                name: "Quick Ask".into(),
                provider: AgentProvider::Auto,
                model: String::new(),
                creature: "sprite-bot-01".into(),
                personality: CreaturePersonality::Lazy,
                persona: Persona::Terse,
                system_prompt: "You answer fast and briefly. Never ask follow-up questions."
                    .into(),
                // Tools off: this preset is for one-shot questions, and tool
                // round-trips make it slow.
                tools_enabled: false,
                max_iterations: 3,
                timeout_secs: 30,
                temperature: 0.3,
                builtin: true,
            },
            AgentProfile {
                id: "builtin-coder".into(),
                name: "Coder".into(),
                provider: AgentProvider::Auto,
                model: String::new(),
                creature: "pixel-frog-01".into(),
                personality: CreaturePersonality::Neutral,
                persona: Persona::Terse,
                system_prompt:
                    "You are a programming assistant. Show code, not prose. Prefer small correct diffs."
                        .into(),
                tools_enabled: true,
                // A coder leans on tools, so it gets a deeper budget.
                max_iterations: 20,
                timeout_secs: 300,
                temperature: 0.2,
                builtin: true,
            },
            AgentProfile {
                id: "builtin-companion".into(),
                name: "Companion".into(),
                provider: AgentProvider::Auto,
                model: String::new(),
                creature: "spirit-fox-01".into(),
                personality: CreaturePersonality::Energetic,
                persona: Persona::Friendly,
                system_prompt:
                    "You are a cheerful desk companion. Keep replies short and conversational."
                        .into(),
                tools_enabled: false,
                max_iterations: 5,
                timeout_secs: 60,
                temperature: 0.9,
                builtin: true,
            },
        ]
    }

    /// Compose the system prompt from persona, role and user instructions.
    ///
    /// Ordering matters: the role line establishes identity, the persona sets
    /// style, then the user's own instructions get the last word.
    pub fn composed_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        let role = if self.system_prompt.trim().is_empty() {
            format!("You are {}, a desktop AI companion.", self.name)
        } else {
            self.system_prompt.trim().to_string()
        };
        parts.push(role);
        parts.push(self.persona.guidance().to_string());

        if !self.model.trim().is_empty() {
            parts.push(format!("You are running as {}.", self.model.trim()));
        }

        parts.join(" ")
    }

    /// The runtime config this profile implies.
    pub fn to_agent_config(&self) -> AgentConfig {
        AgentConfig {
            max_iterations: self.max_iterations.max(1),
            timeout_secs: self.timeout_secs.max(1),
            system_prompt: self.composed_system_prompt(),
            // Empty means "let the engine choose", which is why the runtime
            // filters blank values rather than passing them on.
            model: self.model.trim().to_string(),
            temperature: self.temperature,
            tools_enabled: self.tools_enabled,
        }
    }

    /// Clamp values that would otherwise break a request.
    ///
    /// Called on load so a hand-edited file cannot produce a profile that hangs
    /// or refuses to answer.
    pub fn sanitised(mut self) -> Self {
        if self.name.trim().is_empty() {
            self.name = "Untitled agent".into();
        }
        // Keep the id filename- and key-safe; it is used as a lookup key.
        self.id = sanitize_id(&self.id);
        if self.id.is_empty() {
            self.id = "agent".into();
        }
        self.max_iterations = self.max_iterations.clamp(1, 100);
        self.timeout_secs = self.timeout_secs.clamp(1, 3600);
        // 0 is a valid temperature but almost always a mistake here; 2.0 is
        // where output becomes noise.
        if !self.temperature.is_finite() {
            self.temperature = 0.7;
        }
        self.temperature = self.temperature.clamp(0.0, 2.0);
        self
    }
}

/// Where the active-agent marker lives, beside the profiles file.
///
/// `with_extension` would turn `agents.json` into `agents.active.json`, which is
/// confusing next to the real file; this names it plainly.
fn active_path(profiles_path: &std::path::Path) -> PathBuf {
    profiles_path.with_file_name("active_agent.json")
}

/// Reduce an id to characters that are safe in a filename and a JSON key.
fn sanitize_id(id: &str) -> String {
    id.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .take(64)
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Persisted agent profiles.
pub struct AgentStore {
    path: PathBuf,
    profiles: Mutex<Vec<AgentProfile>>,
}

impl AgentStore {
    /// Store at the default location, `%APPDATA%\ScreenBuddy\agents.json`.
    pub fn default_path() -> PathBuf {
        crate::paths::data_file("agents.json")
    }

    /// Load from disk, falling back to the built-in presets.
    ///
    /// Never fails: a corrupt or missing file yields the presets, because losing
    /// the ability to chat is worse than losing a saved agent.
    pub fn load_or_default(path: Option<PathBuf>) -> Self {
        let path = path.unwrap_or_else(Self::default_path);
        let profiles = std::fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<AgentProfile>>(&text).ok())
            .map(|saved| {
                // Presets are re-merged rather than trusted from disk, so an
                // upgraded build gets new built-ins even with an old file.
                merge_presets(saved)
            })
            .unwrap_or_else(AgentProfile::presets);

        Self {
            path,
            profiles: Mutex::new(profiles.into_iter().map(|p| p.sanitised()).collect()),
        }
    }

    /// In-memory store, for tests.
    pub fn in_memory() -> Self {
        Self {
            path: PathBuf::new(),
            profiles: Mutex::new(
                AgentProfile::presets()
                    .into_iter()
                    .map(|p| p.sanitised())
                    .collect(),
            ),
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// All profiles, presets first, then user profiles alphabetically.
    pub fn list(&self) -> Vec<AgentProfile> {
        let profiles = self.profiles.lock().expect("agent store poisoned");
        let mut out: Vec<AgentProfile> = profiles.clone();
        out.sort_by(|a, b| {
            // Built-ins first. Sorting on `builtin` ascending would put
            // user profiles ahead of the presets, since false < true.
            b.builtin
                .cmp(&a.builtin)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        out
    }

    /// Look up one profile. Returns `None` for an unknown id rather than
    /// erroring, so callers can fall back deliberately.
    pub fn get(&self, id: &str) -> Option<AgentProfile> {
        self.profiles
            .lock()
            .ok()?
            .iter()
            .find(|p| p.id == id)
            .cloned()
    }

    /// Record which agent is in effect, so it survives a restart.
    ///
    /// Without this the active agent is forgotten on exit and the app silently
    /// reverts to defaults, losing the user's choice.
    pub fn set_active(&self, id: &str) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::json!({ "active": id });
        std::fs::write(active_path(&self.path), serde_json::to_string(&json)?)?;
        Ok(())
    }

    /// The agent to restore on startup, if any.
    pub fn active_id(&self) -> Option<String> {
        if self.path.as_os_str().is_empty() {
            return None;
        }
        let raw = std::fs::read_to_string(active_path(&self.path)).ok()?;
        let id = serde_json::from_str::<serde_json::Value>(&raw)
            .ok()?
            .get("active")?
            .as_str()?
            .to_string();
        // Only restore an agent that still exists.
        self.get(&id).map(|_| id)
    }

    /// Insert or update, and persist.
    ///
    /// Built-ins are routed to `update_builtin`, so a UI that saves an edited
    /// preset produces a user copy instead of mutating the shipped default.
    pub fn save(&self, profile: AgentProfile) -> Result<AgentProfile> {
        let profile = profile.sanitised();
        {
            let mut profiles = self.profiles.lock().map_err(|_| {
                crate::error::Error::InvalidConfig("agent store lock poisoned".into())
            })?;

            if profile.builtin {
                if let Some(existing) = profiles.iter_mut().find(|p| p.id == profile.id) {
                    *existing = profile.clone();
                } else {
                    profiles.push(profile.clone());
                }
            } else if let Some(slot) = profiles.iter_mut().find(|p| p.id == profile.id) {
                // Preserve the built-in flag: a profile cannot become built-in
                // just because it was loaded into an editor.
                let was_builtin = slot.builtin;
                *slot = AgentProfile {
                    builtin: was_builtin,
                    ..profile.clone()
                };
            } else {
                profiles.push(profile.clone());
            }
        }
        self.persist()?;
        Ok(profile)
    }

    /// Store a copy of a built-in under a new id, so editing a preset is safe.
    pub fn duplicate(&self, id: &str, new_name: Option<&str>) -> Result<AgentProfile> {
        let source = self.get(id).ok_or_else(|| {
            crate::error::Error::InvalidDefinition(format!("unknown agent '{id}'"))
        })?;

        let name = new_name
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("{} (copy)", source.name));
        // A unique id, so duplicating twice does not silently overwrite.
        let mut copy_id = format!("{}-copy", source.id);
        let mut n = 2;
        while self.get(&copy_id).is_some() {
            copy_id = format!("{}-copy-{}", source.id, n);
            n += 1;
        }

        let copy = AgentProfile {
            id: copy_id,
            name,
            builtin: false,
            ..source
        };
        self.save(copy)
    }

    /// Delete a profile.
    ///
    /// Built-ins are refused: removing them would leave the user with no agents.
    pub fn delete(&self, id: &str) -> Result<bool> {
        let removed = {
            let mut profiles = self.profiles.lock().map_err(|_| {
                crate::error::Error::InvalidConfig("agent store lock poisoned".into())
            })?;
            let target = profiles.iter().find(|p| p.id == id).cloned();
            match target {
                Some(p) if p.builtin => return Ok(false),
                Some(_) => {
                    profiles.retain(|p| p.id != id);
                    true
                }
                None => false,
            }
        };
        if removed {
            self.persist()?;
        }
        Ok(removed)
    }

    /// Duplicate every entry, returning `false` and removing the file so the
    /// next load starts from presets.
    pub fn reset(&self) -> Result<bool> {
        {
            let mut profiles = self.profiles.lock().map_err(|_| {
                crate::error::Error::InvalidConfig("agent store lock poisoned".into())
            })?;
            *profiles = AgentProfile::presets()
                .into_iter()
                .map(|p| p.sanitised())
                .collect();
        }
        self.persist()?;
        Ok(true)
    }

    fn persist(&self) -> Result<()> {
        // An in-memory store has no path; nothing to write.
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let profiles = self
            .profiles
            .lock()
            .map_err(|_| crate::error::Error::InvalidConfig("agent store lock poisoned".into()))?;
        let json = serde_json::to_string_pretty(&*profiles)?;
        std::fs::write(&self.path, json)?;
        Ok(())
    }
}

/// Add any missing built-in presets, preferring the on-disk version.
fn merge_presets(saved: Vec<AgentProfile>) -> Vec<AgentProfile> {
    let mut merged = saved;
    for preset in AgentProfile::presets() {
        if !merged.iter().any(|p| p.id == preset.id) {
            merged.push(preset);
        }
    }
    merged
}

/// The shared store, created once on first use.
pub fn agent_store() -> &'static Arc<AgentStore> {
    static STORE: std::sync::OnceLock<Arc<AgentStore>> = std::sync::OnceLock::new();
    STORE.get_or_init(|| Arc::new(AgentStore::load_or_default(None)))
}

/// Creatures available for an agent to be bound to, as (id, name) pairs.
///
/// Lives here rather than in the binary so the editor's picker and the running
/// app read the same catalogue and cannot drift apart.
pub fn known_creatures() -> Vec<(String, String)> {
    creature_catalogue()
        .into_iter()
        .map(|c| (c.id, c.name))
        .collect()
}

/// A map of agent id to display name, for pickers and IPC.
pub fn agent_index() -> BTreeMap<String, String> {
    agent_store()
        .list()
        .into_iter()
        .map(|p| (p.id, p.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_store_has_the_presets() {
        let store = AgentStore::in_memory();
        let list = store.list();
        assert!(
            list.len() >= 5,
            "expected the built-in presets, got {}",
            list.len()
        );
        assert!(list.iter().all(|p| p.builtin));
    }

    #[test]
    fn presets_are_ordered_before_user_profiles() {
        let store = AgentStore::in_memory();
        store
            .save(AgentProfile::new("zebra", "Zebra"))
            .expect("save");
        let list = store.list();
        let first_user = list
            .iter()
            .position(|p| !p.builtin)
            .expect("a user profile");
        let last_builtin = list.iter().rposition(|p| p.builtin).expect("a preset");
        assert!(
            first_user > last_builtin,
            "user profiles must sort after presets"
        );
    }

    #[test]
    fn user_profiles_sort_alphabetically() {
        let store = AgentStore::in_memory();
        for name in ["delta", "alpha", "charlie"] {
            store.save(AgentProfile::new(name, name)).expect("save");
        }
        let user_names: Vec<String> = store
            .list()
            .into_iter()
            .filter(|p| !p.builtin)
            .map(|p| p.name)
            .collect();
        assert_eq!(user_names, vec!["alpha", "charlie", "delta"]);
    }

    #[test]
    fn saving_the_same_id_updates_rather_than_duplicates() {
        let store = AgentStore::in_memory();
        let before = store.list().len();
        let mut profile = AgentProfile::new("mine", "Mine");
        store.save(profile.clone()).expect("save");
        profile.name = "Renamed".into();
        store.save(profile).expect("update");

        let list = store.list();
        assert_eq!(list.len(), before + 1, "must update in place");
        assert_eq!(store.get("mine").expect("get").name, "Renamed");
    }

    #[test]
    fn a_profile_round_trips_through_the_config_it_produces() {
        let mut profile = AgentProfile::new("tester", "Tester");
        profile.persona = Persona::Terse;
        profile.max_iterations = 7;
        profile.timeout_secs = 45;
        profile.system_prompt = "You test things.".into();

        let config = profile.to_agent_config();
        assert_eq!(config.max_iterations, 7);
        assert_eq!(config.timeout_secs, 45);
        assert!(
            config.system_prompt.contains("You test things."),
            "role instructions must survive: {}",
            config.system_prompt
        );
        assert!(
            config.system_prompt.contains("few words"),
            "persona guidance must be composed in: {}",
            config.system_prompt
        );
    }

    #[test]
    fn persona_guidance_reaches_the_prompt() {
        for persona in Persona::ALL {
            let mut profile = AgentProfile::new("p", "P");
            profile.persona = persona;
            let prompt = profile.composed_system_prompt();
            assert!(
                prompt.contains(persona.guidance()),
                "{persona:?} guidance missing from the prompt"
            );
        }
    }

    #[test]
    fn distinct_personas_produce_distinct_prompts() {
        let mut prompts = std::collections::BTreeSet::new();
        for persona in Persona::ALL {
            let mut profile = AgentProfile::new("p", "P");
            profile.persona = persona;
            prompts.insert(profile.composed_system_prompt());
        }
        assert_eq!(
            prompts.len(),
            Persona::ALL.len(),
            "every persona should be distinguishable in the composed prompt"
        );
    }

    #[test]
    fn a_profile_with_no_role_prompt_still_identifies_itself() {
        let profile = AgentProfile::new("solo", "Solo");
        let prompt = profile.composed_system_prompt();
        assert!(
            prompt.contains("Solo"),
            "an empty role prompt must fall back to the agent name: {prompt}"
        );
    }

    #[test]
    fn the_model_name_is_mentioned_when_set() {
        let mut profile = AgentProfile::new("p", "P");
        profile.model = "cc/claude-opus-4-6".into();
        assert!(profile
            .composed_system_prompt()
            .contains("cc/claude-opus-4-6"));
    }

    #[test]
    fn an_unset_model_is_not_announced() {
        let profile = AgentProfile::new("p", "P");
        assert!(profile.model.is_empty());
        assert!(!profile.composed_system_prompt().contains("running as"));
    }

    #[test]
    fn duplicating_a_preset_yields_an_editable_copy() {
        let store = AgentStore::in_memory();
        let copy = store
            .duplicate("builtin-coder", Some("My Coder"))
            .expect("duplicate");
        assert_eq!(copy.name, "My Coder");
        assert!(!copy.builtin, "a copy must be editable");
        assert_eq!(copy.persona, Persona::Terse, "content must carry over");
        assert_ne!(copy.id, "builtin-coder", "the copy needs its own id");
    }

    #[test]
    fn duplicating_twice_gives_two_distinct_profiles() {
        let store = AgentStore::in_memory();
        let first = store.duplicate("builtin-coder", None).expect("first");
        let second = store.duplicate("builtin-coder", None).expect("second");
        assert_ne!(
            first.id, second.id,
            "the second copy must not overwrite the first"
        );
        assert!(store.get(&first.id).is_some());
        assert!(store.get(&second.id).is_some());
    }

    #[test]
    fn built_ins_cannot_be_deleted() {
        let store = AgentStore::in_memory();
        assert!(
            !store.delete("builtin-assistant").expect("delete"),
            "a built-in must be refused, or the user ends up with no agents"
        );
        assert!(store.get("builtin-assistant").is_some());
    }

    #[test]
    fn user_profiles_can_be_deleted() {
        let store = AgentStore::in_memory();
        store.save(AgentProfile::new("temp", "Temp")).expect("save");
        assert!(store.delete("temp").expect("delete"));
        assert!(store.get("temp").is_none());
    }

    #[test]
    fn deleting_an_unknown_id_is_not_an_error() {
        let store = AgentStore::in_memory();
        assert!(!store.delete("does-not-exist").expect("no error"));
    }

    #[test]
    fn hostile_values_are_clamped_on_save() {
        // A hand-edited file must not be able to produce a profile that hangs.
        let mut profile = AgentProfile::new("bad", "");
        profile.max_iterations = 0;
        profile.timeout_secs = 99_999;
        profile.temperature = f32::NAN;
        profile.id = "../../etc/passwd".into();
        profile.name = "   ".into();

        let clean = profile.sanitised();
        assert_eq!(clean.max_iterations, 1, "must allow at least one iteration");
        assert!(clean.timeout_secs <= 3600);
        assert!(
            clean.temperature.is_finite(),
            "NaN temperature must be replaced"
        );
        assert_eq!(clean.name, "Untitled agent");
        assert!(
            !clean.id.contains('/') && !clean.id.contains(".."),
            "id must be path-safe, got {}",
            clean.id
        );
    }

    #[test]
    fn ids_are_made_safe_and_lowercase() {
        let clean = AgentProfile {
            id: "My Agent!".into(),
            ..AgentProfile::new("x", "X")
        }
        .sanitised();
        assert_eq!(clean.id, "myagent");
    }

    #[test]
    fn profiles_survive_a_round_trip_through_json() {
        let store = AgentStore::in_memory();
        let mut profile = AgentProfile::new("persist-me", "Persist Me");
        profile.persona = Persona::Wry;
        profile.provider = AgentProvider::NineRouter;
        profile.model = "cc/claude-opus-4-6".into();
        profile.personality = CreaturePersonality::Curious;
        profile.temperature = 1.25;
        store.save(profile.clone()).expect("save");

        let json = serde_json::to_string(&store.get("persist-me").expect("get")).expect("json");
        let back: AgentProfile = serde_json::from_str(&json).expect("parse");
        assert_eq!(back, profile, "every field must survive serialisation");
    }

    /// Regression: the wire format used to demand the Rust variant names ("Wry")
    /// and every field, so a profile from MCP or a hand-written file was rejected
    /// outright. Both of these are ordinary things to send.
    #[test]
    fn a_partial_profile_with_lowercase_strings_deserialises() {
        let json = serde_json::json!({
            "id": "from-agent",
            "name": "From Agent",
            "creature": "dragon-01",
            "persona": "wry",
            "provider": "nine_router",
        });
        let profile: AgentProfile = serde_json::from_value(json).expect("must parse");
        assert_eq!(profile.persona, Persona::Wry);
        assert_eq!(profile.provider, AgentProvider::NineRouter);
        assert_eq!(profile.creature, "dragon-01");
        // Absent fields take sane defaults rather than failing.
        assert!(profile.tools_enabled);
        assert_eq!(profile.max_iterations, 10);
        assert_eq!(profile.personality, CreaturePersonality::Neutral);
    }

    #[test]
    fn every_provider_serialises_as_its_stable_string() {
        for provider in AgentProvider::ALL {
            let json = serde_json::to_value(provider).expect("serialise");
            assert_eq!(json, serde_json::Value::String(provider.as_str().into()));
            assert_eq!(
                serde_json::from_value::<AgentProvider>(json).expect("parse"),
                provider
            );
        }
    }

    #[test]
    fn every_persona_serialises_as_its_stable_string() {
        for persona in Persona::ALL {
            let json = serde_json::to_value(persona).expect("serialise");
            assert_eq!(json, serde_json::Value::String(persona.as_str().into()));
            assert_eq!(
                serde_json::from_value::<Persona>(json).expect("parse"),
                persona
            );
        }
    }

    /// Regression: creature and personality travel as enum names, so a profile
    /// naming a creature has to agree with what the catalogue calls it.
    #[test]
    fn a_profile_round_trips_its_personality_enum() {
        let mut profile = AgentProfile::new("p", "P");
        for personality in [
            CreaturePersonality::Curious,
            CreaturePersonality::Shy,
            CreaturePersonality::Lazy,
            CreaturePersonality::Energetic,
            CreaturePersonality::Neutral,
        ] {
            profile.personality = personality;
            let json = serde_json::to_string(&profile).expect("serialise");
            let back: AgentProfile = serde_json::from_str(&json).expect("parse");
            assert_eq!(back.personality, personality);
        }
    }

    #[test]
    fn an_unknown_provider_string_falls_back_to_auto() {
        // A profile from a newer build must still load rather than vanish.
        assert_eq!(AgentProvider::parse("something-new"), AgentProvider::Auto);
    }

    #[test]
    fn an_unknown_persona_string_falls_back_to_assistant() {
        assert_eq!(Persona::parse("grumpy"), Persona::Assistant);
    }

    #[test]
    fn provider_and_persona_labels_and_strings_round_trip() {
        for provider in AgentProvider::ALL {
            assert_eq!(
                AgentProvider::parse(provider.as_str()),
                provider,
                "{provider:?} did not round-trip"
            );
            assert!(!provider.label().is_empty());
        }
        for persona in Persona::ALL {
            assert_eq!(Persona::parse(persona.as_str()), persona);
            assert!(!persona.label().is_empty());
        }
    }

    #[test]
    fn resetting_restores_the_presets_and_drops_user_profiles() {
        let store = AgentStore::in_memory();
        store.save(AgentProfile::new("mine", "Mine")).expect("save");
        assert!(store.reset().expect("reset"));
        assert!(store.get("mine").is_none());
        assert!(store.get("builtin-coder").is_some());
    }

    #[test]
    fn presets_differ_from_each_other_in_useful_ways() {
        let presets = AgentProfile::presets();
        // A preset set where every agent behaves identically is not a preset set.
        let distinct_creatures: std::collections::BTreeSet<_> =
            presets.iter().map(|p| p.creature.clone()).collect();
        assert!(
            distinct_creatures.len() >= 4,
            "presets should span several creatures, got {distinct_creatures:?}"
        );
        assert!(
            presets.iter().all(|p| !p.creature.is_empty()),
            "every preset needs a creature to be visible on screen"
        );
        assert!(
            presets.iter().all(|p| !p.persona.guidance().is_empty()),
            "every preset needs persona guidance"
        );
    }

    #[test]
    fn presets_do_not_all_claim_the_same_model() {
        let presets = AgentProfile::presets();
        // Left as Auto by design, but this guards against a preset hardcoding a
        // model that may not be configured on the user's machine.
        assert!(
            presets.iter().all(|p| p.provider == AgentProvider::Auto),
            "presets should defer to the engine rather than naming a provider"
        );
    }

    #[test]
    fn the_creature_list_is_not_empty() {
        // A picker with no options is a dead picker.
        assert!(!known_creatures().is_empty());
    }

    #[test]
    fn the_agent_index_lists_every_profile() {
        let store = AgentStore::in_memory();
        let index: BTreeMap<String, String> =
            store.list().into_iter().map(|p| (p.id, p.name)).collect();
        assert_eq!(index.len(), store.list().len());
        assert!(index.contains_key("builtin-assistant"));
    }
}
