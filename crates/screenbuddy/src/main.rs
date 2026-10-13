use image::RgbaImage;
use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

mod agent_editor;
mod animation;
mod chat_window;
mod composite_renderer;
mod gdi;
mod main_window;
mod markdown;
mod monitors;
mod settings_window;
mod theme;

use animation::SpriteAnimator;
use chat_window::{ChatWindow, ChatWindowEvent};
use composite_renderer::{CompositeRenderer, RenderableCreature};
use main_window::{create_main_window, MainWindow, UiEvent};
use screenbuddy_core::chat_overlay::MessageRole;
use screenbuddy_core::session::{Role, SessionStore};

const BIRD_IDLE: &[u8] = include_bytes!("../assets/generated/bird_idle.png");
const BIRD_WALK: &[u8] = include_bytes!("../assets/generated/bird_walk.png");
const BIRD_FLY: &[u8] = include_bytes!("../assets/generated/bird_fly.png");
const BIRD_SLEEP: &[u8] = include_bytes!("../assets/generated/bird_sleep.png");
const BIRD_CELEBRATE: &[u8] = include_bytes!("../assets/generated/bird_celebrate.png");

const ROBO_CAT_IDLE: &[u8] = include_bytes!("../assets/generated/robo_cat_idle.png");
const ROBO_CAT_WALK: &[u8] = include_bytes!("../assets/generated/robo_cat_walk.png");
const ROBO_CAT_FLY: &[u8] = include_bytes!("../assets/generated/robo_cat_fly.png");
const ROBO_CAT_SLEEP: &[u8] = include_bytes!("../assets/generated/robo_cat_sleep.png");
const ROBO_CAT_CELEBRATE: &[u8] = include_bytes!("../assets/generated/robo_cat_celebrate.png");

const SLIME_KING_IDLE: &[u8] = include_bytes!("../assets/generated/slime_king_idle.png");
const SLIME_KING_WALK: &[u8] = include_bytes!("../assets/generated/slime_king_walk.png");
const SLIME_KING_FLY: &[u8] = include_bytes!("../assets/generated/slime_king_fly.png");
const SLIME_KING_SLEEP: &[u8] = include_bytes!("../assets/generated/slime_king_sleep.png");
const SLIME_KING_CELEBRATE: &[u8] = include_bytes!("../assets/generated/slime_king_celebrate.png");

const PIXEL_WIZARD_IDLE: &[u8] = include_bytes!("../assets/generated/pixel_wizard_idle.png");
const PIXEL_WIZARD_WALK: &[u8] = include_bytes!("../assets/generated/pixel_wizard_walk.png");
const PIXEL_WIZARD_FLY: &[u8] = include_bytes!("../assets/generated/pixel_wizard_fly.png");
const PIXEL_WIZARD_SLEEP: &[u8] = include_bytes!("../assets/generated/pixel_wizard_sleep.png");
const PIXEL_WIZARD_CELEBRATE: &[u8] =
    include_bytes!("../assets/generated/pixel_wizard_celebrate.png");

const COSMIC_JELLYFISH_IDLE: &[u8] =
    include_bytes!("../assets/generated/cosmic_jellyfish_idle.png");
const COSMIC_JELLYFISH_WALK: &[u8] =
    include_bytes!("../assets/generated/cosmic_jellyfish_walk.png");
const COSMIC_JELLYFISH_FLY: &[u8] = include_bytes!("../assets/generated/cosmic_jellyfish_fly.png");
const COSMIC_JELLYFISH_SLEEP: &[u8] =
    include_bytes!("../assets/generated/cosmic_jellyfish_sleep.png");
const COSMIC_JELLYFISH_CELEBRATE: &[u8] =
    include_bytes!("../assets/generated/cosmic_jellyfish_celebrate.png");

const DRAGON_IDLE: &[u8] = include_bytes!("../assets/generated/dragon_idle.png");
const DRAGON_WALK: &[u8] = include_bytes!("../assets/generated/dragon_walk.png");
const DRAGON_FLY: &[u8] = include_bytes!("../assets/generated/dragon_fly.png");
const DRAGON_SLEEP: &[u8] = include_bytes!("../assets/generated/dragon_sleep.png");
const DRAGON_CELEBRATE: &[u8] = include_bytes!("../assets/generated/dragon_celebrate.png");

const GHOST_IDLE: &[u8] = include_bytes!("../assets/generated/ghost_idle.png");
const GHOST_WALK: &[u8] = include_bytes!("../assets/generated/ghost_walk.png");
const GHOST_FLY: &[u8] = include_bytes!("../assets/generated/ghost_fly.png");
const GHOST_SLEEP: &[u8] = include_bytes!("../assets/generated/ghost_sleep.png");
const GHOST_CELEBRATE: &[u8] = include_bytes!("../assets/generated/ghost_celebrate.png");

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum AnimState {
    Idle,
    Walk,
    Fly,
    Sleep,
    Celebrate,
}

/// Per-frame size and frame count of a sprite sheet.
struct SpriteGeometry {
    frame_size: u32,
    frames: usize,
}

/// The five animation sheets for one creature, in PNG byte form.
struct SpriteSet {
    idle: &'static [u8],
    walk: &'static [u8],
    fly: &'static [u8],
    sleep: &'static [u8],
    celebrate: &'static [u8],
}

#[allow(dead_code)] // `id`/`frame_size` are used once per-pet rendering lands
struct CreatureInstance {
    id: String,
    name: String,
    sheets: HashMap<AnimState, Arc<RgbaImage>>,
    animator: SpriteAnimator,
    physics: screenbuddy_core::CreaturePhysics,
    current_state: AnimState,
    frame_size: u32,
    frames: usize,
    visible: bool,
}

impl CreatureInstance {
    fn from_bytes(
        id: String,
        name: String,
        sprites: &SpriteSet,
        geom: SpriteGeometry,
        x: f32,
        y: f32,
        personality: screenbuddy_core::CreaturePersonality,
    ) -> Result<Self, String> {
        let SpriteGeometry { frame_size, frames } = geom;
        let load = |state: AnimState, bytes: &[u8]| -> Result<Arc<RgbaImage>, String> {
            image::load_from_memory(bytes)
                .map(|img| Arc::new(img.to_rgba8()))
                .map_err(|e| format!("{:?}: {}", state, e))
        };
        let mut sheets = HashMap::new();
        sheets.insert(AnimState::Idle, load(AnimState::Idle, sprites.idle)?);
        sheets.insert(AnimState::Walk, load(AnimState::Walk, sprites.walk)?);
        sheets.insert(AnimState::Fly, load(AnimState::Fly, sprites.fly)?);
        sheets.insert(AnimState::Sleep, load(AnimState::Sleep, sprites.sleep)?);
        sheets.insert(
            AnimState::Celebrate,
            load(AnimState::Celebrate, sprites.celebrate)?,
        );

        let animator = SpriteAnimator::new(frames, 30.0);
        let physics =
            screenbuddy_core::CreaturePhysics::new(x, y, frame_size).with_personality(personality);

        Ok(Self {
            id,
            name,
            sheets,
            animator,
            physics,
            current_state: AnimState::Idle,
            frame_size,
            frames,
            visible: true,
        })
    }

    fn set_state(&mut self, state: AnimState) {
        if self.current_state != state {
            self.current_state = state;
            self.animator.reset();
        }
    }
}

