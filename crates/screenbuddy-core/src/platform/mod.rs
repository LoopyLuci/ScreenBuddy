//! Cross-platform windowing and transparency support.
//!
//! # Status
//!
//! **The platform backends here are stubs and are not used by the application.**
//! Each [`Platform`] implementation's `create_window` returns a null handle and
//! every other method is a no-op, so this trait currently provides type shapes
//! rather than working windows. Nothing in the workspace calls
//! [`create_window`], [`poll_events`] or [`swap_buffers`] -- the desktop app is
//! Win32-only (`crates/screenbuddy/src/*.rs`).
//!
//! They are kept because the trait documents the intended cross-platform shape,
//! but nothing should treat a call into this module as actually creating a
//! window. Implementing these for real requires winit or wgpu's surface API;
//! until then, prefer the native Win32 path in the binary crate.
//!
//! Each backend reports whether it is implemented, so a caller can fail loudly
//! instead of silently getting a window that does nothing.

use crate::error::Result;
use glam::Vec2;

/// Window configuration options
#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub position: Vec2,
    pub transparent: bool,
    pub always_on_top: bool,
    pub click_through: bool,
    pub resizable: bool,
    pub decorations: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "ScreenBuddy".to_string(),
            width: 256,
            height: 256,
            position: Vec2::ZERO,
            transparent: true,
            always_on_top: true,
            click_through: false,
            resizable: false,
            decorations: false,
        }
    }
}

/// Window event types
#[derive(Debug, Clone)]
pub enum WindowEvent {
    Resized { width: u32, height: u32 },
    Moved { x: f32, y: f32 },
    CloseRequest,
    MouseMove { x: f32, y: f32 },
    MouseDown { button: MouseButton, x: f32, y: f32 },
    MouseUp { button: MouseButton, x: f32, y: f32 },
    KeyDown { key: String },
    KeyUp { key: String },
    FocusGained,
    FocusLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// Opaque handle to a window
pub struct WindowHandle {
    pub inner: *mut std::ffi::c_void,
    pub id: u64,
}

impl WindowHandle {
    pub fn new(inner: *mut std::ffi::c_void, id: u64) -> Self {
        Self { inner, id }
    }
}

/// Platform abstraction trait
pub trait Platform {
    /// Whether this backend actually creates working windows.
    ///
    /// False for every backend today. Reported explicitly so a caller cannot
    /// mistake a null handle for a real window.
    const IMPLEMENTED: bool = false;

    fn init() -> Result<()>
    where
        Self: Sized;

    fn create_window(config: &WindowConfig) -> Result<WindowHandle>;
    fn destroy_window(handle: &WindowHandle);
    fn set_window_position(handle: &WindowHandle, pos: Vec2);
    fn set_window_size(handle: &WindowHandle, width: u32, height: u32);
    fn set_window_alpha(handle: &WindowHandle, alpha: f32);
    fn set_click_through(handle: &WindowHandle, enabled: bool);
    fn focus_window(handle: &WindowHandle);
    fn poll_events(handle: &WindowHandle) -> Vec<WindowEvent>;
    fn request_redraw(handle: &WindowHandle);
    fn swap_buffers(handle: &WindowHandle);
}

// Platform-specific implementations
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "linux")]
pub mod linux;

/// Whether any real window backend exists on this platform.
pub const fn windows_available() -> bool {
    #[cfg(target_os = "windows")]
    {
        <windows::WindowsPlatform as Platform>::IMPLEMENTED
    }
    #[cfg(target_os = "macos")]
    {
        <macos::MacPlatform as Platform>::IMPLEMENTED
    }
    #[cfg(target_os = "linux")]
    {
        <linux::LinuxPlatform as Platform>::IMPLEMENTED
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        false
    }
}

/// Create a platform-appropriate window.
///
/// Returns an error rather than a null handle on platforms whose backend is not
/// implemented, so a caller cannot mistake a stub for a working window.
pub fn create_window(config: &WindowConfig) -> Result<WindowHandle> {
    if !windows_available() {
        return Err(crate::error::Error::InvalidConfig(
            "no window backend is implemented on this platform; the desktop app \
             uses the native Win32 path"
                .into(),
        ));
    }
    #[cfg(target_os = "windows")]
    return windows::WindowsPlatform::create_window(config);

    #[cfg(target_os = "macos")]
    return macos::MacPlatform::create_window(config);

    #[cfg(target_os = "linux")]
    return linux::LinuxPlatform::create_window(config);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    compile_error!("Unsupported platform");
}

/// Poll events for the given window
pub fn poll_events(handle: &WindowHandle) -> Vec<WindowEvent> {
    #[cfg(target_os = "windows")]
    return windows::WindowsPlatform::poll_events(handle);

    #[cfg(target_os = "macos")]
    return macos::MacPlatform::poll_events(handle);

    #[cfg(target_os = "linux")]
    return linux::LinuxPlatform::poll_events(handle);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    compile_error!("Unsupported platform");
}

/// Swap buffers for the given window
pub fn swap_buffers(handle: &WindowHandle) {
    #[cfg(target_os = "windows")]
    return windows::WindowsPlatform::swap_buffers(handle);

    #[cfg(target_os = "macos")]
    return macos::MacPlatform::swap_buffers(handle);

    #[cfg(target_os = "linux")]
    return linux::LinuxPlatform::swap_buffers(handle);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    compile_error!("Unsupported platform");
}
