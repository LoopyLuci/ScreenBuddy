//! IPC bridge for ScreenBuddy.
//!
//! A length-prefixed JSON protocol over TCP. Two roles share the transport:
//!
//! * The **Godot editor** authors creature definitions (`SetCreature`,
//!   `ExportCreature`, ...).
//! * An **external agent** (e.g. Hermes via the bundled MCP server) drives the
//!   running app: chat, creature control, audio, settings, status.
//!
//! Control requests arrive over the socket and are pushed onto a broadcast
//! channel that the main loop drains each frame, so handlers never touch UI or
//! render state directly from the network thread.
use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, Notify};

pub const DEFAULT_PORT: u16 = 34567;
/// Frame size ceiling. Guards against a malformed length prefix allocating wildly.
pub const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;

/// Default / maximum number of `memory_search` hits returned.
pub const DEFAULT_SEARCH_LIMIT: usize = 5;
pub const MAX_SEARCH_LIMIT: usize = 50;

/// Creature animation states the runtime can be switched to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnimStateCtl {
    Idle,
    Walk,
    Fly,
    Sleep,
    Celebrate,
}

impl AnimStateCtl {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Fly => "fly",
            Self::Sleep => "sleep",
            Self::Celebrate => "celebrate",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum GodotCommand {
    // --- authoring (Godot editor) ---
    Ping,
    ListCreatures,
    GetCreature {
        id: String,
    },
    SetCreature {
        id: String,
        data: serde_json::Value,
    },
    ReloadCreature {
        id: String,
    },
    ExportCreature {
        data: serde_json::Value,
    },
    SaveConfig {
        config: serde_json::Value,
    },
    LoadConfig,
    Unknown {
        raw: String,
    },

    // --- runtime control (external agent / MCP) ---
    /// Send a chat message through the AI bridge.
    SendChat {
        message: String,
    },
    /// Run a prompt through the agent runtime (tool use enabled).
    RunAgent {
        prompt: String,
    },
    /// Current chat transcript, newest last.
    GetChatHistory,
    /// Set the animation state of one creature, or all when `id` is absent.
    SetAnimation {
        id: Option<String>,
        state: AnimStateCtl,
    },
    /// Move a creature to an absolute screen position.
    MoveCreature {
        id: String,
        x: f32,
        y: f32,
    },
    /// Show or hide a creature.
    SetCreatureVisible {
        id: String,
        visible: bool,
    },
    /// List the creatures currently loaded, with position and state.
    ListCreatureState,
    /// Play a named sound effect.
    PlaySound {
        name: String,
    },
    /// Speak text via the TTS engine.
    Speak {
        text: String,
    },
    /// Set a runtime setting by key (see the settings screen for known keys).
    SetSetting {
        key: String,
        value: serde_json::Value,
    },
    /// Read a runtime setting, or all of them when `key` is absent.
    GetSetting {
        key: Option<String>,
    },
    /// Enable or disable the automatic animation state rotation.
    SetAutoCycle {
        enabled: bool,
    },
    /// Aggregate runtime snapshot: fps, uptime, creature count, ai status.
    GetStatus,
    /// Agent tools and the tool-call transcript size.
    GetAgentInfo,
    /// Results of the most recent `memory_search`.
    GetSearchResults,
    /// Ingest a document into the RAG memory and report the chunk count.
    MemoryIngest {
        source: String,
        content: String,
    },
    /// Search RAG memory. Results land in the status snapshot; poll
    /// `get_setting`/`get_status` or read `search_results` on the next query.
    MemorySearch {
        query: String,
        limit: Option<usize>,
    },
}

/// A control request the main loop should apply on its next tick.
#[derive(Debug, Clone)]
pub enum ControlRequest {
    SendChat(String),
    RunAgent(String),
    SetAnimation {
        id: Option<String>,
        state: AnimStateCtl,
    },
    MoveCreature {
        id: String,
        x: f32,
        y: f32,
    },
    SetCreatureVisible {
        id: String,
        visible: bool,
    },
    PlaySound(String),
    Speak(String),
    SetSetting {
        key: String,
        value: serde_json::Value,
    },
    SetAutoCycle(bool),
    MemoryIngest {
        source: String,
        content: String,
    },
    /// Search RAG memory and publish the hits into the status snapshot.
    MemorySearch {
        query: String,
        limit: usize,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Success { data: Option<serde_json::Value> },
    Error { message: String, code: Option<u32> },
    Pong,
    CreatureList { creatures: Vec<String> },
    CreatureData { id: String, data: serde_json::Value },
    Ack,
}

impl Response {
    pub fn ok(data: serde_json::Value) -> Self {
        Response::Success { data: Some(data) }
    }
    pub fn err(msg: impl Into<String>) -> Self {
        Response::Error {
            message: msg.into(),
            code: None,
        }
    }
    /// True when this response reports a failure.
    pub fn is_error(&self) -> bool {
        matches!(self, Response::Error { .. })
    }
}

/// Runtime snapshot published by the render loop so IPC query commands can be
/// answered from a network thread without touching UI state directly.
///
/// Owned by `screenbuddy-core` so the publisher (the app) and the reader (the
/// IPC server) share one definition.
#[derive(Debug, Clone, Default)]
pub struct RuntimeStatus {
    pub fps: f64,
    pub frame_count: u64,
    pub creature_count: usize,
    pub visible_count: usize,
    pub auto_cycle: bool,
    pub uptime_secs: f64,
    pub model_count: usize,
    pub tool_count: usize,
    pub memory_chunks: usize,
    /// Creature id -> live state summary.
    pub creatures: HashMap<String, serde_json::Value>,
    /// Key -> current setting value.
    pub settings: HashMap<String, serde_json::Value>,
    /// Newest-last chat transcript.
    pub chat_history: Vec<(String, String)>,
    pub last_agent_tool: Option<String>,
    pub last_agent_response: Option<String>,
    /// Result of the most recent `memory_search`, as a JSON array.
    pub search_results: serde_json::Value,
    /// The query that produced `search_results`.
    pub last_query: Option<String>,
}

impl RuntimeStatus {
    /// Snapshot as JSON for `get_status`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "fps": self.fps,
            "frames": self.frame_count,
            "uptime_secs": self.uptime_secs,
            "creature_count": self.creature_count,
            "visible_count": self.visible_count,
            "auto_cycle": self.auto_cycle,
            "model_count": self.model_count,
            "tool_count": self.tool_count,
            "memory_chunks": self.memory_chunks,
        })
    }
}

