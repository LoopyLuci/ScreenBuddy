/// IPC Bridge for ScreenBuddy
///
/// TCP JSON protocol for Godot editor ↔ ScreenBuddy runtime communication.

use std::collections::HashMap;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, Mutex, Notify};

pub const DEFAULT_PORT: u16 = 34567;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum GodotCommand {
    Ping,
    ListCreatures,
    GetCreature { id: String },
    SetCreature { id: String, data: serde_json::Value },
    ReloadCreature { id: String },
    ExportCreature { data: serde_json::Value },
    SaveConfig { config: serde_json::Value },
    LoadConfig,
    Unknown { raw: String },
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

#[derive(Debug, Clone)]
pub struct IpcConfig {
    pub addr: String,
    pub enable_hot_reload: bool,
}

impl Default for IpcConfig {
    fn default() -> Self {
        Self { addr: format!("127.0.0.1:{}", DEFAULT_PORT), enable_hot_reload: true }
    }
}

pub struct IpcServer {
    config: IpcConfig,
    shutdown: Arc<Notify>,
    tx: broadcast::Sender<GodotCommand>,
    creatures: Arc<Mutex<HashMap<String, serde_json::Value>>>,
}

impl IpcServer {
    pub fn new(config: IpcConfig) -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { config, shutdown: Arc::new(Notify::new()), tx, creatures: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub async fn run(&self) -> Result<(), String> {
        let listener = TcpListener::bind(&self.config.addr).await.map_err(|e| format!("Bind failed: {}", e))?;
        tracing::info!("IPC server listening on {}", self.config.addr);
        loop {
            tokio::select! {
                Ok((stream, _)) = listener.accept() => {
                    let tx = self.tx.clone();
                    let creatures = self.creatures.clone();
                    let shutdown = self.shutdown.clone();
                    tokio::spawn(async move { handle_client(stream, tx, creatures, shutdown).await; });
                }
                _ = self.shutdown.notified() => break,
            }
        }
        Ok(())
    }

    pub fn shutdown(&self) { self.shutdown.notify_waiters(); }
    pub fn subscribe(&self) -> broadcast::Receiver<GodotCommand> { self.tx.subscribe() }
    pub async fn load_creature(&self, id: String, data: serde_json::Value) {
        self.creatures.lock().await.insert(id, data);
    }
}

async fn handle_client(
    mut stream: TcpStream,
    tx: broadcast::Sender<GodotCommand>,
    creatures: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    shutdown: Arc<Notify>,
) {
    let mut buf = vec![0u8; 8192];
    loop {
        tokio::select! {
            Ok(n) = stream.read(&mut buf) => {
                if n == 0 { break; }
                if n < 4 { continue; }
                let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
                if n < 4 + len { continue; }
                match serde_json::from_slice::<GodotCommand>(&buf[4..4+len]) {
                    Ok(cmd) => {
                        let resp = process_command(&cmd, &creatures).await;
                        let resp_bytes = serde_json::to_vec(&resp).unwrap_or_default();
                        let header = (resp_bytes.len() as u32).to_be_bytes();
                        let _ = stream.write_all(&header).await;
                        let _ = stream.write_all(&resp_bytes).await;
                        let _ = tx.send(cmd);
                    }
                    Err(e) => {
                        let resp = Response::Error { message: format!("Parse error: {}", e), code: Some(400) };
                        let resp_bytes = serde_json::to_vec(&resp).unwrap_or_default();
                        let header = (resp_bytes.len() as u32).to_be_bytes();
                        let _ = stream.write_all(&header).await;
                        let _ = stream.write_all(&resp_bytes).await;
                    }
                }
            }
            _ = shutdown.notified() => break,
        }
    }
}

async fn process_command(
    cmd: &GodotCommand,
    creatures: &Arc<Mutex<HashMap<String, serde_json::Value>>>,
) -> Response {
    match cmd {
        GodotCommand::Ping => Response::Pong,
        GodotCommand::ListCreatures => {
            let list = creatures.lock().await.keys().cloned().collect();
            Response::CreatureList { creatures: list }
        }
        GodotCommand::GetCreature { id } => {
            match creatures.lock().await.get(id) {
                Some(data) => Response::CreatureData { id: id.clone(), data: data.clone() },
                None => Response::Error { message: format!("Creature '{}' not found", id), code: Some(404) },
            }
        }
        GodotCommand::SetCreature { id, data } => {
            creatures.lock().await.insert(id.clone(), data.clone());
            Response::Ack
        }
        GodotCommand::ReloadCreature { id: _ } => Response::Ack,
        GodotCommand::ExportCreature { data } => {
            let id = data.get("id").and_then(|v| v.as_str()).unwrap_or("unknown");
            creatures.lock().await.insert(id.to_string(), data.clone());
            Response::Success { data: Some(serde_json::json!({"exported": id})) }
        }
        GodotCommand::SaveConfig { config: _ } => Response::Ack,
        GodotCommand::LoadConfig => Response::Success { data: None },
        GodotCommand::Unknown { raw } => {
            Response::Error { message: format!("Unknown command: {}", raw), code: Some(400) }
        }
    }
}

pub struct IpcClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl IpcClient {
    pub async fn connect(addr: &str) -> Result<Self, String> {
        let stream = TcpStream::connect(addr).await.map_err(|e| format!("Connect failed: {}", e))?;
        Ok(Self { stream, buf: vec![0u8; 8192] })
    }

    pub async fn send_command(&mut self, cmd: &GodotCommand) -> Result<Response, String> {
        let cmd_bytes = serde_json::to_vec(cmd).map_err(|e| e.to_string())?;
        let header = (cmd_bytes.len() as u32).to_be_bytes();
        self.stream.write_all(&header).await.map_err(|e| e.to_string())?;
        self.stream.write_all(&cmd_bytes).await.map_err(|e| e.to_string())?;
        let n = self.stream.read(&mut self.buf).await.map_err(|e| e.to_string())?;
        if n < 4 { return Err("Short read".to_string()); }
        let len = u32::from_be_bytes([self.buf[0], self.buf[1], self.buf[2], self.buf[3]]) as usize;
        if n < 4 + len { return Err("Incomplete response".to_string()); }
        serde_json::from_slice(&self.buf[4..4+len]).map_err(|e| e.to_string())
    }

    pub async fn ping(&mut self) -> Result<(), String> {
        match self.send_command(&GodotCommand::Ping).await? {
            Response::Pong => Ok(()),
            _ => Err("Unexpected response".to_string()),
        }
    }

    pub async fn disconnect(mut self) {
        let _ = self.stream.shutdown().await;
    }
}

pub async fn start_server() -> Result<(), String> {
    let config = IpcConfig::default();
    let server = IpcServer::new(config);
    server.run().await
}