fn main() {
    // Declare DPI awareness so the app renders correctly on high-DPI displays.
    unsafe {
        winapi::um::winuser::SetProcessDPIAware();
    }
    println!("ScreenBuddy starting...");
    let hw = screenbuddy_core::hardware_profile();
    println!("Hardware:\n{}", hw.summary());
    let _pool = screenbuddy_core::ThreadPool::new();
    let config = screenbuddy_core::ConfigManager::new();
    let mut audio = screenbuddy_core::AudioSystem::new();
    audio.load_default_sounds();
    let ai_config = config.get().ai.clone();
    // Shared with the chat bridge: a clone would be a separate engine, and
    // settings applied here would never reach the one that sends requests.
    let ai = std::sync::Arc::new(screenbuddy_core::AiEngine::new(ai_config));
    println!(
        "AI engine: {} models available",
        ai.available_models().len()
    );

    use screenbuddy_core::CreaturePersonality;

    let creature_defs = [
        (
            "companion-bird-01",
            "Companion Bird",
            BIRD_IDLE,
            BIRD_WALK,
            BIRD_FLY,
            BIRD_SLEEP,
            BIRD_CELEBRATE,
            CreaturePersonality::Curious,
        ),
        (
            "robo-cat-01",
            "Robo-Cat",
            ROBO_CAT_IDLE,
            ROBO_CAT_WALK,
            ROBO_CAT_FLY,
            ROBO_CAT_SLEEP,
            ROBO_CAT_CELEBRATE,
            CreaturePersonality::Energetic,
        ),
        (
            "slime-king-01",
            "Slime King",
            SLIME_KING_IDLE,
            SLIME_KING_WALK,
            SLIME_KING_FLY,
            SLIME_KING_SLEEP,
            SLIME_KING_CELEBRATE,
            CreaturePersonality::Lazy,
        ),
        (
            "pixel-wizard-01",
            "Pixel Wizard",
            PIXEL_WIZARD_IDLE,
            PIXEL_WIZARD_WALK,
            PIXEL_WIZARD_FLY,
            PIXEL_WIZARD_SLEEP,
            PIXEL_WIZARD_CELEBRATE,
            CreaturePersonality::Neutral,
        ),
        (
            "cosmic-jellyfish-01",
            "Cosmic Jellyfish",
            COSMIC_JELLYFISH_IDLE,
            COSMIC_JELLYFISH_WALK,
            COSMIC_JELLYFISH_FLY,
            COSMIC_JELLYFISH_SLEEP,
            COSMIC_JELLYFISH_CELEBRATE,
            CreaturePersonality::Shy,
        ),
        (
            "dragon-01",
            "Dragon",
            DRAGON_IDLE,
            DRAGON_WALK,
            DRAGON_FLY,
            DRAGON_SLEEP,
            DRAGON_CELEBRATE,
            CreaturePersonality::Neutral,
        ),
        (
            "ghost-01",
            "Ghost",
            GHOST_IDLE,
            GHOST_WALK,
            GHOST_FLY,
            GHOST_SLEEP,
            GHOST_CELEBRATE,
            CreaturePersonality::Shy,
        ),
    ];

    let mut creatures: HashMap<String, CreatureInstance> = HashMap::new();

    for (i, (id, name, idle, walk, fly, sleep, celebrate, personality)) in
        creature_defs.iter().enumerate()
    {
        let x = 50.0 + (i as f32 * 110.0);
        let y = 100.0 + (i as f32 * 70.0);
        let sprites = SpriteSet {
            idle,
            walk,
            fly,
            sleep,
            celebrate,
        };
        let geom = SpriteGeometry {
            frame_size: 128,
            frames: 4,
        };
        match CreatureInstance::from_bytes(
            id.to_string(),
            name.to_string(),
            &sprites,
            geom,
            x,
            y,
            *personality,
        ) {
            Ok(creature) => {
                println!("Loaded: {} ({})", name, id);
                creatures.insert(id.to_string(), creature);
            }
            Err(e) => eprintln!("Failed {}: {}", id, e),
        }
    }

    if creatures.is_empty() {
        eprintln!("No creatures!");
        std::thread::sleep(Duration::from_secs(5));
        return;
    }
    println!(
        "Loaded {} creatures with 5 animations each",
        creatures.len()
    );

    let (hwnd_tx, hwnd_rx) = mpsc::channel::<isize>();
    thread::spawn(move || {
        create_window_and_run(hwnd_tx, 800, 600);
    });

    let hwnd = match hwnd_rx.recv() {
        Ok(h) => h,
        Err(_) => {
            eprintln!("Window failed");
            return;
        }
    };

    let mut renderer = CompositeRenderer::new(hwnd as *mut winapi::ctypes::c_void, 800, 600);
    let start_time = Instant::now();

    let mut tray = screenbuddy_core::SystemTray::new();
    match tray.start() {
        Ok(()) => println!("[Tray] icon installed"),
        Err(e) => println!("[Tray] unavailable: {e}"),
    }
    let chat = screenbuddy_core::ChatOverlay::new();
    chat.start().ok();
    let screen = screenbuddy_core::ScreenManager::new();
    audio.play("celebration").ok();

    let mut active_idx: usize = 0;
    let creature_ids: Vec<String> = creatures.keys().cloned().collect();
    let states = [
        AnimState::Idle,
        AnimState::Walk,
        AnimState::Fly,
        AnimState::Sleep,
        AnimState::Celebrate,
    ];
    let mut state_idx: usize = 0;

    // AI Chat - with console input
    // The same engine the settings are applied to below, not a second one built
    // here. Two engines meant the pinned model, the provider filter and the
    // restored model list all reached an engine that never sent a request,
    // while the bridge's own engine kept the hardcoded catalogue and used one
    // of its names whether or not it was installed.
    let chat_clone = screenbuddy_core::ChatOverlay::new();
    let _ai_bridge = screenbuddy_core::AiChatBridge::with_engine(ai.clone(), chat_clone);
    _ai_bridge.start().ok();

    // Spawn stdin reader for chat input
    let (input_tx, input_rx) = std::sync::mpsc::channel::<String>();
    let input_tx2 = input_tx.clone();
    // Only prompt when attached to an interactive terminal: stdout is also the
    // MCP/stdio channel, and stray prompts would corrupt that protocol stream.
    let interactive = {
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            const FILE_TYPE_CHAR: u32 = 2;
            unsafe {
                winapi::um::fileapi::GetFileType(std::io::stdin().as_raw_handle() as *mut winapi::ctypes::c_void) == FILE_TYPE_CHAR
            }
        }
        #[cfg(not(windows))]
        {
            false
        }
    };

    thread::spawn(move || {
        use std::io::{self, Write};
        let prompt = |io: &mut io::Stdout| {
            if interactive {
                print!("> ");
                io.flush().ok();
            }
        };
        prompt(&mut io::stdout());
        let mut line = String::new();
        loop {
            line.clear();
            if io::stdin().read_line(&mut line).is_err() {
                break;
            }
            let trimmed = line.trim().to_string();
            // read_line returns Ok(0) at EOF, which is not an error but never
            // yields a line either. Without this the loop spun forever and
            // printed the prompt continuously, flooding any captured log.
            if trimmed.is_empty() && line.is_empty() {
                break;
            }
            if !trimmed.is_empty() {
                if trimmed == "/quit" {
                    break;
                }
                input_tx2.send(trimmed).ok();
            }
            prompt(&mut io::stdout());
        }
    });

    // Clone bridge for the input handler
    let bridge_for_input = _ai_bridge.clone();

    // Mouse interaction
    let cursor_pos: Option<glam::Vec2> = None;
    let dragged_creature: Option<usize> = None;

    // Animation speed multiplier, read from settings each frame
    let mut anim_speed: f32 = 1.0;

    // RAG Pipeline - memory system. Restored from disk so ingested documents
    // survive a restart; the startup blurb is only added to a fresh index, or it
    // would be duplicated on every launch.
    let mut rag = screenbuddy_core::RagPipeline::load(rag_index_path());
    if rag.chunk_count() == 0 {
        let _ = rag.ingest_document(
            std::path::Path::new("memory://startup"),
            "ScreenBuddy is an AI desktop companion. It has 7 creatures with unique personalities. The user can interact with them via click, drag, and chat."
        );
        let _ = rag.save(rag_index_path());
    }
    println!(
        "[RAG] Memory system initialized with {} chunk(s)",
        rag.chunk_count()
    );

    // Agent Runtime - agentic capabilities. The default tools (echo, time) make
    // the runtime immediately useful instead of reporting zero tools.
    // The store is created first because restoring the active agent needs it.
    // Agent editor, backed by the persistent profile store.
    let agent_store = screenbuddy_core::agent_profile::agent_store();
    let mut agent_win = agent_editor::AgentEditor::new(agent_store, None);
    agent_editor::create_editor_window(&mut agent_win);

    // Share the one engine, as the chat bridge does. Wrapping ai again here
    // built a second engine whose settings were never applied.
    let agent = std::sync::Arc::new(screenbuddy_core::AgentRuntime::new(ai.clone()));
    for tool in screenbuddy_core::default_tools() {
        agent.register_tool(&tool.name.clone(), tool);
    }
    let mut agent_rx = agent.subscribe();
    println!(
        "[Agent] Runtime initialized with {} tools",
        agent.tool_count()
    );

    // Restore the agent the user had in effect last time. Without this the choice
    // is silently forgotten on restart, which makes activating one pointless.
    if let Some(profile) = agent_store.active_id().and_then(|id| agent_store.get(&id)) {
        agent.apply_profile(&profile);
        println!("[Agent] Restored active agent '{}'", profile.name);
    }

    // ---- IPC control surface -------------------------------------------------
    // Lets an external agent (e.g. Hermes through the bundled MCP server) drive
    // this running instance over TCP. Disabled when the port cannot be bound, so
    // a second instance degrades gracefully instead of failing to start.
    let ipc_config = screenbuddy_core::IpcConfig {
        addr: std::env::var("SCREENBUDDY_IPC_ADDR")
            .unwrap_or_else(|_| format!("127.0.0.1:{}", screenbuddy_core::DEFAULT_PORT)),
        enable_hot_reload: true,
        max_connections: 32,
    };
    let mut control_rx: Option<tokio::sync::broadcast::Receiver<screenbuddy_core::ControlRequest>> =
        None;
    // Creatures pinned by an external `move_creature` call. Physics still
    // integrates them, so the position is re-asserted after each update.
    let mut pinned: HashMap<String, glam::Vec2> = HashMap::new();

    // Shared snapshot the IPC server answers queries from. Both sides must
    // reference this exact handle or queries return defaults forever.
    let status = std::sync::Arc::new(std::sync::Mutex::new(
        screenbuddy_core::RuntimeStatus::default(),
    ));
    let mut auto_cycle = true;
    let mut tts: Option<screenbuddy_core::TtsSystem> = None;

    // Wrapped in a shared handle so the IPC server and MCP tools operate on the
    // same conversations the GUI shows. A window renders a view of a session; it
    // does not own the conversation.
    let shared_sessions = Arc::new(std::sync::Mutex::new(SessionStore::load(sessions_path())));
    // Save at most once a frame rather than on every message: writing the whole
    // store at the caps measures ~9ms, over half a 60fps frame budget.
    if let Ok(mut store) = shared_sessions.lock() {
        store.set_deferred(true);
    }
    println!(
        "[Session] {} session(s) loaded",
        shared_sessions.lock().map(|s| s.list().len()).unwrap_or(0)
    );

    // Settings UI
    //
    // Declared before the IPC server is built so the server can share the same
    // store for export/import. Behind an Arc<Mutex> for that sharing; the
    // allocation is stable for the Arc's lifetime, which matters because the
    // settings window holds a raw pointer into this object and it must not move.
    let settings = Arc::new(std::sync::Mutex::new(screenbuddy_core::SettingsUI::new()));
    // Load persisted settings before any read; without this, fps_target is
    // always the default (30) at startup because the saved value is never read.
    settings.lock().unwrap().load();

    // Held outside the IPC block so the settings window can use it after.
    let mut settings_control: Option<
        tokio::sync::broadcast::Sender<screenbuddy_core::ControlRequest>,
    > = None;

    if std::env::var("SCREENBUDDY_DISABLE_IPC").is_err() {
        // `main` is not inside a Tokio runtime, so the server gets its own and
        // runs on a dedicated OS thread. This mirrors AiChatBridge.
        // The server and this loop must share one snapshot handle; two copies
        // would make every query return defaults.
        //
        // The settings store is shared too, so a profile imported over IPC is
        // live in the render loop immediately rather than on next launch.
        let mut ipc = screenbuddy_core::IpcServer::with_status(ipc_config.clone(), status.clone())
            .with_sessions(shared_sessions.clone());
        ipc.set_settings(settings.clone());
        // Subscribe before starting the thread so a request arriving
        // immediately after startup is not missed.
        control_rx = Some(ipc.subscribe_control());
        // Taken before `ipc` is moved into its thread: the settings window needs
        // a way to ask the render loop to export or import.
        let sender = ipc.control_sender();
        settings_control = Some(sender);
        let ipc_addr = ipc_config.addr.clone();
        thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    eprintln!("[IPC] Could not create runtime: {e}");
                    return;
                }
            };
            if let Err(e) = runtime.block_on(ipc.run()) {
                eprintln!("[IPC] Server stopped: {e}");
            }
        });
        println!("[IPC] Control server listening on {}", ipc_addr);
    } else {
        println!("[IPC] Control server disabled (SCREENBUDDY_DISABLE_IPC set)");
    }

    // Install the logging subscriber. Until now the tracing calls in this crate
    // went nowhere, because nothing ever installed one.
    let log_level = settings
        .lock()
        .unwrap()
        .text("log_level")
        .unwrap_or_else(|| "info".to_string());
    let log_path = screenbuddy_core::logging::default_log_path();
    match screenbuddy_core::logging::init(&log_level, Some(&log_path)) {
        Ok(()) => println!(
            "[Log] level={log_level} file={path}",
            log_level = log_level,
            path = log_path.display()
        ),
        Err(e) => eprintln!("[Log] could not start logging: {e}"),
    }

    // fps_target was a setting nothing read; the loop was pinned at 30. Clamped so
    // a zero or absurd value cannot stall or melt the CPU.
    let read_target_fps = |settings: &screenbuddy_core::SettingsReader| -> u32 {
        // Clamped so a zero or absurd value cannot stall or melt the CPU.
        settings
            .number("fps_target")
            .map(|v| v as i32)
            .unwrap_or(30)
            .clamp(5, 120) as u32
    };
    let read_theme = |settings: &screenbuddy_core::SettingsReader| -> String {
        settings
            .text("theme")
            .unwrap_or_else(|| "dark".to_string())
    };

    // Shareable read handles. Cloned before the settings window takes a mutable
    // borrow of the store, since that borrow lasts as long as the window does.
    let settings_reader = settings.lock().unwrap().reader();
    let ipc_settings_reader = settings_reader.clone();
    // A third handle for the render loop: settings_reader itself is moved into
    // the settings window below.
    let theme_settings_reader = settings_reader.clone();

    let target_fps = read_target_fps(&settings_reader);
    println!("Animation at {target_fps} FPS");

    // The theme setting was declared but nothing read it, so the window always
    // used dark. Read it through the same reader the loop uses and apply it when
    // it changes.
    let mut applied_theme = read_theme(&theme_settings_reader);
    {
        let mut s = settings.lock().unwrap();
        s.set(
            "creature_count",
            screenbuddy_core::SettingValue::Int(creatures.len() as i64),
        )
        .ok();
        s.set(
            "collision_avoidance",
            screenbuddy_core::SettingValue::Bool(1.0),
        )
        .ok();
        s.set(
            "cursor_interaction",
            screenbuddy_core::SettingValue::Bool(1.0),
        )
        .ok();
        println!("[Settings] {} settings loaded", s.export().len());
        // Seed the snapshot from the loaded settings so reads work before any
        // write. Merged, so control-inserted keys are not wiped.
        if let Ok(mut snapshot) = status.lock() {
            for (name, value) in s.export() {
                snapshot
                    .settings
                    .insert(name, serde_json::json!(setting_to_json(&value)));
            }
        }
    }

    // Apply the AI settings the engine was built before settings were loaded.
    // These report what they did rather than failing silently.
    {
        if let Some(screenbuddy_core::SettingValue::String(provider)) = settings.lock().unwrap().get("ai_provider")
        {
            match screenbuddy_core::AiEngine::parse_backend(provider.as_str()) {
                Ok(backend) => {
                    let kept = ai.restrict_to_backend(backend);
                    println!("[AI] provider {provider:?}: {kept} model(s) selectable");
                }
                Err(e) => println!("[AI] {e}; keeping the default model list"),
            }
        }
        if let Some(screenbuddy_core::SettingValue::String(model)) = settings.lock().unwrap().get("ai_model") {
            // An unknown model must say so: silently using a different one is
            // exactly the defect this audit turned up.
            match ai.set_default_model(model.as_str()) {
                Ok(_) => println!("[AI] pinned model {model:?}"),
                Err(e) => println!("[AI] {e}"),
            }
        }
    }

    // Apply initial settings to audio
    if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.lock().unwrap().get("volume_master") {
        audio.set_master_volume(vol as f32);
    }
    // The two bus levels had no path from settings to playback.
    if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.lock().unwrap().get("volume_effects") {
        audio.set_effects_volume(vol as f32);
    }
    if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.lock().unwrap().get("volume_music") {
        audio.set_music_volume(vol as f32);
    }

    // Chat Window
    // Chat sessions persist across restarts; the window renders a view of the
    // current session rather than owning the conversation.
    // The main window is the place a person actually talks to the app. The old
    // chat overlay remains for the tray/menu path, but this is the real UI.
    let mut main_ui = MainWindow::new();
    let main_hwnd = create_main_window(&mut main_ui);
    match main_hwnd {
        Ok(_) => {
            // Honour the saved theme from the first frame.
            main_ui.set_theme_by_name(&applied_theme);
            main_ui.show();
        }
        Err(e) => eprintln!("[UI] could not create main window: {e}"),
    }

    let mut chat_win = ChatWindow::new();
    let initial_history: Vec<(String, String)> = shared_sessions
        .lock()
        .ok()
        .and_then(|s| s.current().map(|c| c.transcript()))
        .unwrap_or_default();
    for (role, text) in initial_history {
        chat_win.add_message(&role, &text);
    }
    let chat_hwnd = chat_window::create_chat_window(&mut chat_win).unwrap_or(0);
    if chat_hwnd != 0 {
        println!("[Chat] Window created: 0x{:x}", chat_hwnd);
    }

    // Settings Window
    // The settings window borrows the store the render loop owns, so it is
    // created after it and given a raw pointer plus a reader handle.
    // The window borrows the store the render loop owns. The reader was cloned
    // above, before this mutable borrow begins.
    let mut settings_win =
        Box::new(unsafe {
            settings_window::SettingsView::new(settings_reader, &mut *settings.lock().unwrap())
        });
    unsafe {
        settings_window::create_settings_window(settings_win.as_mut());
    }
    // The window asks the render loop to export or import; only the loop owns
    // the store, so the window cannot do the file I/O itself.
    if let Some(sender) = settings_control.as_ref() {
        settings_win.set_control(sender.clone());
    }
    // Created hidden: the tray menu opens it on request, so it should not
    // appear unasked at startup.
    if let Some(hwnd) = settings_win.hwnd() {
        println!("[Settings] Window created: 0x{:x}", hwnd as usize);
        settings_win.hide();
    }

    let mut last_frame = Instant::now();
    // Raise the timer resolution for this process. Windows' default is ~15.6ms,
    // so a request to sleep 11ms actually sleeps ~15.6ms and a 13ms frame lands
    // at 17ms. 1ms resolution makes the sleep honour what it was asked for.
    unsafe {
        winapi::um::timeapi::timeBeginPeriod(1);
    }
    // Declared outside the loop so the message pump can set it before the tray
    // section reads it.
    let mut should_quit = false;
    // Throttles the deferred session write; see the flush in the loop body.
    let mut last_session_flush = Instant::now();
    // Last query the cross-session search was run for, so it runs on change.
    let mut last_search_query = String::new();
    // Rolling frame-time window, published as percentiles for `get_metrics`.
    let mut frame_metrics = screenbuddy_core::metrics::FrameMetrics::new(600);
    // Length of the AI overlay transcript at the last sync, so the per-frame
    // comparison does not clone the whole transcript every frame.
    let mut last_overlay_len = 0usize;
    // Cheap signature of the session store, so the window only re-reads it when
    // something actually changed.
    let mut last_session_sig: (usize, usize, usize) = (0, 0, 0);
    // `publish_status` clones the transcript and creature map; it is for IPC
    // polling, so a few times a second is plenty.
    let mut last_status_publish = Instant::now() - Duration::from_secs(1);
    loop {
        // Pump this thread's message queue. Without this the main window never
        // receives WM_PAINT, WM_KEYDOWN, WM_CHAR, WM_LBUTTONDOWN or
        // WM_MOUSEWHEEL: the window renders nothing and ignores every click and
        // keystroke, because its window proc is only ever called by the
        // dispatcher. PeekMessage (not GetMessage) keeps the loop non-blocking.
        {
            let mut msg: winapi::um::winuser::MSG = unsafe { mem::zeroed() };
            while unsafe {
                winapi::um::winuser::PeekMessageW(
                    &mut msg,
                    ptr::null_mut(),
                    0,
                    0,
                    winapi::um::winuser::PM_REMOVE,
                )
            } != 0
            {
                if msg.message == winapi::um::winuser::WM_QUIT {
                    should_quit = true;
                }
                unsafe {
                    winapi::um::winuser::TranslateMessage(&msg);
                    winapi::um::winuser::DispatchMessageW(&msg);
                }
            }
            if should_quit {
                break;
            }
        }

        let elapsed = start_time.elapsed();
        let dt = last_frame.elapsed().as_secs_f64().clamp(0.0, 0.1);
        last_frame = Instant::now();
        // Start of this frame, used as the pacing deadline's origin.
        let frame_start = Instant::now();
        // Record the real frame interval so stutter shows up in the percentiles.
        frame_metrics.record(dt * 1000.0);

        // Forward AI responses to chat window.
        //
        // The whole transcript used to be cloned every frame, twice, to decide
        // whether anything was new. Both clones are now taken only when the
        // overlay has actually grown, which is what made a 13ms frame budget
        // take 40ms.
        {
            let overlay_len = _ai_bridge
                .overlay()
                .lock()
                .map(|o| o.messages().len())
                .unwrap_or(0);
            if overlay_len != last_overlay_len {
                let ai_messages = _ai_bridge.overlay().lock().unwrap().messages().to_vec();
                let chat_messages = chat_win.messages().to_vec();
                for msg in &ai_messages {
                    if chat_messages.iter().all(|(r, t, _)| {
                        !(r.contains(&format!("{:?}", msg.role).to_lowercase())
                            && t == &msg.content)
                    }) && (msg.role == screenbuddy_core::ChatMsgRole::Assistant
                        || msg.role == screenbuddy_core::ChatMsgRole::User)
                    {
                        let role_str = format!("{:?}", msg.role).to_lowercase();
                        chat_win.add_message(&role_str, &msg.content);
                    }
                }
                last_overlay_len = overlay_len;
            }
        }

        // State machine. Skipped while auto-cycle is off so an external
        // controller can hold a specific animation state.
        let state_elapsed = (elapsed.as_secs() % 60) / 5;
        let new_state_idx = (state_elapsed as usize) % states.len();
        if auto_cycle && new_state_idx != state_idx {
            state_idx = new_state_idx;
            for creature in creatures.values_mut() {
                creature.set_state(states[state_idx]);
            }
            println!("State: {:?}", states[state_idx]);
            match states[state_idx] {
                AnimState::Idle => {
                    audio.play("idle_hum").ok();
                }
                AnimState::Walk => {
                    audio.play("walk_step").ok();
                }
                AnimState::Fly => {
                    audio.play("celebrate").ok();
                }
                AnimState::Sleep => {
                    audio.play("snore").ok();
                }
                AnimState::Celebrate => {
                    audio.play("celebrate").ok();
                }
            }
        }

        // Apply control requests before physics: physics integrates velocity
        // every frame, so a position set after it would be immediately drifted.
        apply_control_requests(
            &mut control_rx,
            &mut creatures,
            &bridge_for_input,
            &mut rag,
            &mut audio,
            &mut settings.lock().unwrap(),
            settings_win.as_mut(),
            &mut tts,
            &mut auto_cycle,
            &status,
            &mut pinned,
            agent_store,
            &mut agent_win,
            agent.as_ref(),
            ipc_settings_reader.clone(),
        );

        // Collect positions for collision avoidance
        let all_positions: Vec<glam::Vec2> =
            creatures.values().map(|c| c.physics.position).collect();

        // Frame skipping: if running below 20fps, skip physics but still render
        let skip_physics = dt > 0.05;
        if !skip_physics {
            for (i, (id, creature)) in creatures.iter_mut().enumerate() {
                let cursor = if dragged_creature == Some(i) {
                    None
                } else {
                    cursor_pos
                };
                creature.physics.update(dt as f32, &screen, cursor, &all_positions);
                if dragged_creature == Some(i) {
                    if let Some(ref pos) = cursor_pos {
                        creature.physics.position = *pos;
                    }
                }
                if let Some(target) = pinned.get(id) {
                    creature.physics.position = *target;
                }
                creature.animator.update(elapsed.as_secs_f32() * anim_speed);
            }
        }

        // Render stats overlay (drawn on screen in main window)
        if settings.lock().unwrap().get("show_render_stats").and_then(|v| match v {
            screenbuddy_core::SettingValue::Bool(b) => Some(b != 0.0),
            _ => None,
        }).unwrap_or(false) {
            let fps = if dt > 0.0 { 1.0 / dt } else { 0.0 };
            let stats = format!("FPS: {:.1} | creatures: {} | visible: {}",
                fps, creatures.len(),
                creatures.values().filter(|c| c.visible).count());
            // Forward to main window for on-screen rendering
            main_ui.set_stats_overlay(&stats);
        }

        // Render
        let mut renderables: Vec<RenderableCreature> = Vec::new();
        let mut frame_indices: Vec<usize> = Vec::new();

        for creature in creatures.values() {
            if !creature.visible {
                continue;
            }
            let sheet = creature
                .sheets
                .get(&creature.current_state)
                .unwrap_or_else(|| creature.sheets.get(&AnimState::Idle).unwrap());
            renderables.push(RenderableCreature {
                sprite_sheet: sheet.clone(),
                frame_count: creature.frames,
                x: creature.physics.position.x,
                y: creature.physics.position.y,
                scale: 1.0,
            });
            frame_indices.push(creature.animator.current_frame());
        }

        renderer.present(&renderables, &frame_indices);

        // Process per-pet window events
        // Process agent events
        if let Ok(event) = agent_rx.try_recv() {
            match event {
                screenbuddy_core::AgentEvent::ToolUse { name, input } => {
                    println!("[Agent] Tool called: {} {:?}", name, input);
                }
                screenbuddy_core::AgentEvent::Message(text) => {
                    println!("[Agent] Response: {}", text);
                    screenbuddy_core::notify("Agent", &text);
                }
                _ => {}
            }
        }

        // Process chat input
        if let Ok(input) = input_rx.try_recv() {
            println!("[Chat] You: {}", input);
            bridge_for_input.send_message(&input);
        }

        // Check for settings changes
        if settings.lock().unwrap().changed() {
            if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.lock().unwrap().get("volume_master")
            {
                audio.set_master_volume(vol as f32);
            }
        }
            anim_speed = settings
              .lock()
              .unwrap()
              .get("animation_speed")
                .and_then(|v| match v {
                    screenbuddy_core::SettingValue::Float(f) => Some(f as f32),
                    _ => None,
                })
                .unwrap_or(1.0);

        // Tray events. The tray window runs on its own thread and posts through
        // a process-wide slot, so drain everything it queued rather than taking
        // one per frame and dropping the rest.
        let tray_events: Vec<_> = screenbuddy_core::system_tray::take_pending_events()
            .into_iter()
            .chain(std::iter::from_fn(|| tray.poll_event()))
            .collect();
        should_quit = false;
        for event in tray_events {
            match event {
                screenbuddy_core::TrayEvent::OpenWindow => {
                    main_ui.show();
                    tray.set_visible(true);
                    println!("[Tray] window shown");
                }
                screenbuddy_core::TrayEvent::MinimizeToTray => {
                    // Only hide when there is an icon to restore from, otherwise
                    // the app would vanish with no way back.
                    if tray.is_installed() {
                        main_ui.hide();
                        tray.set_visible(false);
                        println!("[Tray] minimised to tray");
                    } else {
                        println!("[Tray] no tray icon; keeping the window visible");
                    }
                }
                screenbuddy_core::TrayEvent::Quit => {
                    tray.shutdown();
                    should_quit = true;
                }
                screenbuddy_core::TrayEvent::NextCreature => {
                    active_idx = (active_idx + 1) % creature_ids.len();
                    if let Some(id) = creature_ids.get(active_idx) {
                        if let Some(c) = creatures.get(id) {
                            println!("Active: {}", c.name);
                        }
                    }
                    audio.play("click").ok();
                }
                screenbuddy_core::TrayEvent::ShowCreature => {
                    if let Some(id) = creature_ids.get(active_idx) {
                        if let Some(c) = creatures.get_mut(id) {
                            c.visible = true;
                            println!("Show: {}", c.name);
                        }
                    }
                }
                screenbuddy_core::TrayEvent::HideCreature => {
                    if let Some(id) = creature_ids.get(active_idx) {
                        if let Some(c) = creatures.get_mut(id) {
                            c.visible = false;
                            println!("Hide: {}", c.name);
                        }
                    }
                }
                screenbuddy_core::TrayEvent::OpenChat => {
                    println!("[Chat] Toggling chat window");
                    chat_win.toggle();
                }
                screenbuddy_core::TrayEvent::OpenAgents => {
                    agent_editor::show_editor(&mut agent_win);
                    println!("[Agents] Editor opened");
                }
                screenbuddy_core::TrayEvent::OpenSettings => {
                    println!("[Settings] Opening settings window");
                    if settings_win.is_visible() {
                        settings_win.hide();
                    } else {
                        // Reuses the create-or-show path, so a window closed with
                        // the X button comes back.
                        unsafe {
                            settings_window::show_settings_window(settings_win.as_mut());
                        }
                    }
                }
                screenbuddy_core::TrayEvent::About => {
                    show_about_dialog();
                }
                screenbuddy_core::TrayEvent::AlwaysOnTop => {
                    let hwnd = main_ui.hwnd().unwrap_or(std::ptr::null_mut());
                    if hwnd.is_null() {
                        println!("[Tray] no main window to set always-on-top on");
                    } else {
                        let new_state = !screenbuddy_core::always_on_top();
                        screenbuddy_core::set_always_on_top(new_state);
                        unsafe {
                            let flags = winapi::um::winuser::SWP_NOMOVE
                                | winapi::um::winuser::SWP_NOSIZE
                                | winapi::um::winuser::SWP_NOACTIVATE
                                | winapi::um::winuser::SWP_NOOWNERZORDER;
                            let z_order = if new_state {
                                winapi::um::winuser::HWND_TOPMOST
                            } else {
                                winapi::um::winuser::HWND_NOTOPMOST
                            };
                            winapi::um::winuser::SetWindowPos(
                                hwnd,
                                z_order,
                                0,
                                0,
                                0,
                                0,
                                flags,
                            );
                        }
                        tray.set_always_on_top(new_state);
                        println!("[Tray] always-on-top: {}", new_state);
                    }
                }
            }
        }

        // Drain the agent editor. Saves and deletes are applied here because the
        // store is the app's, not the window's -- so the same agents are visible
        // to IPC and MCP afterwards.
        // Keep the settings window current: it re-reads values and repaints, so a
        // change made over IPC appears while the window is open.
        if settings_win.is_visible() {
            settings_win.sync();
            settings_win.refresh();
        }

        for event in agent_win.take_events() {
            match event {
                agent_editor::EditorEvent::Saved(profile) => {
                    match agent_store.save(profile.clone()) {
                        Ok(saved) => println!("[Agents] Saved '{}' ({})", saved.name, saved.id),
                        Err(e) => eprintln!("[Agents] Save failed: {e}"),
                    }
                    agent_win.reload(agent_store);
                }
                agent_editor::EditorEvent::Activate(profile) => {
                    // Activating from the GUI is deliberately separate from saving:
                    // the draft applies as it stands, so a change can be tried
                    // before committing it.
                    agent.apply_profile(&profile);
                    if let Err(e) = agent_store.set_active(&profile.id) {
                        eprintln!("[Agents] Could not persist activation: {e}");
                    }
                    println!("[Agents] Activated '{}' from the editor", profile.name);
                }
                agent_editor::EditorEvent::Deleted(id) => {
                    match agent_store.delete(&id) {
                        Ok(true) => println!("[Agents] Deleted '{id}'"),
                        Ok(false) => println!("[Agents] '{id}' is a preset and was kept"),
                        Err(e) => eprintln!("[Agents] Delete failed: {e}"),
                    }
                    agent_win.reload(agent_store);
                }
                agent_editor::EditorEvent::Activated(id) => {
                    // "Duplicate" on a preset: make an editable copy and open it.
                    match agent_store.duplicate(&id, None) {
                        Ok(copy) => {
                            println!("[Agents] Duplicated '{}' as '{}'", id, copy.name);
                            agent_win.reload(agent_store);
                            agent_win.select(&copy.id);
                        }
                        Err(e) => eprintln!("[Agents] Duplicate failed: {e}"),
                    }
                }
                agent_editor::EditorEvent::Closed => agent_editor::hide_editor(&mut agent_win),
            }
        }
        agent_win.refresh();

        // Drive the main window: apply its events, then re-read the store so
        // anything an agent changed over IPC shows up.
        while let Some(event) = main_ui.poll_event() {
            match event {
                UiEvent::SendMessage(text) => {
                    println!("[Chat] You: {}", text);
                    if let Ok(mut store) = shared_sessions.lock() {
                        store.push(Role::User, text.clone());
                    }
                    bridge_for_input.send_message(&text);
                    main_ui.set_generating(true);
                    // Audible confirmation that the message went out.
                    audio.play("click").ok();
                }
                UiEvent::NewSession => {
                    if let Ok(mut store) = shared_sessions.lock() {
                        store.new_session("New chat");
                    }
                }
                UiEvent::SelectSession(id) => {
                    if let Ok(mut store) = shared_sessions.lock() {
                        if let Err(e) = store.select(&id) {
                            eprintln!("[UI] {e}");
                        }
                    }
                }
                UiEvent::DeleteSession(id) => {
                    if let Ok(mut store) = shared_sessions.lock() {
                        if let Err(e) = store.delete(&id) {
                            eprintln!("[UI] {e}");
                        }
                    }
                }
                UiEvent::Minimise => {
                    // The window's minimise button; the tray icon keeps the app
                    // reachable, so hiding is the right resting place.
                    main_ui.hide();
                    tray.set_visible(false);
                }
                UiEvent::Closed => {
                    main_ui.hide();
                    tray.set_visible(false);
                }
                UiEvent::ClearSession => {
                    if let Ok(mut store) = shared_sessions.lock() {
                        store.clear_current();
                    }
                }
                UiEvent::Quit => {
                    println!("[UI] quit requested");
                    std::process::exit(0);
                }
                UiEvent::StopGenerating => {
                    // Cancellation is best-effort: the bridge has no cancel
                    // channel, so clear the generating flag and note it.
                    println!("[Chat] Stop requested");
                    main_ui.set_generating(false);
                }
                UiEvent::ExportConversation => {
                    match main_ui.export_conversation() {
                        Ok(path) => println!("[Chat] Exported conversation to {path}"),
                        Err(e) => eprintln!("[Chat] Export failed: {e}"),
                    }
                }
                UiEvent::ToggleSearch => {
                    main_ui.toggle_search();
                }
                UiEvent::EditMessage { index, text } => {
                    // Replace the message and drop what followed it, then ask
                    // again from the corrected prompt.
                    let replaced = shared_sessions
                        .lock()
                        .map(|mut store| store.replace_from(index, text.clone()))
                        .unwrap_or(false);
                    if replaced {
                        println!("[Chat] Edited message {index}: {text}");
                        bridge_for_input.send_message(&text);
                        main_ui.set_generating(true);
                    } else {
                        eprintln!("[Chat] Could not edit message {index}");
                    }
                }
                UiEvent::Regenerate => {
                    // Re-send the most recent user message, which is what
                    // "regenerate" means in a chat: ask again from the same
                    // prompt. Nothing is removed, so the previous reply stays in
                    // the transcript for comparison.
                    let last_user = shared_sessions.lock().ok().and_then(|store| {
                        store.current().and_then(|s| {
                            s.messages
                                .iter()
                                .rev()
                                .find(|m| m.role == Role::User)
                                .map(|m| m.text.clone())
                        })
                    });
                    match last_user {
                        Some(prompt) => {
                            println!("[Chat] Regenerating from: {}", prompt);
                            bridge_for_input.send_message(&prompt);
                            main_ui.set_generating(true);
                        }
                        None => println!("[Chat] Nothing to regenerate"),
                    }
                }
            }
        }

        // Keep the window's view in step with the store and the AI.
        //
        // `refresh_from` clones the session list and the whole transcript, so it
        // is done only when a cheap signature of the store changes rather than
        // on every frame.
        if let Ok(store) = shared_sessions.lock() {
            let sig = (
                store.list().len(),
                store.current_id().len(),
                store.current().map(|s| s.messages.len()).unwrap_or(0),
            );
            if sig != last_session_sig {
                main_ui.refresh_from(&store);
                last_session_sig = sig;
            }
        }

        // Apply the theme setting when it changes. It was declared but read by
        // nothing, so the window was always dark whatever the user chose.
        let want_theme = theme_settings_reader
            .text("theme")
            .unwrap_or_else(|| "dark".to_string());
        if want_theme != applied_theme {
            main_ui.set_theme_by_name(&want_theme);
            println!("[UI] theme -> {want_theme}");
            applied_theme = want_theme;
        }

        // Feed cross-session search results while a query is active. Recomputed
        // only when the query changes, not every frame.
        if main_ui.is_searching() {
            let query = main_ui.search_query().to_string();
            if query != last_search_query {
                let hits = shared_sessions
                    .lock()
                    .map(|store| {
                        store
                            .search_all(&query, 50)
                            .into_iter()
                            .map(|h| main_window::HitRow {
                                session_id: h.session_id,
                                session_title: h.session_title,
                                snippet: h.snippet,
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                main_ui.set_search_hits(hits);
                last_search_query = query;
            }
        } else if !last_search_query.is_empty() {
            last_search_query.clear();
        }

        // Persist any pending session change, at most once a second. Writing on
        // every message measured ~9ms (over half a 60fps frame), so the store
        // marks itself dirty and the write is batched here.
        if last_session_flush.elapsed() >= Duration::from_millis(1000) {
            if let Ok(mut store) = shared_sessions.lock() {
                store.flush();
            }
            last_session_flush = Instant::now();
        }

        // Process chat window events
        if let Some(event) = chat_win.poll_event() {
            match event {
                ChatWindowEvent::Input(text) => {
                    println!("[Chat] You: {}", text);
                    if let Ok(mut store) = shared_sessions.lock() {
                        store.push(Role::User, text.clone());
                    }
                    bridge_for_input.send_message(&text);
                    main_ui.set_generating(true);
                }
                ChatWindowEvent::Toggle => {
                    chat_win.toggle();
                }
                ChatWindowEvent::Close => {
                    chat_win.hide();
                }
            }
        }

        // Forward AI responses into the session and the chat window.
        //
        // The bridge writes replies to its ChatOverlay; this drains them. Without
        // this, replies were only printed to stdout and never appeared in the UI.
        // The offset comes from the bridge's own cursor, not the window's
        // message count: the window also receives messages by other paths, so
        // using its count made the drain skip past replies and lose them.
        let consumed = bridge_for_input.consumed();
        let overlay_messages = bridge_for_input
            .overlay()
            .lock()
            .map(|o| o.messages().len())
            .unwrap_or(0);
        if overlay_messages > consumed {
            let fresh: Vec<(String, String)> = bridge_for_input
                .take_unconsumed()
                .into_iter()
                .map(|(role, text)| {
                    let role = match role {
                        MessageRole::User => "user".to_string(),
                        MessageRole::Assistant => "assistant".to_string(),
                        MessageRole::System => "system".to_string(),
                    };
                    (role, text)
                })
                .collect();
            for (role, text) in fresh {
                chat_win.add_message(&role, &text);
                // A reply means the AI finished, so the typing indicator stops.
                if role == "assistant" {
                    main_ui.set_generating(false);
                    // Chime on a reply: the window may not be in view.
                    audio.play("notification").ok();
                }
                // The main window re-reads the store each frame, so recording the
                // reply here is what makes it appear there.
                if let Ok(mut store) = shared_sessions.lock() {
                    let session_role = match role.as_str() {
                        "user" => Role::User,
                        "assistant" => Role::Assistant,
                        _ => Role::System,
                    };
                    store.push(session_role, text);
                }
            }
            // take_unconsumed already advanced the bridge's cursor.
        }

        if should_quit {
            break;
        }

        // Publish a status snapshot for `get_status` / query commands.
        //
        // Throttled: it clones the chat transcript, the creature map and every
        // setting, which is wasted work at 60+ frames a second when the only
        // consumer is a poller over IPC.
        if last_status_publish.elapsed() >= Duration::from_millis(250) {
            publish_status(
                &status,
                &creatures,
                &start_time,
                auto_cycle,
                &ai,
                &agent,
                (agent.tool_count(), rag.chunk_count()),
                &_ai_bridge,
                &frame_metrics,
            );
            // Keep the published settings snapshot in step with the store. It
            // used to be mirrored only on an IPC write, so undo and redo (which
            // change values without going through that path) left the snapshot
            // reporting the old value even though the store had reverted.
            //
            // Merged, not replaced: the snapshot also carries keys the control
            // loop inserts directly (`agents.list`, `agents.saved`, ...), and
            // overwriting the whole map wiped them on the next publish.
            if let Ok(mut st) = status.lock() {
                merge_store_settings(&mut st.settings, &theme_settings_reader);
            }
            last_status_publish = Instant::now();
        }

        // Hybrid sleep: sleep for most of the duration, then busy-wait for the
        // remainder. std::thread::sleep on Windows has ~15ms granularity, so the
        // sleep alone overshoots a short frame; the spin finishes the wait.
        //
        // The deadline is measured from the START of the frame. Measuring from
        // after the sleep added the whole target again, so every frame cost about
        // twice its budget (13ms target produced ~30ms frames).
        //
        // The target is deliberately NOT clamped to the display refresh. A
        // layered window is composited by DWM at the refresh rate, so the loop
        // cannot actually present faster than that; but the frame time is
        // target + work, so a lower target still produces a faster frame while
        // the work dominates. Clamping to 60Hz measured slower (21ms) than
        // leaving the 77fps setting (17ms).
        let target_ms = 1000.0 / read_target_fps(&theme_settings_reader) as f64;
        let frame_deadline = frame_start + Duration::from_secs_f64(target_ms / 1000.0);
        let sleep_ms = (target_ms - 2.0).max(0.0) as u64;
        if sleep_ms > 0 {
            std::thread::sleep(Duration::from_millis(sleep_ms));
        }
        while Instant::now() < frame_deadline {
            std::hint::spin_loop();
        }
    }

    // Stop any in-flight speech before exit.
    if let Some(engine) = tts.take() {
        engine.stop();
    }

    // Release the raised timer resolution; leaving it set affects the rest of
    // the system.
    unsafe {
        winapi::um::timeapi::timeEndPeriod(1);
    }

    // Land any deferred session write; a change made in the last second would
    // otherwise be lost on a clean exit.
    if let Ok(mut store) = shared_sessions.lock() {
        store.flush();
    }

    println!("ScreenBuddy shutdown");
}

/// Drain pending IPC control requests and apply them to live state.
///
/// Called once per frame from the main loop, so every effect happens on the
/// thread that owns the renderer and creature map.
#[allow(clippy::too_many_arguments)]
fn apply_control_requests(
    control_rx: &mut Option<tokio::sync::broadcast::Receiver<screenbuddy_core::ControlRequest>>,
    creatures: &mut HashMap<String, CreatureInstance>,
    bridge: &screenbuddy_core::AiChatBridge,
    rag: &mut screenbuddy_core::RagPipeline,
    audio: &mut screenbuddy_core::AudioSystem,
    settings: &mut screenbuddy_core::SettingsUI,
    // The settings window, so an import can re-read the rows it just changed.
    settings_win: &mut settings_window::SettingsView,
    tts: &mut Option<screenbuddy_core::TtsSystem>,
    auto_cycle: &mut bool,
    status: &std::sync::Arc<std::sync::Mutex<screenbuddy_core::RuntimeStatus>>,
    pinned: &mut HashMap<String, glam::Vec2>,
    // The agent store and the editor, so an agent changed over IPC shows up in
    // the GUI immediately rather than on next launch.
    agent_store: &std::sync::Arc<screenbuddy_core::AgentStore>,
    agent_win: &mut agent_editor::AgentEditor,
    agent_runtime: &screenbuddy_core::AgentRuntime,
    // Re-read per batch so a settings change takes effect without a restart.
    // A reader handle rather than `&SettingsUI`, because the render loop keeps a
    // mutable borrow of the owner while draining control requests.
    settings_ui: screenbuddy_core::SettingsReader,
) {
    let Some(rx) = control_rx.as_mut() else {
        return;
    };

    // rag_enabled was inert; read it per batch so the toggle is live.
    // A "Bool" here carries a number (1.0 = on), hence number().
    let rag_enabled = settings_ui
        .number("rag_enabled")
        .map(|v| v != 0.0)
        .unwrap_or(true);

    // Bound the batch so a flood of requests cannot starve the render loop.
    for _ in 0..64 {
        match rx.try_recv() {
            Ok(screenbuddy_core::ControlRequest::SendChat(message)) => {
                println!("[IPC] Chat: {}", message);
                bridge.send_message(&message);
            }
            Ok(screenbuddy_core::ControlRequest::RunAgent(prompt)) => {
                println!("[IPC] Agent prompt: {}", prompt);
            }
            Ok(screenbuddy_core::ControlRequest::SetAnimation { id, state }) => {
                let target = to_anim_state(state);
                let mut changed = 0;
                for (cid, creature) in creatures.iter_mut() {
                    // `None` means "apply to every creature".
                    if let Some(want) = &id {
                        if cid != want && creature.name != *want {
                            continue;
                        }
                    }
                    creature.set_state(target);
                    changed += 1;
                }
                println!(
                    "[IPC] Animation -> {} ({} creature(s))",
                    state.as_str(),
                    changed
                );
            }
            Ok(screenbuddy_core::ControlRequest::MoveCreature { id, x, y }) => {
                match creatures.get_mut(&id) {
                    Some(creature) => {
                        let target = glam::Vec2::new(x, y);
                        creature.physics.position = target;
                        // Pin so physics integration does not drift it away.
                        pinned.insert(id.clone(), target);
                        println!("[IPC] Moved {} to ({}, {})", id, x, y);
                    }
                    None => eprintln!("[IPC] Unknown creature '{}'", id),
                }
            }
            Ok(screenbuddy_core::ControlRequest::SetCreatureVisible { id, visible }) => {
                match creatures.get_mut(&id) {
                    Some(creature) => {
                        creature.visible = visible;
                        println!("[IPC] {} {}", id, if visible { "shown" } else { "hidden" });
                    }
                    None => eprintln!("[IPC] Unknown creature '{}'", id),
                }
            }
            Ok(screenbuddy_core::ControlRequest::PlaySound(name)) => {
                if let Err(e) = audio.play(&name) {
                    eprintln!("[IPC] Sound '{}' failed: {}", name, e);
                }
            }
            Ok(screenbuddy_core::ControlRequest::Speak(text)) => {
                let engine = tts.get_or_insert_with(screenbuddy_core::TtsSystem::new);
                if !engine.is_available() {
                    eprintln!("[IPC] TTS engine unavailable");
                } else if let Err(e) = engine.speak(&text) {
                    eprintln!("[IPC] TTS failed: {}", e);
                }
            }
            Ok(screenbuddy_core::ControlRequest::ListAgents) => {
                // Published into the status snapshot so `get_setting` can read it
                // back: the control channel has no reply path, and inventing one
                // here would mean plumbing a response channel through the loop.
                let agents: Vec<serde_json::Value> = agent_store
                    .list()
                    .iter()
                    .map(|a| serde_json::to_value(a).unwrap_or(serde_json::Value::Null))
                    .collect();
                println!("[Agents] {} profile(s) available", agents.len());
                if let Ok(mut snapshot) = status.lock() {
                    snapshot
                        .settings
                        .insert("agents.list".into(), serde_json::json!(agents));
                }
            }
            Ok(screenbuddy_core::ControlRequest::SaveAgent { profile }) => {
                // Deserialise into the typed profile so a malformed or hostile
                // payload is validated and clamped rather than stored raw.
                match serde_json::from_value::<screenbuddy_core::AgentProfile>(profile) {
                    Ok(parsed) => match agent_store.save(parsed) {
                        Ok(saved) => {
                            println!("[Agents] Saved '{}' over IPC", saved.name);
                            if let Ok(mut snapshot) = status.lock() {
                                snapshot.settings.insert(
                                    "agents.saved".into(),
                                    serde_json::to_value(&saved).unwrap_or(serde_json::Value::Null),
                                );
                            }
                        }
                        Err(e) => eprintln!("[Agents] Save rejected: {e}"),
                    },
                    Err(e) => eprintln!("[Agents] Malformed agent profile: {e}"),
                }
                agent_win.reload(agent_store);
            }
            Ok(screenbuddy_core::ControlRequest::DuplicateAgent { id }) => {
                match agent_store.duplicate(&id, None) {
                    Ok(copy) => {
                        println!("[Agents] Duplicated '{}' as '{}'", id, copy.name);
                        if let Ok(mut snapshot) = status.lock() {
                            snapshot.settings.insert(
                                "agents.saved".into(),
                                serde_json::to_value(&copy).unwrap_or(serde_json::Value::Null),
                            );
                        }
                    }
                    Err(e) => eprintln!("[Agents] Duplicate failed: {e}"),
                }
                agent_win.reload(agent_store);
            }
            Ok(screenbuddy_core::ControlRequest::ActivateAgent { id }) => {
                match agent_store.get(&id) {
                    Some(profile) => {
                        // This is the step that makes an agent real: the profile's
                        // persona, model, tools, temperature and budget all become
                        // the settings the next request uses.
                        agent_runtime.apply_profile(&profile);
                        if let Err(e) = agent_store.set_active(&id) {
                            eprintln!("[Agents] Could not persist activation: {e}");
                        }
                        println!(
                            "[Agents] Activated '{}' (persona {:?}, {} tool steps)",
                            profile.name, profile.persona, profile.max_iterations
                        );
                        if let Ok(mut snapshot) = status.lock() {
                            snapshot.settings.insert(
                                "agents.active".into(),
                                serde_json::to_value(&profile).unwrap_or(serde_json::Value::Null),
                            );
                        }
                    }
                    None => eprintln!("[Agents] No agent '{id}' to activate"),
                }
            }
            Ok(screenbuddy_core::ControlRequest::DeleteAgent { id }) => {
                match agent_store.delete(&id) {
                    Ok(true) => println!("[Agents] Deleted '{id}'"),
                    Ok(false) => println!("[Agents] '{id}' is a preset or unknown; kept"),
                    Err(e) => eprintln!("[Agents] Delete failed: {e}"),
                }
                agent_win.reload(agent_store);
            }
            Ok(screenbuddy_core::ControlRequest::ExportSettings { path }) => {
                // The file dialog already ran on the settings window's thread;
                // the write happens here because the render loop owns the store.
                let target = std::path::PathBuf::from(&path);
                match settings.export_to(&target) {
                    Ok(n) => {
                        println!("[Settings] Exported {n} setting(s) to {path}");
                        settings_win.sync();
                    }
                    Err(e) => eprintln!("[Settings] Export failed: {e}"),
                }
            }
            Ok(screenbuddy_core::ControlRequest::ImportSettings { path }) => {
                let source = std::path::PathBuf::from(&path);
                match settings.import_from(&source) {
                    Ok((applied, skipped)) => {
                        println!(
                            "[Settings] Imported {applied} setting(s) from {path} ({} skipped)",
                            skipped.len()
                        );
                        // Re-read so the rows show the imported values rather
                        // than the ones painted before the import.
                        settings_win.sync();
                        if !skipped.is_empty() {
                            println!("[Settings] Skipped unknown keys: {}", skipped.join(", "));
                        }
                    }
                    Err(e) => eprintln!("[Settings] Import failed: {e}"),
                }
            }
            Ok(screenbuddy_core::ControlRequest::NineRouterStatus { endpoint, api_key }) => {
                // Discovery is the useful part: it tells the user whether 9Router
                // is running and what it can actually reach, instead of leaving
                // them to guess from a failed chat.
                // Probe on its own thread with its own runtime: blocking inside
                // the render loop would stall the window, and calling block_on
                // from within the app's runtime would deadlock.
                let probe_endpoint = endpoint.clone();
                let probe_key = api_key.clone();
                let discovered = std::thread::spawn(move || {
                    let worker = screenbuddy_core::AiEngine::new(screenbuddy_core::AiConfig {
                        nine_router_endpoint: probe_endpoint.clone(),
                        nine_router_api_key: probe_key.clone(),
                        ..Default::default()
                    });
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(rt) => rt.block_on(worker.nine_router_models_at(
                            probe_endpoint.as_deref(),
                            probe_key.as_deref(),
                        )),
                        Err(e) => Err(screenbuddy_core::Error::Network(format!(
                            "9Router probe runtime unavailable: {e}"
                        ))),
                    }
                })
                .join()
                .unwrap_or_else(|_| {
                    Err(screenbuddy_core::Error::Network(
                        "9Router probe thread panicked".into(),
                    ))
                });
                match discovered {
                    Ok(models) => {
                        let ids: Vec<String> = models.iter().map(|m| m.id.clone()).collect();
                        if let Ok(mut snapshot) = status.lock() {
                            snapshot
                                .settings
                                .insert("nine_router.reachable".into(), serde_json::json!(true));
                        }
                        println!("[9Router] {} models available: {:?}", ids.len(), ids);
                        if let Ok(mut snapshot) = status.lock() {
                            snapshot
                                .settings
                                .insert("nine_router.models".into(), serde_json::json!(ids));
                        }
                    }
                    Err(e) => eprintln!("[9Router] unavailable: {e}"),
                }
            }
            Ok(screenbuddy_core::ControlRequest::SetSetting { key, value }) => {
                let parsed = json_to_setting(value.clone());
                // The store rejects unknown keys and out-of-range values. That
                // has to reach the caller: replying "success" regardless is how
                // a rejected write looks like an accepted one.
                match settings.set(&key, parsed) {
                    Ok(()) => {
                        // Mirror into the snapshot so `get_setting` reads back
                        // the value that was just written.
                        if let Ok(mut snapshot) = status.lock() {
                            snapshot.settings.insert(key.clone(), value.clone());
                        }
                        println!("[IPC] Setting '{}' updated", key);
                    }
                    Err(e) => eprintln!("[IPC] Setting '{}' rejected: {}", key, e),
                }
            }
            Ok(screenbuddy_core::ControlRequest::SetAutoCycle { enabled }) => {
                *auto_cycle = enabled;
                println!(
                    "[IPC] Auto animation cycle {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }
            Ok(screenbuddy_core::ControlRequest::MemoryIngest { source, content }) => {
                // rag_enabled was a setting nothing read; both paths honour it now.
                if !rag_enabled {
                    println!("[IPC] Memory is disabled in settings; ingest ignored");
                    continue;
                }
                match rag.ingest_str(&source, &content) {
                    Ok(n) => {
                        println!("[IPC] Ingested {} chunk(s) from {}", n, source);
                        // Persist immediately: an ingested document that vanished
                        // on restart was the whole complaint about the old
                        // in-memory-only index.
                        match rag.save(rag_index_path()) {
                            Ok(()) => println!("[RAG] index saved ({} chunks)", rag.chunk_count()),
                            Err(e) => eprintln!("[RAG] could not save index: {e}"),
                        }
                    }
                    Err(e) => eprintln!("[IPC] Ingest failed: {}", e),
                }
            }
            Ok(screenbuddy_core::ControlRequest::MemorySearch { query, limit }) => {
                let hits = if rag_enabled {
                    rag.search(&query, limit)
                } else {
                    Vec::new()
                };
                println!("[IPC] Memory search '{}' -> {} hit(s)", query, hits.len());
                if let Ok(mut snapshot) = status.lock() {
                    snapshot.search_results = serde_json::Value::Array(
                        hits.into_iter()
                            .map(|(score, chunk)| {
                                serde_json::json!({
                                    "score": score,
                                    "source": chunk.source,
                                    "text": chunk.text,
                                })
                            })
                            .collect(),
                    );
                    snapshot.last_query = Some(query);
                }
            }
            // The receiver lagged behind; report rather than silently continuing.
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(skipped)) => {
                eprintln!(
                    "[IPC] Dropped {} control request(s): main loop too busy",
                    skipped
                );
            }
            Err(tokio::sync::broadcast::error::TryRecvError::Closed)
            | Err(tokio::sync::broadcast::error::TryRecvError::Empty) => break,
        }
    }
}

fn to_anim_state(state: screenbuddy_core::AnimStateCtl) -> AnimState {
    match state {
        screenbuddy_core::AnimStateCtl::Idle => AnimState::Idle,
        screenbuddy_core::AnimStateCtl::Walk => AnimState::Walk,
        screenbuddy_core::AnimStateCtl::Fly => AnimState::Fly,
        screenbuddy_core::AnimStateCtl::Sleep => AnimState::Sleep,
        screenbuddy_core::AnimStateCtl::Celebrate => AnimState::Celebrate,
    }
}

/// Show the About box.
///
/// A tray menu item that only writes to stdout is invisible to the user, so this
/// puts the same information in a real dialog.
fn show_about_dialog() {
    use std::mem;
    use std::ptr;
    use winapi::um::winuser::*;

    unsafe {
        let message: Vec<u16> = format!(
            "ScreenBuddy v{}\n\nAI Desktop Companion\n\nA small creature that lives on your\ndesktop and reacts to what you are doing.\n\nSessions, chat and memory are stored locally.\nNo telemetry, no accounts.",
            env!("CARGO_PKG_VERSION")
        )
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

        MessageBoxW(
            ptr::null_mut(),
            message.as_ptr(),
            wide("About ScreenBuddy ").as_ptr(),
            MB_OK | MB_ICONINFORMATION,
        );
        let _ = mem::size_of::<i32>();
    }
}

/// Encode a literal as a NUL-terminated wide string.
#[cfg(windows)]
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Where chat sessions are persisted.
///
/// Kept beside the other app data rather than in the current directory, so the
/// transcript is not lost by running the binary from somewhere else.
fn sessions_path() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    base.join("ScreenBuddy").join("sessions.json")
}

/// Where the RAG index is persisted, beside the other app data.
fn rag_index_path() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(std::path::PathBuf::from))
        .unwrap_or_else(std::env::temp_dir);
    base.join("ScreenBuddy").join("rag_index.json")
}

