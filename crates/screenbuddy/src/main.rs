use image::RgbaImage;
use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

#[cfg(windows)]
use winapi::shared::windef::HWND;
#[cfg(windows)]
use winapi::um::winuser::*;

mod animation;
mod chat_window;
mod composite_renderer;
mod settings_window;

use animation::SpriteAnimator;
use chat_window::{ChatWindow, ChatWindowEvent};
use composite_renderer::{CompositeRenderer, RenderableCreature};
use settings_window::{SettingsEvent, SettingsWindow, SettingValue};

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
const PIXEL_WIZARD_CELEBRATE: &[u8] = include_bytes!("../assets/generated/pixel_wizard_celebrate.png");

const COSMIC_JELLYFISH_IDLE: &[u8] = include_bytes!("../assets/generated/cosmic_jellyfish_idle.png");
const COSMIC_JELLYFISH_WALK: &[u8] = include_bytes!("../assets/generated/cosmic_jellyfish_walk.png");
const COSMIC_JELLYFISH_FLY: &[u8] = include_bytes!("../assets/generated/cosmic_jellyfish_fly.png");
const COSMIC_JELLYFISH_SLEEP: &[u8] = include_bytes!("../assets/generated/cosmic_jellyfish_sleep.png");
const COSMIC_JELLYFISH_CELEBRATE: &[u8] = include_bytes!("../assets/generated/cosmic_jellyfish_celebrate.png");

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
enum AnimState { Idle, Walk, Fly, Sleep, Celebrate }

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
        idle: &[u8],
        walk: &[u8],
        fly: &[u8],
        sleep: &[u8],
        celebrate: &[u8],
        frame_size: u32,
        frames: usize,
        x: f32,
        y: f32,
        personality: screenbuddy_core::CreaturePersonality,
    ) -> Result<Self, String> {
        let mut sheets = HashMap::new();
        sheets.insert(AnimState::Idle, Arc::new(image::load_from_memory(idle).map_err(|e| format!("idle: {}", e))?.to_rgba8()));
        sheets.insert(AnimState::Walk, Arc::new(image::load_from_memory(walk).map_err(|e| format!("walk: {}", e))?.to_rgba8()));
        sheets.insert(AnimState::Fly, Arc::new(image::load_from_memory(fly).map_err(|e| format!("fly: {}", e))?.to_rgba8()));
        sheets.insert(AnimState::Sleep, Arc::new(image::load_from_memory(sleep).map_err(|e| format!("sleep: {}", e))?.to_rgba8()));
        sheets.insert(AnimState::Celebrate, Arc::new(image::load_from_memory(celebrate).map_err(|e| format!("celebrate: {}", e))?.to_rgba8()));

        let animator = SpriteAnimator::new(frames, 30.0);
        let physics = screenbuddy_core::CreaturePhysics::new(x, y, frame_size).with_personality(personality);

        Ok(Self { id, name, sheets, animator, physics, current_state: AnimState::Idle, frame_size, frames, visible: true })
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
    println!("AI engine: {} models available", ai.available_models().len());

    use screenbuddy_core::CreaturePersonality;

    let creature_defs = vec![
        ("companion-bird-01", "Companion Bird", BIRD_IDLE, BIRD_WALK, BIRD_FLY, BIRD_SLEEP, BIRD_CELEBRATE, CreaturePersonality::Curious),
        ("robo-cat-01", "Robo-Cat", ROBO_CAT_IDLE, ROBO_CAT_WALK, ROBO_CAT_FLY, ROBO_CAT_SLEEP, ROBO_CAT_CELEBRATE, CreaturePersonality::Energetic),
        ("slime-king-01", "Slime King", SLIME_KING_IDLE, SLIME_KING_WALK, SLIME_KING_FLY, SLIME_KING_SLEEP, SLIME_KING_CELEBRATE, CreaturePersonality::Lazy),
        ("pixel-wizard-01", "Pixel Wizard", PIXEL_WIZARD_IDLE, PIXEL_WIZARD_WALK, PIXEL_WIZARD_FLY, PIXEL_WIZARD_SLEEP, PIXEL_WIZARD_CELEBRATE, CreaturePersonality::Neutral),
        ("cosmic-jellyfish-01", "Cosmic Jellyfish", COSMIC_JELLYFISH_IDLE, COSMIC_JELLYFISH_WALK, COSMIC_JELLYFISH_FLY, COSMIC_JELLYFISH_SLEEP, COSMIC_JELLYFISH_CELEBRATE, CreaturePersonality::Shy),
        ("dragon-01", "Dragon", DRAGON_IDLE, DRAGON_WALK, DRAGON_FLY, DRAGON_SLEEP, DRAGON_CELEBRATE, CreaturePersonality::Neutral),
        ("ghost-01", "Ghost", GHOST_IDLE, GHOST_WALK, GHOST_FLY, GHOST_SLEEP, GHOST_CELEBRATE, CreaturePersonality::Shy),
    ];

    let mut creatures: HashMap<String, CreatureInstance> = HashMap::new();

    for (i, (id, name, idle, walk, fly, sleep, celebrate, personality)) in creature_defs.iter().enumerate() {
        let x = 50.0 + (i as f32 * 110.0);
        let y = 100.0 + (i as f32 * 70.0);
        match CreatureInstance::from_bytes(id.to_string(), name.to_string(), idle, walk, fly, sleep, celebrate, 128, 4, x, y, *personality) {
            Ok(creature) => { println!("Loaded: {} ({})", name, id); creatures.insert(id.to_string(), creature); }
            Err(e) => eprintln!("Failed {}: {}", id, e),
        }
    }

    if creatures.is_empty() { eprintln!("No creatures!"); std::thread::sleep(Duration::from_secs(5)); return; }
    println!("Loaded {} creatures with 5 animations each", creatures.len());

    let (hwnd_tx, hwnd_rx) = mpsc::channel::<isize>();
    thread::spawn(move || { create_window_and_run(hwnd_tx, 800, 600); });

    let hwnd = match hwnd_rx.recv() { Ok(h) => h, Err(_) => { eprintln!("Window failed"); return; } };

    let mut renderer = CompositeRenderer::new(hwnd as *mut core::ffi::c_void, 800, 600);
    let frame_duration = Duration::from_millis(1000 / 30);
    let start_time = Instant::now();
    println!("Animation at 30 FPS");

    let mut tray = screenbuddy_core::SystemTray::new();
    tray.start().ok();
    let mut chat = screenbuddy_core::ChatOverlay::new();
    chat.start().ok();
    let screen = screenbuddy_core::ScreenManager::new();
    audio.play("celebration").ok();

    let mut active_idx: usize = 0;
    let creature_ids: Vec<String> = creatures.keys().cloned().collect();
    let states = [AnimState::Idle, AnimState::Walk, AnimState::Fly, AnimState::Sleep, AnimState::Celebrate];
    let mut state_idx: usize = 0;

    // AI Chat - with console input
    let ai_clone = screenbuddy_core::AiEngine::new(config.get().ai.clone());
    let chat_clone = screenbuddy_core::ChatOverlay::new();
    let _ai_bridge = screenbuddy_core::AiChatBridge::new(ai_clone, chat_clone);
    _ai_bridge.start().ok();
    
    // Spawn stdin reader for chat input
    let (input_tx, input_rx) = std::sync::mpsc::channel::<String>();
    let input_tx2 = input_tx.clone();
    thread::spawn(move || {
        use std::io::{self, Write};
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        loop {
            line.clear();
            if io::stdin().read_line(&mut line).is_err() { break; }
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if trimmed == "/quit" { break; }
                input_tx2.send(trimmed.to_string()).ok();
            }
            print!("> ");
            io::stdout().flush().ok();
        }
    });
    
    // Clone bridge for the input handler
    let bridge_for_input = _ai_bridge.clone();

    // Mouse interaction
    let mut cursor_pos: Option<glam::Vec2> = None;
    let mut dragged_creature: Option<usize> = None;

    // RAG Pipeline - memory system
    let mut rag = screenbuddy_core::RagPipeline::new();
    let _ = rag.ingest_document(
        std::path::Path::new("memory://startup"),
        "ScreenBuddy is an AI desktop companion. It has 7 creatures with unique personalities. The user can interact with them via click, drag, and chat."
    );
    println!("[RAG] Memory system initialized");

    // Agent Runtime - agentic capabilities
    let agent = screenbuddy_core::AgentRuntime::new(std::sync::Arc::new(ai.clone()));
    let mut agent_rx = agent.subscribe();
    println!("[Agent] Runtime initialized with {} tools", agent.tool_count());

    // Settings UI
    let mut settings = screenbuddy_core::SettingsUI::new();
    settings.set("creature_count", screenbuddy_core::SettingValue::Int(creatures.len() as i64)).ok();
    settings.set("collision_avoidance", screenbuddy_core::SettingValue::Bool(1.0)).ok();
    settings.set("cursor_interaction", screenbuddy_core::SettingValue::Bool(1.0)).ok();
    println!("[Settings] {} settings loaded", settings.export().len());
    
    // Apply initial settings to audio
    if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.get("volume_master") {
        audio.set_master_volume(vol as f32);
    }

    // Per-Pet Windows
    let mut pet_windows = screenbuddy_core::PerPetWindowManager::new();
    for (i, (id, creature)) in creatures.iter().enumerate() {
        let x = 50 + (i as i32 * 150);
        let y = 100 + (i as i32 * 100);
        pet_windows.create_window(&creature.name, x, y);
    }
    println!("[PerPet] {} pet windows created", pet_windows.window_count());
    
    // Chat Window
    let mut chat_win = ChatWindow::new();
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
        println!("[PerPet] Created window for {} at ({}, {})", creature.name, x, y);
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
                if chat_messages.iter().all(|(r, t)| !(r.contains(&format!("{:?}", msg.role).to_lowercase()) && t == &msg.content)) {
                    if msg.role == screenbuddy_core::ChatMsgRole::Assistant || msg.role == screenbuddy_core::ChatMsgRole::User {
                        let role_str = format!("{:?}", msg.role).to_lowercase();
                        chat_win.add_message(&role_str, &msg.content);
                    }
                }
            }
        }

        // State machine
        let state_elapsed = (elapsed.as_secs() % 60) / 5;
        let new_state_idx = (state_elapsed as usize) % states.len();
        if new_state_idx != state_idx {
            state_idx = new_state_idx;
            for creature in creatures.values_mut() {
                creature.set_state(states[state_idx]);
            }
            println!("State: {:?}", states[state_idx]);
            match states[state_idx] {
                AnimState::Idle => { audio.play("idle_hum").ok(); }
                AnimState::Walk => { audio.play("walk_step").ok(); }
                AnimState::Fly => { audio.play("celebrate").ok(); }
                AnimState::Sleep => { audio.play("snore").ok(); }
                AnimState::Celebrate => { audio.play("celebrate").ok(); }
            }
        }

        // Collect positions for collision avoidance
        let all_positions: Vec<glam::Vec2> = creatures.values().map(|c| c.physics.position).collect();

        // Update creatures
        for (i, creature) in creatures.values_mut().enumerate() {
            let cursor = if dragged_creature == Some(i) { None } else { cursor_pos };
            creature.physics.update(dt, &screen, cursor, &all_positions);
            creature.animator.update(elapsed.as_secs_f32());
        }

        // Render
        let mut renderables: Vec<RenderableCreature> = Vec::new();
        let mut frame_indices: Vec<usize> = Vec::new();

        for creature in creatures.values() {
            if !creature.visible { continue; }
            let sheet = creature.sheets.get(&creature.current_state).unwrap_or_else(|| creature.sheets.get(&AnimState::Idle).unwrap());
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
            if let Some(screenbuddy_core::SettingValue::Float(vol)) = settings.get("volume_master") {
                audio.set_master_volume(vol as f32);
            }
            if let Some(screenbuddy_core::SettingValue::Float(speed)) = settings.get("animation_speed") {
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
                        if let Some(c) = creatures.get(id) { println!("Active: {}", c.name); }
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
                    println!("[About] ScreenBuddy v0.1.0 - AI Desktop Companion");
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

        // Forward AI responses to chat window
        {
            let messages = chat_win.messages().to_vec();
            if let Some((role, text)) = messages.last() {
                if role == "assistant" {
                    // Already displayed
                }
            }
        }

        std::thread::sleep(frame_duration);
    }

    println!("ScreenBuddy shutdown");
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
            hCursor: winapi::um::winuser::LoadCursorW(ptr::null_mut(), winapi::um::winuser::IDC_ARROW),
            lpszClassName: class_name.as_ptr(),
            ..mem::zeroed()
        };
        winapi::um::winuser::RegisterClassExW(&wc);

        let ex_style = winapi::um::winuser::WS_EX_LAYERED | winapi::um::winuser::WS_EX_TOPMOST | winapi::um::winuser::WS_EX_TOOLWINDOW;
        let hwnd = winapi::um::winuser::CreateWindowExW(
            ex_style, class_name.as_ptr(), window_title.as_ptr(),
            winapi::um::winuser::WS_POPUP | winapi::um::winuser::WS_VISIBLE,
            100, 100, width, height,
            ptr::null_mut(), ptr::null_mut(), h_instance, ptr::null_mut(),
        );
        if hwnd.is_null() { eprintln!("Window failed"); return; }
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
        while winapi::um::winuser::PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, winapi::um::winuser::PM_REMOVE) != 0 {
            winapi::um::winuser::TranslateMessage(&msg);
            winapi::um::winuser::DispatchMessageW(&msg);
        }
    }
}

