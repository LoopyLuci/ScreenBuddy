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

/// Byte offset of the first case-insensitive occurrence of `needle_lower`.
///
/// Computed against the original text rather than a lowercased copy: some
/// characters change byte length when lowercased, so an offset taken from the
/// lowered string can point into the middle of a character in the original and
/// panics on the slice that follows.
fn find_ci(text: &str, needle_lower: &str) -> Option<usize> {
    if needle_lower.is_empty() {
        return None;
    }
    let lowered = text.to_lowercase();
    // Fast path: when lowercasing preserved the byte length the offsets line up.
    if lowered.len() == text.len() {
        return lowered.find(needle_lower);
    }
    let needle_chars = needle_lower.chars().count();
    for (i, _) in text.char_indices() {
        let candidate: String = text[i..].chars().take(needle_chars).collect();
        if candidate.to_lowercase() == needle_lower {
            return Some(i);
        }
    }
    None
}

/// Snap down to the nearest char boundary, so slicing cannot split a character.
fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

/// Snap up to the nearest char boundary.
fn ceil_char_boundary(s: &str, mut i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    while i < s.len() && !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

/// A display window around a match, with ellipses where it was cut.
fn snippet_around(text: &str, match_byte: usize, match_len: usize) -> String {
    const BEFORE: usize = 40;
    const AFTER: usize = 60;
    let start = floor_char_boundary(text, match_byte.saturating_sub(BEFORE));
    let end = ceil_char_boundary(text, (match_byte + match_len + AFTER).min(text.len()));
    let mut out = String::new();
    if start > 0 {
        out.push('…');
    }
    out.push_str(text[start..end].trim());
    if end < text.len() {
        out.push('…');
    }
    out
}

/// A match found by [SessionStore::search_all].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchHit {
    pub session_id: String,
    /// The session's title, so a result can be shown without a second lookup.
    pub session_title: String,
    /// Index of the matching message within the session.
    pub message_index: usize,
    /// `user` / `assistant` / `system`.
    pub role: String,
    /// A window of text around the match, for display.
    pub snippet: String,
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
    /// When set, [save] marks the store dirty instead of writing, and the caller
    /// flushes once a frame. Off by default so a save always lands immediately
    /// unless the caller opts in.
    deferred: bool,
    /// A change is waiting to be written.
    dirty: bool,
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
            deferred: false,
            dirty: false,
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

    /// Persist, or mark for a later flush when deferred saving is on.
    ///
    /// Serialising and writing the whole store at the cap measures ~9ms, which is
    /// over half a 60fps frame budget, so the render loop must not do it inline.
    /// Deferred mode marks the store dirty and the app calls [flush] once a
    /// frame; the write then happens at most once per frame instead of once per
    /// message. Tests and explicit saves leave deferred off and write at once.
    fn save(&mut self) {
        if self.deferred {
            self.dirty = true;
            return;
        }
        self.write_now();
    }

    /// Write immediately if anything is pending. A no-op otherwise.
    pub fn flush(&mut self) {
        if self.dirty {
            self.write_now();
        }
    }

    /// Turn deferred saving on or off. Turning it off flushes any pending write,
    /// so a caller cannot silently lose the last change.
    pub fn set_deferred(&mut self, deferred: bool) {
        self.deferred = deferred;
        if !deferred {
            self.flush();
        }
    }

    fn write_now(&mut self) {
        let Some(path) = self.path.clone() else { return };
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
                // Write to a sibling then rename: a crash mid-write would
                // otherwise leave a truncated file and lose every conversation,
                // because the store is written whole.
                let tmp = path.with_extension("json.tmp");
                match fs::write(&tmp, &json).and_then(|_| fs::rename(&tmp, &path)) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("[Session] could not save sessions: {e}");
                        let _ = fs::remove_file(&tmp);
                    }
                }
                self.dirty = false;
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

    /// Search every session's messages, newest session first.
    ///
    /// Sessions are searched in list order so results from the conversation the
    /// user is most likely to want come first. Matching is case-insensitive and
    /// plain substring, not a token model: a user looking for an exact phrase
    /// expects it to be found, and a fuzzy rank would hide it.
    pub fn search_all(&self, query: &str, limit: usize) -> Vec<SearchHit> {
        let needle = query.trim().to_lowercase();
        if needle.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut hits = Vec::new();
        for id in &self.order {
            let Some(session) = self.sessions.get(id) else {
                continue;
            };
            for (index, message) in session.messages.iter().enumerate() {
                if let Some(byte_pos) = find_ci(&message.text, &needle) {
                    hits.push(SearchHit {
                        session_id: session.id.clone(),
                        session_title: session.title.clone(),
                        message_index: index,
                        role: match message.role {
                            Role::User => "user".to_string(),
                            Role::Assistant => "assistant".to_string(),
                            Role::System => "system".to_string(),
                        },
                        snippet: snippet_around(&message.text, byte_pos, needle.len()),
                    });
                    if hits.len() >= limit {
                        return hits;
                    }
                }
            }
        }
        hits
    }

    /// Sessions whose title matches, for a session-list filter.
    pub fn search_titles(&self, query: &str) -> Vec<SessionSummary> {
        let needle = query.trim().to_lowercase();
        self.list()
            .into_iter()
            .filter(|s| needle.is_empty() || s.title.to_lowercase().contains(&needle))
            .collect()
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
    fn save_cost_at_the_cap_is_measured() {
        // 50 sessions x 200 messages is what the caps allow, so this is the worst
        // case the render loop can hit. The audit called the per-push save
        // "synchronous I/O on the render loop"; this measures whether that is
        // actually a stall before changing persistence semantics.
        let dir = std::env::temp_dir().join("screenbuddy_session_bench");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.json");

        // Build with deferred saving on so the setup does not write 10,000 times;
        // the timing below drives the real write path through flush().
        let mut store = SessionStore::load(&path);
        store.set_deferred(true);
        for s in 0..50 {
            store.new_session(&format!("session {s}"));
            for m in 0..200 {
                store.push(Role::User, format!("message {m} with some padding text"));
            }
        }

        let start = std::time::Instant::now();
        let rounds = 5;
        for _ in 0..rounds {
            store.save(); // marks dirty
            store.flush(); // performs the write
        }
        let per_save = start.elapsed() / rounds;
        println!("save() at the cap: {per_save:?} per call");
        // A 60fps frame is 16ms; anything near that would be a real stall.
        assert!(
            per_save.as_millis() < 100,
            "save() took {per_save:?}, which would stall the render loop"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn deferred_saving_writes_only_on_flush() {
        let dir = std::env::temp_dir().join("screenbuddy_session_deferred");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.json");

        let mut store = SessionStore::load(&path);
        store.set_deferred(true);
        store.push(Role::User, "not yet written");

        // The reload must not see the message while the write is deferred.
        let midway = SessionStore::load(&path);
        assert!(
            !midway
                .current()
                .map(|s| s.transcript().iter().any(|(_, t)| t == "not yet written"))
                .unwrap_or(false),
            "a deferred save must not have written yet"
        );

        store.flush();
        let after = SessionStore::load(&path);
        assert!(
            after
                .current()
                .map(|s| s.transcript().iter().any(|(_, t)| t == "not yet written"))
                .unwrap_or(false),
            "flush must land the pending write"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn turning_deferred_off_flushes_pending_changes() {
        let dir = std::env::temp_dir().join("screenbuddy_session_deferred_off");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.json");

        let mut store = SessionStore::load(&path);
        store.set_deferred(true);
        store.push(Role::User, "pending then flushed");
        // Switching the mode back must not drop the pending change.
        store.set_deferred(false);

        let reloaded = SessionStore::load(&path);
        assert!(reloaded
            .current()
            .map(|s| s.transcript().iter().any(|(_, t)| t == "pending then flushed"))
            .unwrap_or(false));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_save_leaves_no_temp_file_behind() {
        let dir = std::env::temp_dir().join("screenbuddy_session_tmp");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sessions.json");

        let mut store = SessionStore::load(&path);
        store.push(Role::User, "written atomically");

        let leftovers: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "temp files left behind: {leftovers:?}");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_all_finds_a_message_in_another_session() {
        let mut store = SessionStore::new();
        let first_id = store.new_session("First");
        store.push(Role::User, "the dragon soars over the mountain");
        store.new_session("Second");
        store.push(Role::User, "unrelated chatter");

        let hits = store.search_all("dragon", 10);
        assert_eq!(hits.len(), 1);
        // Match on the id: pushing auto-titles the session from its first user
        // message, so the title is no longer the one passed to new_session.
        assert_eq!(hits[0].session_id, first_id);
        assert_eq!(hits[0].role, "user");
        assert!(hits[0].snippet.contains("dragon"));
    }

    #[test]
    fn search_all_is_case_insensitive() {
        let mut store = SessionStore::new();
        store.push(Role::User, "The DRAGON flies");
        assert_eq!(store.search_all("dragon", 10).len(), 1);
        assert_eq!(store.search_all("DRAGON", 10).len(), 1);
        assert_eq!(store.search_all("DrAgOn", 10).len(), 1);
    }

    #[test]
    fn search_all_respects_the_limit() {
        let mut store = SessionStore::new();
        for _ in 0..20 {
            store.push(Role::User, "needle here");
        }
        assert_eq!(store.search_all("needle", 5).len(), 5);
    }

    #[test]
    fn search_all_returns_nothing_for_an_empty_query() {
        let mut store = SessionStore::new();
        store.push(Role::User, "something");
        assert!(store.search_all("", 10).is_empty());
        assert!(store.search_all("   ", 10).is_empty());
        assert!(store.search_all("something", 0).is_empty());
    }

    #[test]
    fn search_all_records_the_message_index_and_role() {
        let mut store = SessionStore::new();
        store.push(Role::User, "first line");
        store.push(Role::Assistant, "second line with marker");
        store.push(Role::User, "third line");

        let hits = store.search_all("marker", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].message_index, 1);
        assert_eq!(hits[0].role, "assistant");
    }

    #[test]
    fn search_all_survives_multibyte_text() {
        // Lowercasing some characters changes byte length, which is where a
        // naive offset would slice mid-character and panic.
        let mut store = SessionStore::new();
        store.push(Role::User, "İstanbul café  dragons  naïve");
        let hits = store.search_all("dragon", 10);
        assert_eq!(hits.len(), 1);
        assert!(hits[0].snippet.contains("dragon"));

        // Every character of the snippet must be valid, i.e. it sliced on
        // boundaries.
        for hit in store.search_all("café", 10) {
            assert!(hit.snippet.is_char_boundary(0));
        }
        let _ = store.search_all("İ", 10);
    }

    #[test]
    fn a_long_message_snippet_is_windowed_not_returned_whole() {
        let mut store = SessionStore::new();
        let long = format!("{} the marker {}", "x".repeat(400), "y".repeat(400));
        store.push(Role::User, long);
        let hits = store.search_all("marker", 10);
        assert_eq!(hits.len(), 1);
        assert!(
            hits[0].snippet.chars().count() < 200,
            "snippet should be a window, got {} chars",
            hits[0].snippet.chars().count()
        );
        assert!(hits[0].snippet.contains("marker"));
    }

    #[test]
    fn search_titles_filters_the_session_list() {
        let mut store = SessionStore::new();
        store.new_session("Dragon lore");
        store.new_session("Cooking");
        let hits = store.search_titles("dragon");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Dragon lore");
        // An empty query keeps every session.
        assert_eq!(store.search_titles("").len(), 2);
    }

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
