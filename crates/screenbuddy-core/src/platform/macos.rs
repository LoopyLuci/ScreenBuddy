use glam::Vec2;

use super::{Platform, WindowConfig, WindowEvent, WindowHandle};
use crate::error::Result;

pub struct MacPlatform;

impl Platform for MacPlatform {
    fn init() -> Result<()> {
        Ok(())
    }

    fn create_window(_config: &WindowConfig) -> Result<WindowHandle> {
        Ok(WindowHandle::new(std::ptr::null_mut(), 1))
    }

    fn destroy_window(_handle: &WindowHandle) {}
    fn set_window_position(_handle: &WindowHandle, _pos: Vec2) {}
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