/// Map a JSON setting value onto the typed setting enum.
fn json_to_setting(value: serde_json::Value) -> screenbuddy_core::SettingValue {
    use screenbuddy_core::SettingValue as SV;
    match value {
        serde_json::Value::Bool(b) => SV::Bool(if b { 1.0 } else { 0.0 }),
        serde_json::Value::Number(n) => {
            let f = n.as_f64().unwrap_or(0.0);
            // Preserve integer inputs as Int so count-style settings stay typed.
            if n.as_i64().is_some() {
                SV::Int(f as i64)
            } else {
                SV::Float(f)
            }
        }
        serde_json::Value::String(s) => SV::String(s),
        other => SV::String(other.to_string()),
    }
}

/// Merge the settings store into a published snapshot.
///
/// Merged rather than replaced because the snapshot also carries keys the
/// control loop inserts directly (`agents.list`, `agents.saved`, ...).
/// Replacing the whole map wiped those on the next publish, which made
/// `get_setting agents.list` return nothing even though the control handler
/// had just populated it.
fn merge_store_settings(
    snapshot: &mut HashMap<String, serde_json::Value>,
    store: &screenbuddy_core::SettingsReader,
) {
    for (key, value) in store.snapshot() {
        snapshot.insert(key, setting_to_json(&value));
    }
}