fn create_pet_window(creature_name: &str, x: i32, y: i32) -> *mut core::ffi::c_void {
    unsafe {
        let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());
        let class_name: Vec<u16> = format!("ScreenBuddyPet{}\0", x).encode_utf16().collect();
        let window_title: Vec<u16> = format!("{} - ScreenBuddy\0", creature_name).encode_utf16().collect();

        let wc = winapi::um::winuser::WNDCLASSEXW {
            cbSize: mem::size_of::<winapi::um::winuser::WNDCLASSEXW>() as u32,
            style: winapi::um::winuser::CS_HREDRAW | winapi::um::winuser::CS_VREDRAW,
            lpfnWndProc: Some(pet_window_proc),
            hInstance: h_instance,
            hCursor: winapi::um::winuser::LoadCursorW(ptr::null_mut(), winapi::um::winuser::IDC_ARROW),
            lpszClassName: class_name.as_ptr(),
            ..mem::zeroed()
        };
        winapi::um::winuser::RegisterClassExW(&wc);

        let ex_style = winapi::um::winuser::WS_EX_LAYERED | winapi::um::winuser::WS_EX_TOPMOST | winapi::um::winuser::WS_EX_TOOLWINDOW;
        let hwnd = winapi::um::winuser::CreateWindowExW(
            ex_style, class_name.as_ptr(), window_title.as_ptr(),
            winapi::um::winuser::WS_POPUP | winapi::um::winuser::WS_VISIBLE,
            x, y, 128, 128,
            ptr::null_mut(), ptr::null_mut(), h_instance, ptr::null_mut(),
        );
        hwnd as *mut core::ffi::c_void
    }
}

unsafe extern "system" fn pet_window_proc(hwnd: winapi::shared::windef::HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        winapi::um::winuser::WM_DESTROY => { winapi::um::winuser::PostQuitMessage(0); 0 }
        winapi::um::winuser::WM_LBUTTONDOWN => { 
            winapi::um::winuser::SendMessageW(hwnd, winapi::um::winuser::WM_NCLBUTTONDOWN, winapi::um::winuser::HTCAPTION as usize, 0); 0 
        }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe extern "system" fn window_proc(hwnd: winapi::shared::windef::HWND, msg: u32, wparam: usize, lparam: isize) -> isize {
    match msg {
        winapi::um::winuser::WM_DESTROY => { winapi::um::winuser::PostQuitMessage(0); 0 }
        winapi::um::winuser::WM_LBUTTONDOWN => { winapi::um::winuser::SendMessageW(hwnd, winapi::um::winuser::WM_NCLBUTTONDOWN, winapi::um::winuser::HTCAPTION as usize, 0); 0 }
        _ => winapi::um::winuser::DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