#[derive(Debug, Clone)]
pub struct IpcConfig {
    pub addr: String,
    pub enable_hot_reload: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self {
            addr: format!("127.0.0.1:{}", DEFAULT_PORT),
            enable_hot_reload: true,
        }
    }
}

pub struct IpcServer {
    config: IpcConfig,
    shutdown: Arc<Notify>,
    tx: broadcast::Sender<GodotCommand>,
    /// Control requests awaiting the main loop.
    control: broadcast::Sender<ControlRequest>,
    creatures: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    /// Live snapshot written by the render loop; read by query commands.
    status: Arc<std::sync::Mutex<RuntimeStatus>>,
}

impl IpcServer {
    pub fn new(config: IpcConfig) -> Self {
        let (tx, _) = broadcast::channel(256);
        let (control, _) = broadcast::channel(256);
        Self {
            config,
            shutdown: Arc::new(Notify::new()),
            tx,
            control,
            creatures: Arc::new(Mutex::new(HashMap::new())),
            status: Arc::new(std::sync::Mutex::new(RuntimeStatus::default())),
        }
    }

    /// Shared handle the render loop publishes into each frame.
    pub fn status_handle(&self) -> Arc<std::sync::Mutex<RuntimeStatus>> {
        self.status.clone()
    }

    /// Build a server that publishes into an externally-owned snapshot.
    ///
    /// The caller must pass the *same* handle it writes each frame. Letting the
    /// server allocate its own would mean every query reads a snapshot nobody
    /// updates, and silently returns defaults forever.
    pub fn with_status(config: IpcConfig, status: Arc<std::sync::Mutex<RuntimeStatus>>) -> Self {
        let mut server = Self::new(config);
        server.status = status;
        server
    }

