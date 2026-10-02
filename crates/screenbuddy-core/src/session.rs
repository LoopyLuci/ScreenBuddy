//! Chat sessions: history, persistence, and the store the GUI renders.
//!
//! The GUI and the MCP control surface must show the same conversation, so the
//! transcript lives here rather than inside a window. A window owns a view of a
//! session; it does not own the conversation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Who sent a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::System => "system",
        }
    }

    /// Parse the role names used by the IPC/MCP surface.
    pub fn parse(s: &str) -> Option<Role> {
        match s.trim().to_ascii_lowercase().as_str() {
            "user" | "human" | "you" => Some(Role::User),
            "assistant" | "ai" | "bot" => Some(Role::Assistant),
            "system" => Some(Role::System),
            _ => None,
        }
    }
}

/// A single turn in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub text: String,
    /// Unix seconds. Kept as an integer so JSON stays stable across platforms.
    pub timestamp: i64,
    /// False while an assistant turn is still being generated, so the GUI can
    /// show a typing indicator instead of a blank bubble.
    #[serde(default = "default_true")]
    pub complete: bool,
}

fn default_true() -> bool {
    true
}

impl Message {
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            role,
            text: text.into(),
            timestamp: now_secs(),
            complete: true,
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A named conversation.
///
/// A session outlives a window: closing the chat does not end the conversation,
/// and a session can be reopened from the session list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub messages: Vec<Message>,
}

impl Session {
    pub fn new(id: impl Into<String>) -> Self {
        let now = now_secs();
        Self {
            id: id.into(),
            title: "New chat".to_string(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    /// A title derived from the first user message, which is far more useful in
    /// a session list than "New chat" repeated.
    pub fn auto_title(&mut self) {
        if let Some(first) = self
            .messages
            .iter()
            .find(|m| m.role == Role::User && !m.text.trim().is_empty())
        {
            let mut t: String = first.text.trim().chars().take(40).collect();
            if first.text.trim().chars().count() > 40 {
                t.push('…');
            }
            self.title = t;
        }
    }

    pub fn push(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = now_secs();
        self.auto_title();
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// The transcript as (role, text) pairs, which is what the chat window and
    /// the IPC surface both consume.
    pub fn transcript(&self) -> Vec<(String, String)> {
        self.messages
            .iter()
            .map(|m| (m.role.as_str().to_string(), m.text.clone()))
            .collect()
    }

    /// Whether an assistant reply is still pending.
    pub fn has_pending(&self) -> bool {
        self.messages.iter().any(|m| !m.complete)
    }

    /// Mark any incomplete assistant turn as finished.
    pub fn close_pending(&mut self) {
        for m in &mut self.messages {
            if !m.complete {
                m.complete = true;
            }
        }
    }
}

/// In-memory store of sessions, persisted to disk so a restart does not lose the
/// conversation.
#[derive(Debug)]
pub struct SessionStore {
    sessions: HashMap<String, Session>,
    /// Insertion order, so the session list is stable rather than hash order.
    order: Vec<String>,
    current_id: String,
    path: Option<PathBuf>,
    next_id: AtomicU64,
}

/// How many sessions to keep. A desktop companion should not accumulate an
/// unbounded transcript.
const MAX_SESSIONS: usize = 50;
/// Messages retained per session, mirroring the chat window's own cap.
const MAX_MESSAGES_PER_SESSION: usize = 200;

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStore {
    /// A store with no sessions, for tests and for callers that will add their own.
    ///
    /// Prefer [SessionStore::load] for the app: it guarantees at least one session
    /// exists, which the UI and the session list both rely on.
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            order: Vec::new(),
            current_id: String::new(),
            path: None,
            next_id: AtomicU64::new(1),
        }
    }

    /// A store with one empty session ready to use.
    pub fn with_default_session() -> Self {
        let mut s = Self::new();
        s.new_session("New chat");
        s
    }

    /// Load from `path`, creating a session when the file is missing or corrupt.
    /// A corrupt store must not stop the app from starting.
    pub fn load(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref().to_path_buf();
        let mut store = Self::new();
        store.path = Some(path.clone());

        if let Ok(raw) = fs::read_to_string(&path) {
            if let Ok(list) = serde_json::from_str::<Vec<Session>>(&raw) {
                for session in list {
                    store.order.push(session.id.clone());
                    store.sessions.insert(session.id.clone(), session);
                }
            } else {
                eprintln!("[Session] ignoring unreadable session file at {path:?}");
            }
        }

        // Never leave the store without somewhere to write.
        if store.order.is_empty() {
            let _ = store.new_session("New chat");
        } else {
            store.current_id = store.order[0].clone();
        }
        store
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let list: Vec<Session> = self
            .order
            .iter()
            .filter_map(|id| self.sessions.get(id).cloned())
            .collect();
        match serde_json::to_string_pretty(&list) {
            Ok(json) => {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Err(e) = fs::write(path, json) {
                    eprintln!("[Session] could not save sessions: {e}");
                }
            }
            Err(e) => eprintln!("[Session] could not serialise sessions: {e}"),
        }
    }

    fn mint_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("session-{n}")
    }

    /// Create a session and make it current.
    pub fn new_session(&mut self, title: &str) -> String {
        let mut session = Session::new(self.mint_id());
        session.title = title.to_string();
        let id = session.id.clone();
        self.order.insert(0, id.clone());
        self.sessions.insert(id.clone(), session);
        self.current_id = id.clone();
        self.prune();
        self.save();
        id
    }

    /// Drop the oldest sessions past the cap, keeping the current one.
    fn prune(&mut self) {
        while self.order.len() > MAX_SESSIONS {
            let Some(victim) = self.order.last().cloned() else {
                break;
            };
            if victim == self.current_id {
                break;
            }
            self.order.pop();
            self.sessions.remove(&victim);
        }
    }

    pub fn current(&self) -> Option<&Session> {
        self.sessions.get(&self.current_id)
    }

    pub fn current_mut(&mut self) -> Option<&mut Session> {
        self.sessions.get_mut(&self.current_id)
    }

    pub fn current_id(&self) -> &str {
        &self.current_id
    }

    /// Switch to an existing session. Unknown ids are rejected rather than
    /// silently creating an empty one, so a bad id from an agent is visible.
    pub fn select(&mut self, id: &str) -> Result<&Session, String> {
        if !self.sessions.contains_key(id) {
            return Err(format!("unknown session: {id}"));
        }
        self.current_id = id.to_string();
        Ok(self.sessions.get(id).expect("checked above"))
    }

    pub fn get(&self, id: &str) -> Option<&Session> {
        self.sessions.get(id)
    }

    /// Sessions newest-first, for the GUI's session list.
    pub fn list(&self) -> Vec<SessionSummary> {
        self.order
            .iter()
            .filter_map(|id| self.sessions.get(id))
            .map(|s| SessionSummary {
                id: s.id.clone(),
                title: s.title.clone(),
                updated_at: s.updated_at,
                message_count: s.messages.len(),
                is_current: s.id == self.current_id,
            })
            .collect()
    }

    /// Append to the current session and persist.
    pub fn push(&mut self, role: Role, text: impl Into<String>) {
        self.push_to_current(Message::new(role, text));
    }

    pub fn push_to_current(&mut self, message: Message) {
        if self.current_id.is_empty() {
            self.new_session("New chat");
        }
        if let Some(session) = self.current_id_mut() {
            session.push(message);
            if session.messages.len() > MAX_MESSAGES_PER_SESSION {
                let excess = session.messages.len() - MAX_MESSAGES_PER_SESSION;
                session.messages.drain(0..excess);
            }
        }
        self.save();
    }

    fn current_id_mut(&mut self) -> Option<&mut Session> {
        let id = self.current_id.clone();
        self.sessions.get_mut(&id)
    }

    /// Delete a session, keeping at least one so the UI is never empty.
    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        if !self.sessions.contains_key(id) {
            return Err(format!("unknown session: {id}"));
        }
        if self.order.len() == 1 {
            return Err("cannot delete the only session".into());
        }
        self.order.retain(|x| x != id);
        self.sessions.remove(id);
        if self.current_id == id {
            if let Some(next) = self.order.first().cloned() {
                self.current_id = next;
            }
        }
        self.save();
        Ok(())
    }