fn setting_to_json(value: &screenbuddy_core::SettingValue) -> serde_json::Value {
    use screenbuddy_core::SettingValue as SV;
    match value {
        // Bool is stored as 1.0/0.0 internally; report it as a real bool.
        SV::Bool(v) => serde_json::Value::Bool(*v != 0.0),
        SV::Int(v) => serde_json::json!(v),
        SV::Float(v) => serde_json::json!(v),
        SV::String(v) => serde_json::json!(v),
        SV::Enum(name, options) => serde_json::json!({ "name": name, "options": options }),
    }
}

/// Refresh the shared status snapshot.
#[allow(clippy::too_many_arguments)]
fn publish_status(
    status: &std::sync::Arc<std::sync::Mutex<screenbuddy_core::RuntimeStatus>>,
    creatures: &HashMap<String, CreatureInstance>,
    start_time: &Instant,
    auto_cycle: bool,
    ai: &screenbuddy_core::AiEngine,
    agent: &screenbuddy_core::AgentRuntime,
    loaded: (usize, usize),
    ai_bridge: &screenbuddy_core::AiChatBridge,
    metrics: &screenbuddy_core::metrics::FrameMetrics,
) {
    let (tool_count, memory_chunks) = loaded;
    let elapsed = start_time.elapsed().as_secs_f64();
    let Ok(mut snapshot) = status.lock() else {
        return;
    };
    snapshot.frame_count += 1;
    // Frame-time percentiles, so stutter is visible. The average below cannot
    // show a hitch, which is the thing a user actually notices.
    snapshot.metrics = metrics.summary();
    // Average FPS across the whole run: total frames over total uptime. This is
    // stable, unlike a per-frame instantaneous reading that would alias.
    snapshot.fps = if elapsed > 0.0 {
        snapshot.frame_count as f64 / elapsed
    } else {
        0.0
    };
    snapshot.creature_count = creatures.len();
    snapshot.visible_count = creatures.values().filter(|c| c.visible).count();
    snapshot.auto_cycle = auto_cycle;
    snapshot.uptime_secs = elapsed;
    snapshot.model_count = ai.available_models().len();
    snapshot.tool_count = tool_count;
    snapshot.memory_chunks = memory_chunks;
    // Publish which agent is in effect and the settings it is running with, so a
    // query can confirm that activating an agent actually changed anything.
    snapshot.active_agent = agent.active_agent();
    snapshot.active_agent_settings =
        serde_json::to_value(agent.active_config()).unwrap_or(serde_json::Value::Null);
    snapshot.creatures.extend(creatures.iter().map(|(id, c)| {
        (
            id.clone(),
            serde_json::json!({
                "name": c.name,
                "x": c.physics.position.x,
                "y": c.physics.position.y,
                "visible": c.visible,
                "state": format!("{:?}", c.current_state).to_lowercase(),
            }),
        )
    }));
    // Drop entries for creatures that no longer exist.
    let live: std::collections::HashSet<&String> = creatures.keys().collect();
    snapshot.creatures.retain(|id, _| live.contains(id));
    snapshot.chat_history = ai_bridge.overlay().lock().unwrap().messages()
        .iter()
        .map(|m| (m.role.to_string(), m.content.clone()))
        .collect();
    // Sync to process-wide atomics so the tray thread reads fresh state.
    screenbuddy_core::set_creature_count(creatures.len());
    screenbuddy_core::set_always_on_top(
        snapshot.always_on_top,
    );
}