    pub async fn run(&self) -> Result<(), String> {
        let listener = TcpListener::bind(&self.config.addr)
            .await
            .map_err(|e| format!("Bind failed on {}: {}", self.config.addr, e))?;
        tracing::info!("IPC server listening on {}", self.config.addr);
        loop {
            tokio::select! {
                Ok((stream, peer)) = listener.accept() => {
                    tracing::debug!("IPC client connected: {}", peer);
                    let tx = self.tx.clone();
                    let control = self.control.clone();
                    let creatures = self.creatures.clone();
                    let status = self.status.clone();
                    let shutdown = self.shutdown.clone();
                    tokio::spawn(async move {
                        handle_client(stream, tx, control, creatures, status, shutdown).await;
                    });
                }
                _ = self.shutdown.notified() => break,
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        self.shutdown.notify_waiters();
    }

    pub fn subscribe(&self) -> broadcast::Receiver<GodotCommand> {
        self.tx.subscribe()
    }

    /// Subscribe to control requests. The main loop drains these each frame.
    pub fn subscribe_control(&self) -> broadcast::Receiver<ControlRequest> {
        self.control.subscribe()
    }

    pub async fn load_creature(&self, id: String, data: serde_json::Value) {
        self.creatures.lock().await.insert(id, data);
    }
}

/// Read a length-prefixed frame: 4-byte big-endian length, then that many bytes.
async fn read_frame(stream: &mut TcpStream, buf: &mut [u8]) -> std::io::Result<Option<Vec<u8>>> {
    // Header first.
    let mut header = [0u8; 4];
    match stream.read_exact(&mut header).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(header) as usize;
    if len > MAX_FRAME_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame of {len} bytes exceeds {MAX_FRAME_BYTES}"),
        ));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    let _ = buf;
    Ok(Some(payload))
}

async fn write_frame(stream: &mut TcpStream, value: &Response) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(value).unwrap_or_else(|_| b"{\"status\":\"error\"}".to_vec());
    stream
        .write_all(&(bytes.len() as u32).to_be_bytes())
        .await?;
    stream.write_all(&bytes).await
}

async fn handle_client(
    mut stream: TcpStream,
    tx: broadcast::Sender<GodotCommand>,
    control: broadcast::Sender<ControlRequest>,
    creatures: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    status: Arc<std::sync::Mutex<RuntimeStatus>>,
    shutdown: Arc<Notify>,
) {
    let mut buf = vec![0u8; 8192];
    loop {
        let frame = tokio::select! {
            frame = read_frame(&mut stream, &mut buf) => frame,
            _ = shutdown.notified() => break,
        };

        let payload = match frame {
            Ok(Some(p)) => p,
            // Clean disconnect.
            Ok(None) => break,
            Err(e) => {
                let _ = write_frame(
                    &mut stream,
                    &Response::Error {
                        message: format!("Protocol error: {e}"),
                        code: Some(400),
                    },
                )
                .await;
                break;
            }
        };

        match serde_json::from_slice::<GodotCommand>(&payload) {
            Ok(cmd) => {
                let resp = process_command(&cmd, &creatures, &control, &status).await;
                let _ = write_frame(&mut stream, &resp).await;
                // `send` fails only when there are no subscribers, which is fine:
                // the app may not be draining this channel.
                let _ = tx.send(cmd);
            }
            Err(e) => {
                let _ = write_frame(
                    &mut stream,
                    &Response::Error {
                        message: format!("Parse error: {e}"),
                        code: Some(400),
                    },
                )
                .await;
            }
        }
    }
}

