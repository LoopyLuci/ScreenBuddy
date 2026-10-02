use std::sync::atomic::{AtomicU64, Ordering};

use super::{Platform, WindowConfig, WindowEvent, WindowHandle};
use crate::error::Result;

static NEXT_WINDOW_ID: AtomicU64 = AtomicU64::new(1);

pub struct WindowsPlatform;

impl Platform for WindowsPlatform {
    fn init() -> Result<()> {
        Ok(())
    }

    fn create_window(config: &WindowConfig) -> Result<WindowHandle> {
        let id = NEXT_WINDOW_ID.fetch_add(1, Ordering::SeqCst);
        Ok(WindowHandle::new(std::ptr::null_mut(), id))
    }

    fn destroy_window(_handle: &WindowHandle) {}
    fn set_window_position(_handle: &WindowHandle, _pos: glam::Vec2) {}
    fn set_window_size(_handle: &WindowHandle, _width: u32, _height: u32) {}
    fn set_window_alpha(_handle: &WindowHandle, _alpha: f32) {}
    fn set_click_through(_handle: &WindowHandle, _enabled: bool) {}
    fn focus_window(_handle: &WindowHandle) {}
    fn poll_events(_handle: &WindowHandle) -> Vec<WindowEvent> {
        Vec::new()
    }
    fn request_redraw(_handle: &WindowHandle) {}
    fn swap_buffers(_handle: &WindowHandle) {}
}