    /// Clear the current session's messages, keeping the session itself.
    pub fn clear_current(&mut self) {
        if let Some(session) = self.current_id_mut() {
            session.messages.clear();
            session.title = "New chat".to_string();
        }
        self.save();
    }

    pub fn rename(&mut self, id: &str, title: &str) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(id)
            .ok_or_else(|| format!("unknown session: {id}"))?;
        session.title = title.to_string();
        self.save();
        Ok(())
    }
}

/// Lightweight row for the session list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub updated_at: i64,
    pub message_count: usize,
    pub is_current: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_session_becomes_current() {
        let mut store = SessionStore::new();
        let id = store.new_session("Test");
        assert_eq!(store.current_id(), id);
        assert!(store.current().expect("current exists").is_empty());
    }

    #[test]
    fn first_user_message_becomes_the_title() {
        let mut store = SessionStore::new();
        store.push(Role::User, "How do I make the dragon fly?");
        assert_eq!(
            store.current().expect("current").title,
            "How do I make the dragon fly?"
        );
    }

    #[test]
    fn long_first_message_is_truncated_for_the_title() {
        let mut store = SessionStore::new();
        store.push(Role::User, "a".repeat(80));
        let title = &store.current().expect("current").title;
        assert!(title.chars().count() <= 41, "title too long: {title}");
        assert!(title.ends_with('…'));
    }

    #[test]
    fn assistant_reply_does_not_set_the_title() {
        let mut store = SessionStore::new();
        store.push(Role::Assistant, "I am a greeting");
        assert_eq!(store.current().expect("current").title, "New chat");
    }

    #[test]
    fn sessions_are_listed_newest_first() {
        let mut store = SessionStore::new();
        let a = store.new_session("A");
        let b = store.new_session("B");
        let list = store.list();
        assert_eq!(list[0].id, b, "most recent first");
        assert_eq!(list[1].id, a);
        assert!(
            list.iter()
                .find(|s| s.id == b)
                .expect("b listed")
                .is_current
        );
    }

    #[test]
    fn selecting_switches_the_current_session() {
        let mut store = SessionStore::new();
        let a = store.new_session("A");
        store.new_session("B");
        store.select(&a).expect("select a");
        assert_eq!(store.current_id(), a);
        store.push(Role::User, "in A");
        assert_eq!(store.get(&a).expect("a").messages.len(), 1);
    }

    #[test]
    fn selecting_an_unknown_session_is_an_error() {
        let mut store = SessionStore::new();
        assert!(store.select("nope").is_err());
    }

    #[test]
    fn deleting_switches_current_and_refuses_the_last_session() {
        let mut store = SessionStore::new();
        let a = store.new_session("A");
        let b = store.new_session("B");
        store.delete(&b).expect("delete b");
        assert_eq!(store.current_id(), a);
        assert!(store.select(&b).is_err(), "b should be gone");

        // The last session is never deletable: the session list and the chat view
        // both need somewhere to be.
        assert!(store.delete(&a).is_err(), "cannot delete the last session");
        assert_eq!(store.list().len(), 1);
        assert!(store.get(&a).is_some(), "a still exists");
    }

    #[test]
    fn clearing_keeps_the_session_but_empties_it() {
        let mut store = SessionStore::new();
        store.push(Role::User, "hello");
        let id = store.current_id().to_string();
        store.clear_current();
        assert!(store.current().expect("current").is_empty());
        assert_eq!(store.current_id(), id, "same session, just emptied");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn message_history_is_capped() {
        let mut store = SessionStore::new();
        for i in 0..(MAX_MESSAGES_PER_SESSION + 25) {
            store.push(Role::User, format!("message {i}"));
        }
        let len = store.current().expect("current").messages.len();
        assert_eq!(len, MAX_MESSAGES_PER_SESSION);
        // The oldest were dropped, the newest kept.
        assert!(store
            .current()
            .expect("current")
            .messages
            .last()
            .expect("last")
            .text
            .contains("message 224"));
    }

    #[test]
    fn pending_messages_are_visible_until_closed() {
        let mut session = Session::new("s");
        session.push(Message::new(Role::Assistant, "typing..."));
        assert!(!session.has_pending());
        // Simulate an in-flight turn.
        session.messages.push(Message {
            role: Role::Assistant,
            text: String::new(),
            timestamp: 0,
            complete: false,
        });
        assert!(session.has_pending());
        session.close_pending();
        assert!(!session.has_pending());
    }

    #[test]
    fn transcript_pairs_roles_with_text() {
        let mut store = SessionStore::new();
        store.push(Role::User, "hi");
        store.push(Role::Assistant, "hello");
        let t = store.current().expect("current").transcript();
        assert_eq!(
            t,
            vec![
                ("user".into(), "hi".into()),
                ("assistant".into(), "hello".into())
            ]
        );
    }

    #[test]
    fn roles_parse_from_the_wire_names() {
        assert_eq!(Role::parse("user"), Some(Role::User));
        assert_eq!(Role::parse("ASSISTANT"), Some(Role::Assistant));
        assert_eq!(Role::parse("bot"), Some(Role::Assistant));
        assert_eq!(Role::parse("nonsense"), None);
    }

    // ---- persistence ----

    fn temp_path(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!(
            "screenbuddy-session-test-{tag}-{}.json",
            std::process::id()
        ));
        p
    }

    #[test]
    fn sessions_survive_a_reload() {
        let path = temp_path("reload");
        let _ = std::fs::remove_file(&path);

        let mut store = SessionStore::load(&path);
        store.push(Role::User, "remember me");
        store.new_session("second");

        let reloaded = SessionStore::load(&path);
        assert_eq!(reloaded.list().len(), 2);
        let first = reloaded
            .list()
            .into_iter()
            .find(|s| s.title == "remember me")
            .expect("session restored");
        assert_eq!(first.message_count, 1);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_corrupt_store_still_yields_a_usable_session() {
        let path = temp_path("corrupt");
        std::fs::write(&path, "{ this is not json").expect("write");
        let store = SessionStore::load(&path);
        assert_eq!(store.list().len(), 1, "fell back to a fresh session");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let path = temp_path("missing");
        let _ = std::fs::remove_file(&path);
        let store = SessionStore::load(&path);
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn rename_changes_the_title_without_touching_messages() {
        let mut store = SessionStore::new();
        store.push(Role::User, "original");
        let id = store.current_id().to_string();
        store.rename(&id, "My title").expect("rename");
        assert_eq!(store.get(&id).expect("session").title, "My title");
        assert_eq!(store.get(&id).expect("session").messages.len(), 1);
        assert!(store.rename("nope", "x").is_err());
    }
}