/// Split a command into an authoring operation and any control request it implies.
fn control_request_for(cmd: &GodotCommand) -> Option<ControlRequest> {
    Some(match cmd {
        GodotCommand::SendChat { message } => ControlRequest::SendChat(message.clone()),
        GodotCommand::RunAgent { prompt } => ControlRequest::RunAgent(prompt.clone()),
        GodotCommand::SetAnimation { id, state } => ControlRequest::SetAnimation {
            id: id.clone(),
            state: *state,
        },
        GodotCommand::MoveCreature { id, x, y } => ControlRequest::MoveCreature {
            id: id.clone(),
            x: *x,
            y: *y,
        },
        GodotCommand::SetCreatureVisible { id, visible } => ControlRequest::SetCreatureVisible {
            id: id.clone(),
            visible: *visible,
        },
        GodotCommand::PlaySound { name } => ControlRequest::PlaySound(name.clone()),
        GodotCommand::Speak { text } => ControlRequest::Speak(text.clone()),
        GodotCommand::SetSetting { key, value } => ControlRequest::SetSetting {
            key: key.clone(),
            value: value.clone(),
        },
        GodotCommand::SetAutoCycle { enabled } => ControlRequest::SetAutoCycle(*enabled),
        GodotCommand::MemoryIngest { source, content } => ControlRequest::MemoryIngest {
            source: source.clone(),
            content: content.clone(),
        },
        GodotCommand::MemorySearch { query, limit } => ControlRequest::MemorySearch {
            query: query.clone(),
            limit: limit
                .unwrap_or(DEFAULT_SEARCH_LIMIT)
                .clamp(1, MAX_SEARCH_LIMIT),
        },
        _ => return None,
    })
}

async fn process_command(
    cmd: &GodotCommand,
    creatures: &Arc<Mutex<HashMap<String, serde_json::Value>>>,
    control: &broadcast::Sender<ControlRequest>,
    status: &Arc<std::sync::Mutex<RuntimeStatus>>,
) -> Response {
    // Query-style commands are answered from server-side state where possible.
    match cmd {
        GodotCommand::Ping => return Response::Pong,

        GodotCommand::ListCreatures => {
            let list: Vec<String> = creatures.lock().await.keys().cloned().collect();
            return Response::CreatureList { creatures: list };
        }

        GodotCommand::GetCreature { id } => {
            return match creatures.lock().await.get(id) {
                Some(data) => Response::CreatureData {
                    id: id.clone(),
                    data: data.clone(),
                },
                None => Response::Error {
                    message: format!("Creature '{id}' not found"),
                    code: Some(404),
                },
            };
        }

        GodotCommand::SetCreature { id, data } => {
            creatures.lock().await.insert(id.clone(), data.clone());
            return Response::Ack;
        }

        GodotCommand::ReloadCreature { .. } => return Response::Ack,

        GodotCommand::ExportCreature { data } => {
            let id = data
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            creatures.lock().await.insert(id.clone(), data.clone());
            return Response::ok(serde_json::json!({ "exported": id }));
        }

        GodotCommand::SaveConfig { .. } => return Response::Ack,
        GodotCommand::LoadConfig => return Response::Success { data: None },

        GodotCommand::Unknown { raw } => {
            return Response::Error {
                message: format!("Unknown command: {raw}"),
                code: Some(400),
            };
        }

        // Commands needing live runtime state are answered from the shared
        // snapshot the render loop publishes each frame.
        GodotCommand::ListCreatureState
        | GodotCommand::GetStatus
        | GodotCommand::GetChatHistory
        | GodotCommand::GetSearchResults => {
            let Ok(snapshot) = status.lock() else {
                return Response::err("status lock poisoned");
            };
            let payload = match cmd {
                GodotCommand::ListCreatureState => serde_json::json!({
                    "creatures": snapshot.creatures,
                }),
                GodotCommand::GetStatus => snapshot.to_json(),
                GodotCommand::GetChatHistory => serde_json::json!({
                    "messages": snapshot.chat_history,
                }),
                GodotCommand::GetSearchResults => serde_json::json!({
                    "query": snapshot.last_query,
                    "results": snapshot.search_results,
                }),
                _ => unreachable!("only these reach this arm"),
            };
            return Response::ok(payload);
        }

        GodotCommand::GetSetting { key } => {
            let Ok(snapshot) = status.lock() else {
                return Response::err("status lock poisoned");
            };
            return match key {
                None => Response::ok(serde_json::json!({ "settings": snapshot.settings })),
                Some(k) => match snapshot.settings.get(k.as_str()) {
                    Some(v) => Response::ok(serde_json::json!({ "key": k, "value": v })),
                    None => Response::err(format!("unknown setting '{k}'")),
                },
            };
        }

        GodotCommand::GetAgentInfo => {
            let Ok(snapshot) = status.lock() else {
                return Response::err("status lock poisoned");
            };
            return Response::ok(serde_json::json!({
                "tool_count": snapshot.tool_count,
                "memory_chunks": snapshot.memory_chunks,
                "last_tool": snapshot.last_agent_tool,
                "last_response": snapshot.last_agent_response,
            }));
        }

        _ => {}
    }

    // Everything else is a control request: enqueue it for the main loop.
    match control_request_for(cmd) {
        Some(req) => {
            // No subscribers means nothing will drain the request; report that
            // rather than pretending it was accepted.
            if control.receiver_count() == 0 {
                return Response::Error {
                    message: "Runtime is not accepting control requests (is the app running?)"
                        .to_string(),
                    code: Some(503),
                };
            }
            match control.send(req) {
                Ok(_) => Response::ok(serde_json::json!({ "queued": true })),
                Err(_) => Response::err("control channel closed"),
            }
        }
        None => Response::err("command requires runtime support that is unavailable"),
    }
}