fn create_window_and_run(hwnd_tx: mpsc::Sender<isize>, width: i32, height: i32) {
    unsafe {
        let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
        let class_name: Vec<u16> = "ScreenBuddyWindow\0".encode_utf16().collect();
        let window_title: Vec<u16> = "ScreenBuddy\0".encode_utf16().collect();

        let wc = winapi::um::winuser::WNDCLASSEXW {
            cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
            style: winapi::um::winuser::CS_HREDRAW | winapi::um::winuser::CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: h_instance,
            hCursor: winapi::um::winuser::LoadCursorW(
                ptr::null_mut(),
                winapi::um::winuser::IDC_ARROW,
            ),
            lpszClassName: class_name.as_ptr(),
            ..mem::zeroed()
        };
        winapi::um::winuser::RegisterClassExW(&wc);

        let ex_style = winapi::um::winuser::WS_EX_LAYERED
            | winapi::um::winuser::WS_EX_TOPMOST
            | winapi::um::winuser::WS_EX_TOOLWINDOW;
        let hwnd = winapi::um::winuser::CreateWindowExW(
            ex_style,
            class_name.as_ptr(),
            window_title.as_ptr(),
            winapi::um::winuser::WS_POPUP | winapi::um::winuser::WS_VISIBLE,
            100,
            100,
            width,
            height,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );
        if hwnd.is_null() {
            eprintln!("Window failed");
            return;
        }
        hwnd_tx.send(hwnd as isize).ok();

        let mut msg: winapi::um::winuser::MSG = mem::zeroed();
        while winapi::um::winuser::GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
            winapi::um::winuser::TranslateMessage(&msg);
            winapi::um::winuser::DispatchMessageW(&msg);
        }
    }
}



