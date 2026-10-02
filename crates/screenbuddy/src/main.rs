use image::RgbaImage;
use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

mod animation;
mod chat_window;
mod composite_renderer;
mod settings_window;

use animation::SpriteAnimator;
use chat_window::{ChatWindow, ChatWindowEvent};
use composite_renderer::{CompositeRenderer, RenderableCreature};
use screenbuddy_core::chat_overlay::MessageRole;
use screenbuddy_core::session::{Role, SessionStore};
use settings_window::SettingsWindow;

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
    println!("ScreenBuddy starting...");
    let hw = screenbuddy_core::hardware_profile();
    println!("Hardware:\n{}", hw.summary());
    let _pool = screenbuddy_core::ThreadPool::new();
    let config = screenbuddy_core::ConfigManager::new();
    let mut audio = screenbuddy_core::AudioSystem::new();
    audio.load_default_sounds();
    let ai_config = config.get().ai.clone();
    let ai = screenbuddy_core::AiEngine::new(ai_config);
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

    let mut renderer = CompositeRenderer::new(hwnd as *mut core::ffi::c_void, 800, 600);
    let frame_duration = Duration::from_millis(1000 / 30);
    let start_time = Instant::now();
    println!("Animation at 30 FPS");

    let mut tray = screenbuddy_core::SystemTray::new();
    tray.start().ok();
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
    let ai_clone = screenbuddy_core::AiEngine::new(config.get().ai.clone());
    let chat_clone = screenbuddy_core::ChatOverlay::new();
    let _ai_bridge = screenbuddy_core::AiChatBridge::new(ai_clone, chat_clone);
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
                winapi::um::fileapi::GetFileType(std::io::stdin().as_raw_handle()) == FILE_TYPE_CHAR
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
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if trimmed == "/quit" {
                    break;
                }
                input_tx2.send(trimmed.to_string()).ok();
            }
            prompt(&mut io::stdout());
        }
    });

    // Clone bridge for the input handler
    let bridge_for_input = _ai_bridge.clone();

    // Mouse interaction
    let cursor_pos: Option<glam::Vec2> = None;
    let dragged_creature: Option<usize> = None;

    // RAG Pipeline - memory system
    let mut rag = screenbuddy_core::RagPipeline::new();
    let _ = rag.ingest_document(
        std::path::Path::new("memory://startup"),
        "ScreenBuddy is an AI desktop companion. It has 7 creatures with unique personalities. The user can interact with them via click, drag, and chat."
    );
    println!("[RAG] Memory system initialized");

    // Agent Runtime - agentic capabilities. The default tools (echo, time) make
    // the runtime immediately useful instead of reporting zero tools.
    let agent = screenbuddy_core::AgentRuntime::new(std::sync::Arc::new(ai.clone()));
    for tool in screenbuddy_core::default_tools() {
        agent.register_tool(&tool.name.clone(), tool);
    }
    let mut agent_rx = agent.subscribe();
    println!(
        "[Agent] Runtime initialized with {} tools",
        agent.tool_count()
    );

    // ---- IPC control surface -------------------------------------------------
    // Lets an external agent (e.g. Hermes through the bundled MCP server) drive
    // this running instance over TCP. Disabled when the port cannot be bound, so
    // a second instance degrades gracefully instead of failing to start.
    let ipc_config = screenbuddy_core::IpcConfig {
        addr: std::env::var("SCREENBUDDY_IPC_ADDR")
            .unwrap_or_else(|_| format!("127.0.0.1:{}", screenbuddy_core::DEFAULT_PORT)),
        enable_hot_reload: true,
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
    println!(
        "[Session] {} session(s) loaded",
        shared_sessions.lock().map(|s| s.list().len()).unwrap_or(0)
    );

    if std::env::var("SCREENBUDDY_DISABLE_IPC").is_err() {
        // `main` is not inside a Tokio runtime, so the server gets its own and
        // runs on a dedicated OS thread. This mirrors AiChatBridge.
        // The server and this loop must share one snapshot handle; two copies
        // would make every query return defaults.
        let ipc = screenbuddy_core::IpcServer::with_status(ipc_config.clone(), status.clone())
            .with_sessions(shared_sessions.clone());
        // Subscribe before starting the thread so a request arriving
        // immediately after startup is not missed.
        control_rx = Some(ipc.subscribe_control());
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

    // Settings UI
    let mut settings = screenbuddy_core::SettingsUI::new();
    settings
        .set(
            "creature_count",
            screenbuddy_core::SettingValue::Int(creatures.len() as i64),
        )
        .ok();
    settings
        .set(
            "collision_avoidance",
            screenbuddy_core::SettingValue::Bool(1.0),
        )
        .ok();
    settings
        .set(
            "cursor_interaction",
            screenbuddy_core::SettingValue::Bool(1.0),
        )
        .ok();
    println!("[Settings] {} settings loaded", settings.export().len());
    // Seed the snapshot from the loaded settings so reads work before any write.
    if let Ok(mut snapshot) = status.lock() {
        for (name, value) in settings.export() {
            snapshot
                .settings
                .insert(name, serde_json::json!(setting_to_json(&value)));
        }
    }

    // Apply initial settings to audio
    if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.get("volume_master") {
        audio.set_master_volume(vol as f32);
    }

    // Per-Pet Windows
    let mut pet_windows = screenbuddy_core::PerPetWindowManager::new();
    for (i, (_id, creature)) in creatures.iter().enumerate() {
        let x = 50 + (i as i32 * 150);
        let y = 100 + (i as i32 * 100);
        pet_windows.create_window(&creature.name, x, y);
    }
    println!(
        "[PerPet] {} pet windows created",
        pet_windows.window_count()
    );

    // Chat Window
    // Chat sessions persist across restarts; the window renders a view of the
    // current session rather than owning the conversation.
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
    let mut settings_win = SettingsWindow::new();
    let settings_hwnd = settings_window::create_settings_window().unwrap_or(0);
    if settings_hwnd != 0 {
        println!("[Settings] Window created: 0x{:x}", settings_hwnd);
    }

    // Create per-pet windows
    let mut pet_hwnds: Vec<*mut core::ffi::c_void> = Vec::new();
    for (i, (_id, creature)) in creatures.iter().enumerate() {
        let x = 100 + (i as i32 * 160);
        let y = 100 + (i as i32 * 120);
        let hwnd = create_pet_window(&creature.name, x, y);
        pet_hwnds.push(hwnd);
        println!(
            "[PerPet] Created window for {} at ({}, {})",
            creature.name, x, y
        );
    }

    loop {
        let elapsed = start_time.elapsed();
        let dt = 1.0 / 30.0;

        // Forward AI responses to chat window
        {
            let ai_messages = _ai_bridge.overlay().lock().unwrap().messages().to_vec();
            let chat_messages = chat_win.messages().to_vec();
            // Sync new AI messages to chat window
            for msg in &ai_messages {
                if chat_messages.iter().all(|(r, t)| {
                    !(r.contains(&format!("{:?}", msg.role).to_lowercase()) && t == &msg.content)
                }) && (msg.role == screenbuddy_core::ChatMsgRole::Assistant
                    || msg.role == screenbuddy_core::ChatMsgRole::User)
                {
                    let role_str = format!("{:?}", msg.role).to_lowercase();
                    chat_win.add_message(&role_str, &msg.content);
                }
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
            &mut settings,
            &mut tts,
            &mut auto_cycle,
            &status,
            &mut pinned,
        );

        // Collect positions for collision avoidance
        let all_positions: Vec<glam::Vec2> =
            creatures.values().map(|c| c.physics.position).collect();

        // Update creatures
        for (i, (id, creature)) in creatures.iter_mut().enumerate() {
            let cursor = if dragged_creature == Some(i) {
                None
            } else {
                cursor_pos
            };
            creature.physics.update(dt, &screen, cursor, &all_positions);
            // Re-assert an externally requested position after integration.
            if let Some(target) = pinned.get(id) {
                creature.physics.position = *target;
            }
            creature.animator.update(elapsed.as_secs_f32());
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
        pet_windows.process_events();

        // Process agent events
        if let Ok(event) = agent_rx.try_recv() {
            match event {
                screenbuddy_core::AgentEvent::ToolUse { name, input } => {
                    println!("[Agent] Tool called: {} {:?}", name, input);
                }
                screenbuddy_core::AgentEvent::Message(text) => {
                    println!("[Agent] Response: {}", text);
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
        if settings.changed() {
            if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.get("volume_master")
            {
                audio.set_master_volume(vol as f32);
            }
            if let Some(screenbuddy_core::SettingValue::Float(_speed)) =
                settings.get("animation_speed")
            {
                // Could apply to animator
            }
        }

        // Tray events
        if let Some(event) = tray.poll_event() {
            match event {
                screenbuddy_core::TrayEvent::Quit => break,
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
                screenbuddy_core::TrayEvent::OpenSettings => {
                    println!("[Settings] Toggling settings window");
                    settings_win.toggle();
                }
                screenbuddy_core::TrayEvent::About => {
                    println!(
                        "[About] ScreenBuddy v{} - AI Desktop Companion",
                        env!("CARGO_PKG_VERSION")
                    );
                }
            }
        }

        // Process pet window messages
        pet_window_message_loop();

        // Process chat window events
        if let Some(event) = chat_win.poll_event() {
            match event {
                ChatWindowEvent::Input(text) => {
                    println!("[Chat] You: {}", text);
                    if let Ok(mut store) = shared_sessions.lock() {
                        store.push(Role::User, text.clone());
                    }
                    bridge_for_input.send_message(&text);
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
        let rendered = chat_win.messages().len();
        let overlay_messages = bridge_for_input
            .overlay()
            .lock()
            .map(|o| o.messages().len())
            .unwrap_or(0);
        if overlay_messages > rendered {
            let fresh: Vec<(String, String)> = bridge_for_input
                .overlay()
                .lock()
                .map(|o| {
                    o.messages()
                        .iter()
                        .skip(rendered)
                        .map(|m| {
                            let role = match m.role {
                                MessageRole::User => "user".to_string(),
                                MessageRole::Assistant => "assistant".to_string(),
                                MessageRole::System => "system".to_string(),
                            };
                            (role, m.content.clone())
                        })
                        .collect()
                })
                .unwrap_or_default();
            for (role, text) in fresh {
                chat_win.add_message(&role, &text);
                if role == "assistant" {
                    if let Ok(mut store) = shared_sessions.lock() {
                        store.push(Role::Assistant, text);
                    }
                }
            }
        }

        // Publish a status snapshot for `get_status` / query commands.
        publish_status(
            &status,
            &creatures,
            &start_time,
            auto_cycle,
            &ai,
            agent.tool_count(),
            rag.chunk_count(),
        );

        std::thread::sleep(frame_duration);
    }

    // Stop any in-flight speech before exit.
    if let Some(engine) = tts.take() {
        engine.stop();
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
    tts: &mut Option<screenbuddy_core::TtsSystem>,
    auto_cycle: &mut bool,
    status: &std::sync::Arc<std::sync::Mutex<screenbuddy_core::RuntimeStatus>>,
    pinned: &mut HashMap<String, glam::Vec2>,
) {
    let Some(rx) = control_rx.as_mut() else {
        return;
    };

    // Bound the batch so a flood of requests cannot starve the render loop.
    for _ in 0..64 {
        match rx.try_recv() {
            Ok(screenbuddy_core::ControlRequest::SendChat(message)) => {
                println!("[IPC] Chat: {}", message);
                bridge.send_message(&message);
            }
            Ok(screenbuddy_core::ControlRequest::RunAgent(prompt)) => {
                // The agent runtime is async and borrows self; drive it inline on
                // the tokio handle so a response can be awaited without borrowing
                // across the spawn boundary.
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
            Ok(screenbuddy_core::ControlRequest::SetSetting { key, value }) => {
                let parsed = json_to_setting(value.clone());
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
            Ok(screenbuddy_core::ControlRequest::SetAutoCycle(enabled)) => {
                *auto_cycle = enabled;
                println!(
                    "[IPC] Auto animation cycle {}",
                    if enabled { "enabled" } else { "disabled" }
                );
            }
            Ok(screenbuddy_core::ControlRequest::MemoryIngest { source, content }) => {
                match rag.ingest_str(&source, &content) {
                    Ok(n) => println!("[IPC] Ingested {} chunk(s) from {}", n, source),
                    Err(e) => eprintln!("[IPC] Ingest failed: {}", e),
                }
            }
            Ok(screenbuddy_core::ControlRequest::MemorySearch { query, limit }) => {
                let hits = rag.search(&query, limit);
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

/// Render a typed setting back to JSON for the snapshot.
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
fn publish_status(
    status: &std::sync::Arc<std::sync::Mutex<screenbuddy_core::RuntimeStatus>>,
    creatures: &HashMap<String, CreatureInstance>,
    start_time: &Instant,
    auto_cycle: bool,
    ai: &screenbuddy_core::AiEngine,
    tool_count: usize,
    memory_chunks: usize,
) {
    let elapsed = start_time.elapsed().as_secs_f64();
    let Ok(mut snapshot) = status.lock() else {
        return;
    };
    snapshot.frame_count += 1;
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

fn pet_window_message_loop() {
    unsafe {
        let mut msg: winapi::um::winuser::MSG = mem::zeroed();
        while winapi::um::winuser::PeekMessageW(
            &mut msg,
            ptr::null_mut(),
            0,
            0,
            winapi::um::winuser::PM_REMOVE,
        ) != 0
        {
            winapi::um::winuser::TranslateMessage(&msg);
            winapi::um::winuser::DispatchMessageW(&msg);
        }
    }
}

fn create_pet_window(creature_name: &str, x: i32, y: i32) -> *mut core::ffi::c_void {
    unsafe {
        let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
        let class_name: Vec<u16> = format!("ScreenBuddyPet{}\0", x).encode_utf16().collect();
        let window_title: Vec<u16> = format!("{} - ScreenBuddy\0", creature_name)
            .encode_utf16()
            .collect();

        let wc = winapi::um::winuser::WNDCLASSEXW {
            cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
            style: winapi::um::winuser::CS_HREDRAW | winapi::um::winuser::CS_VREDRAW,
            lpfnWndProc: Some(pet_window_proc),
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
            x,
            y,
            128,
            128,
            ptr::null_mut(),
            ptr::null_mut(),
            h_instance,
            ptr::null_mut(),
        );
        hwnd as *mut core::ffi::c_void
    }
}

unsafe extern "system" fn pet_window_proc(
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