pub struct IpcClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl IpcClient {
    pub async fn connect(addr: &str) -> Result<Self, String> {
        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| format!("Connect failed: {e}"))?;
        Ok(Self {
            stream,
            buf: vec![0u8; 8192],
        })
    }

    pub async fn send_command(&mut self, cmd: &GodotCommand) -> Result<Response, String> {
        let cmd_bytes = serde_json::to_vec(cmd).map_err(|e| e.to_string())?;
        self.stream
            .write_all(&(cmd_bytes.len() as u32).to_be_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stream
            .write_all(&cmd_bytes)
            .await
            .map_err(|e| e.to_string())?;

        let mut header = [0u8; 4];
        self.stream
            .read_exact(&mut header)
            .await
            .map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(header) as usize;
        if len > MAX_FRAME_BYTES {
            return Err(format!("response of {len} bytes exceeds limit"));
        }
        self.buf.resize(len, 0);
        self.stream
            .read_exact(&mut self.buf)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&self.buf).map_err(|e| e.to_string())
    }

    pub async fn ping(&mut self) -> Result<(), String> {
        match self.send_command(&GodotCommand::Ping).await? {
            Response::Pong => Ok(()),
            other => Err(format!("unexpected response: {other:?}")),
        }
    }

    pub async fn send_command_raw(&mut self, cmd_bytes: &[u8]) -> Result<Response, String> {
        self.stream
            .write_all(&(cmd_bytes.len() as u32).to_be_bytes())
            .await
            .map_err(|e| e.to_string())?;
        self.stream
            .write_all(cmd_bytes)
            .await
            .map_err(|e| e.to_string())?;
        let mut header = [0u8; 4];
        self.stream
            .read_exact(&mut header)
            .await
            .map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(header) as usize;
        if len > MAX_FRAME_BYTES {
            return Err(format!("response of {len} bytes exceeds limit"));
        }
        self.buf.resize(len, 0);
        self.stream
            .read_exact(&mut self.buf)
            .await
            .map_err(|e| e.to_string())?;
        serde_json::from_slice(&self.buf).map_err(|e| e.to_string())
    }

    pub async fn disconnect(mut self) {
        let _ = self.stream.shutdown().await;
    }
}