unsafe extern "system" fn window_proc(
    hwnd: winapi::shared::windef::HWND,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match msg {
        winapi::um::winuser::WM_DESTROY => {
            winapi::um::winuser::PostQuitMessage(0);
            0
        }
        winapi::um::winuser::WM_LBUTTONDOWN => {
            winapi::um::winuser::SendMessageW(
                hwnd,
                winapi::um::winuser::WM_NCLBUTTONDOWN,
                winapi::um::winuser::HTCAPTION as usize,
                0,
            );
            0
        }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use screenbuddy_core::SettingValue;

    #[test]
    fn merging_the_store_keeps_keys_the_control_loop_inserted() {
        // Regression: the publish used to replace the whole settings map with the
        // store snapshot, which wiped `agents.list` a control handler had just
        // inserted. get_setting then returned nothing for it.
        let mut store = screenbuddy_core::SettingsUI::in_memory();
        store.set("creature_count", SettingValue::Int(7)).unwrap();
        let reader = store.reader();

        let mut snapshot = HashMap::new();
        snapshot.insert(
            "agents.list".to_string(),
            serde_json::json!([{"name": "Coder"}]),
        );

        merge_store_settings(&mut snapshot, &reader);

        assert_eq!(
            snapshot.get("agents.list"),
            Some(&serde_json::json!([{"name": "Coder"}])),
            "a key the control loop inserted must survive the store merge"
        );
        assert_eq!(
            snapshot.get("creature_count"),
            Some(&serde_json::json!(7)),
            "the store value must reach the snapshot"
        );
    }

    #[test]
    fn merging_twice_reflects_a_changed_store_value() {
        let mut store = screenbuddy_core::SettingsUI::in_memory();
        store.set("creature_count", SettingValue::Int(7)).unwrap();
        let reader = store.reader();
        let mut snapshot = HashMap::new();

        merge_store_settings(&mut snapshot, &reader);
        store.set("creature_count", SettingValue::Int(3)).unwrap();
        merge_store_settings(&mut snapshot, &reader);

        assert_eq!(
            snapshot.get("creature_count"),
            Some(&serde_json::json!(3)),
            "a later merge must overwrite the earlier value"
        );
    }

    #[test]
    fn the_publish_deadline_is_measured_from_the_frame_start() {
        // Regression: measuring from after the sleep added the whole target again,
        // so every frame cost about twice its budget.
        let target_ms = 1000.0 / 77.0;
        let frame_start = Instant::now();
        // Simulate work already done this frame.
        std::thread::sleep(Duration::from_millis(4));
        let deadline = frame_start + Duration::from_secs_f64(target_ms / 1000.0);

        // The deadline must lie inside the original frame. At 77fps the budget is
        // ~13ms, so after 4ms of work the remaining wait is ~9ms, not ~13ms more.
        let remaining = deadline.saturating_duration_since(Instant::now());
        assert!(
            remaining < Duration::from_millis(target_ms as u64),
            "deadline must fall inside the frame: remaining {remaining:?}"
        );
        assert!(
            remaining > Duration::from_millis(1),
            "there should be time left to wait: {remaining:?}"
        );
    }

    #[test]
    fn a_bool_setting_is_published_as_a_real_bool() {
        // Stored as 1.0/0.0; a consumer reading JSON must not get a number.
        assert_eq!(setting_to_json(&SettingValue::Bool(1.0)), serde_json::json!(true));
        assert_eq!(setting_to_json(&SettingValue::Bool(0.0)), serde_json::json!(false));
        assert_eq!(setting_to_json(&SettingValue::Int(3)), serde_json::json!(3));
        assert_eq!(setting_to_json(&SettingValue::Float(0.5)), serde_json::json!(0.5));
        assert_eq!(
            setting_to_json(&SettingValue::String("light".into())),
            serde_json::json!("light")
        );
    }
}