pub async fn start_server() -> Result<(), String> {
    IpcServer::new(IpcConfig::default()).run().await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server() -> IpcServer {
        IpcServer::new(IpcConfig::default())
    }

    #[tokio::test]
    async fn ping_returns_pong() {
        let s = server();
        let resp = process_command(&GodotCommand::Ping, &s.creatures, &s.control, &s.status).await;
        assert!(matches!(resp, Response::Pong));
    }

    #[tokio::test]
    async fn set_then_get_creature_round_trips() {
        let s = server();
        let data = serde_json::json!({ "name": "Test", "personality": "curious" });
        let set = process_command(
            &GodotCommand::SetCreature {
                id: "c1".into(),
                data: data.clone(),
            },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        assert!(matches!(set, Response::Ack));

        let got = process_command(
            &GodotCommand::GetCreature { id: "c1".into() },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match got {
            Response::CreatureData { id, data: d } => {
                assert_eq!(id, "c1");
                assert_eq!(d, data);
            }
            other => panic!("expected CreatureData, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_creature_reports_not_found() {
        let s = server();
        let resp = process_command(
            &GodotCommand::GetCreature { id: "nope".into() },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Error { code, .. } => assert_eq!(code, Some(404)),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_creatures_reflects_stored_creatures() {
        let s = server();
        for id in ["a", "b"] {
            process_command(
                &GodotCommand::SetCreature {
                    id: id.into(),
                    data: serde_json::json!({}),
                },
                &s.creatures,
                &s.control,
                &s.status,
            )
            .await;
        }
        match process_command(
            &GodotCommand::ListCreatures,
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await
        {
            Response::CreatureList { mut creatures } => {
                creatures.sort();
                assert_eq!(creatures, vec!["a".to_string(), "b".to_string()]);
            }
            other => panic!("expected CreatureList, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn export_creature_uses_payload_id() {
        let s = server();
        let resp = process_command(
            &GodotCommand::ExportCreature {
                data: serde_json::json!({ "id": "exported-1" }),
            },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Success { data: Some(d) } => {
                assert_eq!(d["exported"], "exported-1");
            }
            other => panic!("expected Success, got {other:?}"),
        }
        let list = process_command(
            &GodotCommand::ListCreatures,
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        assert!(
            matches!(list, Response::CreatureList { creatures } if creatures.contains(&"exported-1".to_string()))
        );
    }

    #[tokio::test]
    async fn control_commands_are_queued_for_the_main_loop() {
        let s = server();
        // A receiver must exist, standing in for the main loop draining requests.
        let mut rx = s.subscribe_control();

        let resp = process_command(
            &GodotCommand::SendChat {
                message: "hello".into(),
            },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        assert!(!resp.is_error(), "expected acceptance, got {resp:?}");

        match rx.recv().await.expect("control request") {
            ControlRequest::SendChat(msg) => assert_eq!(msg, "hello"),
            other => panic!("expected SendChat, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn control_commands_fail_loudly_when_nothing_is_draining() {
        let s = server();
        // No subscriber: accepting the request would silently drop it.
        let resp = process_command(
            &GodotCommand::Speak { text: "hi".into() },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Error { code, .. } => assert_eq!(code, Some(503)),
            other => panic!("expected Error, got {other:?}"),
        }
    }

    /// A command paired with the predicate its control request must satisfy.
    type MappingCase = (GodotCommand, fn(&ControlRequest) -> bool);

    #[tokio::test]
    async fn every_control_command_maps_to_a_request() {
        let cases: Vec<MappingCase> = vec![
            (
                GodotCommand::SendChat {
                    message: "a".into(),
                },
                |r| matches!(r, ControlRequest::SendChat(m) if m == "a"),
            ),
            (
                GodotCommand::RunAgent { prompt: "b".into() },
                |r| matches!(r, ControlRequest::RunAgent(p) if p == "b"),
            ),
            (
                GodotCommand::SetAnimation {
                    id: Some("c".into()),
                    state: AnimStateCtl::Walk,
                },
                |r| {
                    matches!(
                        r,
                        ControlRequest::SetAnimation {
                            state: AnimStateCtl::Walk,
                            ..
                        }
                    )
                },
            ),
            (
                GodotCommand::SetAnimation {
                    id: None,
                    state: AnimStateCtl::Sleep,
                },
                |r| matches!(r, ControlRequest::SetAnimation { id: None, .. }),
            ),
            (
                GodotCommand::MoveCreature {
                    id: "d".into(),
                    x: 1.0,
                    y: 2.0,
                },
                |r| matches!(r, ControlRequest::MoveCreature { x, y, .. } if *x == 1.0 && *y == 2.0),
            ),
            (
                GodotCommand::SetCreatureVisible {
                    id: "e".into(),
                    visible: false,
                },
                |r| matches!(r, ControlRequest::SetCreatureVisible { visible: false, .. }),
            ),
            (
                GodotCommand::PlaySound {
                    name: "celebrate".into(),
                },
                |r| matches!(r, ControlRequest::PlaySound(n) if n == "celebrate"),
            ),
            (
                GodotCommand::Speak { text: "f".into() },
                |r| matches!(r, ControlRequest::Speak(t) if t == "f"),
            ),
            (
                GodotCommand::SetSetting {
                    key: "volume_master".into(),
                    value: serde_json::json!(0.5),
                },
                |r| matches!(r, ControlRequest::SetSetting { key, .. } if key == "volume_master"),
            ),
            (GodotCommand::SetAutoCycle { enabled: true }, |r| {
                matches!(r, ControlRequest::SetAutoCycle(true))
            }),
            (
                GodotCommand::MemoryIngest {
                    source: "s".into(),
                    content: "c".into(),
                },
                |r| matches!(r, ControlRequest::MemoryIngest { content, .. } if content == "c"),
            ),
        ];

        for (cmd, check) in cases {
            let req = control_request_for(&cmd).unwrap_or_else(|| panic!("no request for {cmd:?}"));
            assert!(check(&req), "unexpected mapping for {cmd:?} -> {req:?}");
        }
    }

    #[tokio::test]
    async fn query_commands_do_not_queue_control_requests() {
        // These must not enqueue work; they are (or will be) answered inline.
        for cmd in [
            GodotCommand::Ping,
            GodotCommand::ListCreatures,
            GodotCommand::GetStatus,
            GodotCommand::GetChatHistory,
        ] {
            assert!(
                control_request_for(&cmd).is_none(),
                "{cmd:?} should not map to a control request"
            );
        }
    }

    #[test]
    fn memory_search_routes_through_the_control_channel() {
        // Retrieval needs the live pipeline, which only the render loop owns,
        // so it is queued rather than answered by the socket handler.
        let cmd = GodotCommand::MemorySearch {
            query: "q".into(),
            limit: None,
        };
        match control_request_for(&cmd) {
            Some(ControlRequest::MemorySearch { query, limit }) => {
                assert_eq!(query, "q");
                assert_eq!(limit, DEFAULT_SEARCH_LIMIT);
            }
            other => panic!("expected MemorySearch, got {other:?}"),
        }
    }

    #[test]
    fn memory_search_limit_is_clamped() {
        let too_many = GodotCommand::MemorySearch {
            query: "q".into(),
            limit: Some(10_000),
        };
        match control_request_for(&too_many) {
            Some(ControlRequest::MemorySearch { limit, .. }) => {
                assert_eq!(limit, MAX_SEARCH_LIMIT);
            }
            other => panic!("expected MemorySearch, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn query_commands_answer_from_the_status_snapshot() {
        let s = server();
        {
            let mut snapshot = s.status.lock().unwrap();
            snapshot.fps = 42.5;
            snapshot.creature_count = 7;
            snapshot
                .settings
                .insert("volume_master".into(), serde_json::json!(0.5));
        }
        let resp = process_command(
            &GodotCommand::GetStatus,
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Success { data: Some(d) } => {
                assert_eq!(d["fps"], 42.5);
                assert_eq!(d["creature_count"], 7);
            }
            other => panic!("expected Success, got {other:?}"),
        }

        let resp = process_command(
            &GodotCommand::GetSetting {
                key: Some("volume_master".into()),
            },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Success { data: Some(d) } => assert_eq!(d["value"], 0.5),
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_results_are_read_from_the_snapshot() {
        let s = server();
        {
            let mut snapshot = s.status.lock().unwrap();
            snapshot.last_query = Some("falcon".into());
            snapshot.search_results =
                serde_json::json!([{ "score": 0.9, "text": "dives fast", "source": "a://1" }]);
        }
        let resp = process_command(
            &GodotCommand::GetSearchResults,
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Success { data: Some(d) } => {
                assert_eq!(d["query"], "falcon");
                assert_eq!(d["results"][0]["text"], "dives fast");
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_results_are_empty_before_any_search() {
        let s = server();
        let resp = process_command(
            &GodotCommand::GetSearchResults,
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        match resp {
            Response::Success { data: Some(d) } => {
                assert!(d["query"].is_null());
                assert!(d["results"].is_null() || d["results"] == serde_json::json!([]));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unknown_setting_reports_an_error() {
        let s = server();
        let resp = process_command(
            &GodotCommand::GetSetting {
                key: Some("nope".into()),
            },
            &s.creatures,
            &s.control,
            &s.status,
        )
        .await;
        assert!(resp.is_error(), "expected error, got {resp:?}");
    }

    #[tokio::test]
    async fn status_snapshot_serialises_the_expected_fields() {
        let s = server();
        let json = RuntimeStatus {
            fps: 30.0,
            frame_count: 100,
            creature_count: 7,
            visible_count: 6,
            auto_cycle: true,
            uptime_secs: 3.5,
            model_count: 4,
            tool_count: 2,
            memory_chunks: 10,
            ..Default::default()
        }
        .to_json();
        assert_eq!(json["fps"], 30.0);
        assert_eq!(json["frames"], 100);
        assert_eq!(json["creature_count"], 7);
        assert_eq!(json["memory_chunks"], 10);
        let _ = &s;
    }

    #[tokio::test]
    async fn frame_round_trip_over_tcp() {
        // Bind an ephemeral port so the test never collides with a running app.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let s = server();
        let creatures = s.creatures.clone();
        let control = s.control.clone();
        let tx = s.tx.clone();
        let status = s.status.clone();
        let shutdown = s.shutdown.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (tx, control, creatures, status, shutdown) = (
                    tx.clone(),
                    control.clone(),
                    creatures.clone(),
                    status.clone(),
                    shutdown.clone(),
                );
                tokio::spawn(handle_client(
                    stream, tx, control, creatures, status, shutdown,
                ));
            }
        });

        let mut client = IpcClient::connect(&addr).await.expect("connect");
        client.ping().await.expect("ping");

        client
            .send_command(&GodotCommand::SetCreature {
                id: "ipc-1".into(),
                data: serde_json::json!({ "name": "Over IPC" }),
            })
            .await
            .expect("set");

        match client
            .send_command(&GodotCommand::GetCreature { id: "ipc-1".into() })
            .await
            .unwrap()
        {
            Response::CreatureData { data, .. } => {
                assert_eq!(data["name"], "Over IPC");
            }
            other => panic!("expected CreatureData, got {other:?}"),
        }

        match client
            .send_command(&GodotCommand::GetCreature {
                id: "absent".into(),
            })
            .await
            .unwrap()
        {
            Response::Error { .. } => {}
            other => panic!("expected Error, got {other:?}"),
        }
        client.disconnect().await;
    }

    #[tokio::test]
    async fn malformed_json_returns_a_parse_error() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let s = server();
        let (tx, control, creatures, status, shutdown) = (
            s.tx.clone(),
            s.control.clone(),
            s.creatures.clone(),
            s.status.clone(),
            s.shutdown.clone(),
        );
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (tx, control, creatures, status, shutdown) = (
                    tx.clone(),
                    control.clone(),
                    creatures.clone(),
                    status.clone(),
                    shutdown.clone(),
                );
                tokio::spawn(handle_client(
                    stream, tx, control, creatures, status, shutdown,
                ));
            }
        });

        let mut client = IpcClient::connect(&addr).await.unwrap();
        let resp = client.send_command_raw(b"{not json").await.unwrap();
        match resp {
            Response::Error { message, .. } => assert!(message.contains("Parse error")),
            other => panic!("expected parse error, got {other:?}"),
        }
        // The connection stays usable after a bad frame.
        client.ping().await.expect("ping after parse error");
    }

    #[test]
    fn anim_state_serialises_in_snake_case() {
        let json = serde_json::to_string(&AnimStateCtl::Celebrate).unwrap();
        assert_eq!(json, "\"celebrate\"");
        let back: AnimStateCtl = serde_json::from_str("\"sleep\"").unwrap();
        assert_eq!(back, AnimStateCtl::Sleep);
        assert_eq!(AnimStateCtl::Walk.as_str(), "walk");
    }

    #[test]
    fn commands_parse_from_tagged_json() {
        let cmd: GodotCommand =
            serde_json::from_str(r#"{"cmd":"send_chat","message":"yo"}"#).unwrap();
        assert!(matches!(cmd, GodotCommand::SendChat { message } if message == "yo"));

        let cmd: GodotCommand =
            serde_json::from_str(r#"{"cmd":"set_animation","id":"a","state":"fly"}"#).unwrap();
        assert!(matches!(
            cmd,
            GodotCommand::SetAnimation {
                state: AnimStateCtl::Fly,
                ..
            }
        ));
    }

    #[test]
    fn response_round_trips() {
        let r = Response::ok(serde_json::json!({ "queued": true }));
        let back: Response = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
        assert!(!back.is_error());
    }
}
