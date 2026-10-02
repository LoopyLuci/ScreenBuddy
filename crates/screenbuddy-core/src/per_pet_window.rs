//! Per-Pet Window Manager
//!
//! Each creature gets its own OS-level transparent window.
//! This is the true desktop pet behavior.

use std::collections::HashMap;
use std::mem;
use std::ptr;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

/// Unique ID for each pet window
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PetWindowId(pub u64);

/// Configuration for a pet window
#[derive(Debug, Clone)]
pub struct PetWindowConfig {
    pub id: PetWindowId,
    pub creature_name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub opacity: f32,
    pub click_through: bool,
    pub always_on_top: bool,
}

impl PetWindowConfig {
    pub fn new(id: u64, creature_name: &str, x: i32, y: i32) -> Self {
        Self {
            id: PetWindowId(id),
            creature_name: creature_name.to_string(),
            x,
            y,
            width: 128,
            height: 128,
            visible: true,
            opacity: 1.0,
            click_through: false,
            always_on_top: true,
        }
    }
}

/// Events that can be sent to pet windows
#[derive(Debug, Clone)]
pub enum PetWindowEvent {
    Show(PetWindowId),
    Hide(PetWindowId),
    Move(PetWindowId, i32, i32),
    Resize(PetWindowId, u32, u32),
    SetOpacity(PetWindowId, f32),
    Close(PetWindowId),
    CloseAll,
    BringToFront(PetWindowId),
    SendToBack(PetWindowId),
}

/// Manages all pet windows
pub struct PerPetWindowManager {
    windows: HashMap<PetWindowId, PetWindowConfig>,
    event_sender: Sender<PetWindowEvent>,
    event_receiver: Arc<Mutex<Receiver<PetWindowEvent>>>,
    next_id: u64,
}

impl PerPetWindowManager {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            windows: HashMap::new(),
            event_sender: tx,
            event_receiver: Arc::new(Mutex::new(rx)),
            next_id: 1,
        }
    }

    pub fn create_window(&mut self, creature_name: &str, x: i32, y: i32) -> PetWindowId {
        let id = PetWindowId(self.next_id);
        self.next_id += 1;
        let config = PetWindowConfig::new(id.0, creature_name, x, y);
        self.windows.insert(id, config);
        println!("[PerPet] Created window for {} at ({}, {})", creature_name, x, y);
        id
    }

    pub fn close_window(&mut self, id: PetWindowId) -> bool {
        self.windows.remove(&id).is_some()
    }

    pub fn close_all(&mut self) {
        self.windows.clear();
    }

    pub fn get_window(&self, id: PetWindowId) -> Option<&PetWindowConfig> {
        self.windows.get(&id)
    }

    pub fn get_window_mut(&mut self, id: PetWindowId) -> Option<&mut PetWindowConfig> {
        self.windows.get_mut(&id)
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    pub fn send_event(&self, event: PetWindowEvent) {
        self.event_sender.send(event).ok();
    }

    pub fn poll_event(&self) -> Option<PetWindowEvent> {
        self.event_receiver.lock().unwrap().try_recv().ok()
    }

    pub fn process_events(&mut self) {
        while let Some(event) = self.poll_event() {
            match event {
                PetWindowEvent::Show(id) => {
                    if let Some(w) = self.windows.get_mut(&id) { w.visible = true; }
                }
                PetWindowEvent::Hide(id) => {
                    if let Some(w) = self.windows.get_mut(&id) { w.visible = false; }
                }
                PetWindowEvent::Move(id, x, y) => {
                    if let Some(w) = self.windows.get_mut(&id) { w.x = x; w.y = y; }
                }
                PetWindowEvent::Resize(id, width, height) => {
                    if let Some(w) = self.windows.get_mut(&id) { w.width = width; w.height = height; }
                }
                PetWindowEvent::SetOpacity(id, opacity) => {
                    if let Some(w) = self.windows.get_mut(&id) { w.opacity = opacity.clamp(0.0, 1.0); }
                }
                PetWindowEvent::Close(id) => { self.close_window(id); }
                PetWindowEvent::CloseAll => { self.close_all(); }
                PetWindowEvent::BringToFront(id) => {
                    if let Some(_w) = self.windows.get(&id) {
                        println!("[PerPet] Bring {} to front", id.0);
                    }
                }
                PetWindowEvent::SendToBack(id) => {
                    if let Some(_w) = self.windows.get(&id) {
                        println!("[PerPet] Send {} to back", id.0);
                    }
                }
            }
        }
    }

    pub fn get_all_windows(&self) -> &HashMap<PetWindowId, PetWindowConfig> {
        &self.windows
    }

    pub fn move_window(&mut self, id: PetWindowId, x: i32, y: i32) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.x = x;
            w.y = y;
        }
    }

    pub fn set_opacity(&mut self, id: PetWindowId, opacity: f32) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.opacity = opacity.clamp(0.0, 1.0);
        }
    }

    pub fn toggle_visibility(&mut self, id: PetWindowId) {
        if let Some(w) = self.windows.get_mut(&id) {
            w.visible = !w.visible;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_window() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Bird", 100, 100);
        assert_eq!(mgr.window_count(), 1);
        assert_eq!(mgr.get_window(id).unwrap().creature_name, "Bird");
    }

    #[test]
    fn test_close_window() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Cat", 200, 200);
        assert!(mgr.close_window(id));
        assert_eq!(mgr.window_count(), 0);
    }

    #[test]
    fn test_close_all() {
        let mut mgr = PerPetWindowManager::new();
        mgr.create_window("A", 0, 0);
        mgr.create_window("B", 0, 0);
        mgr.close_all();
        assert_eq!(mgr.window_count(), 0);
    }

    #[test]
    fn test_move_window() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Test", 0, 0);
        mgr.move_window(id, 500, 500);
        assert_eq!(mgr.get_window(id).unwrap().x, 500);
    }

    #[test]
    fn test_set_opacity() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Test", 0, 0);
        mgr.set_opacity(id, 0.5);
        assert_eq!(mgr.get_window(id).unwrap().opacity, 0.5);
    }

    #[test]
    fn test_opacity_clamp() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Test", 0, 0);
        mgr.set_opacity(id, 1.5);
        assert_eq!(mgr.get_window(id).unwrap().opacity, 1.0);
        mgr.set_opacity(id, -0.5);
        assert_eq!(mgr.get_window(id).unwrap().opacity, 0.0);
    }

    #[test]
    fn test_toggle_visibility() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Test", 0, 0);
        assert!(mgr.get_window(id).unwrap().visible);
        mgr.toggle_visibility(id);
        assert!(!mgr.get_window(id).unwrap().visible);
    }

    #[test]
    fn test_event_show_hide() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Test", 0, 0);
        mgr.send_event(PetWindowEvent::Hide(id));
        mgr.process_events();
        assert!(!mgr.get_window(id).unwrap().visible);
        mgr.send_event(PetWindowEvent::Show(id));
        mgr.process_events();
        assert!(mgr.get_window(id).unwrap().visible);
    }

    #[test]
    fn test_event_move() {
        let mut mgr = PerPetWindowManager::new();
        let id = mgr.create_window("Test", 0, 0);
        mgr.send_event(PetWindowEvent::Move(id, 300, 400));
        mgr.process_events();
        let w = mgr.get_window(id).unwrap();
        assert_eq!(w.x, 300);
        assert_eq!(w.y, 400);
    }

    #[test]
    fn test_multiple_windows() {
        let mut mgr = PerPetWindowManager::new();
        let ids: Vec<_> = (0..5).map(|i| mgr.create_window(&format!("C{}", i), i * 100, 0)).collect();
        assert_eq!(mgr.window_count(), 5);
        for id in ids {
            assert!(mgr.get_window(id).is_some());
        }
    }
}

// ============================================================================
// Windows implementation for per-pet transparent windows
// ============================================================================
#[cfg(windows)]
pub mod windows {
    use super::*;
    use std::mem;
    use std::ptr;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::Arc;
    use winapi::shared::windef::{HBITMAP, HDC, HWND, POINT, RECT, SIZE};
    use winapi::um::wingdi::{
        BLENDFUNCTION, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC,
        CreateDIBSection, DeleteDC, DeleteObject, DIB_RGB_COLORS, BI_RGB, SelectObject,
    };
    use winapi::um::winuser::*;

    /// Sprite sheet data for rendering creature frames
    #[derive(Clone)]
    pub struct SpriteSheetData {
        pub pixels: Arc<Vec<u8>>, // RGBA pixel data
        pub width: u32,
        pub height: u32,
        pub frame_count: usize,
    }

    impl SpriteSheetData {
        /// Create sprite sheet from raw RGBA bytes
        pub fn from_rgba(pixels: Vec<u8>, width: u32, height: u32, frame_count: usize) -> Self {
            Self {
                pixels: Arc::new(pixels),
                width,
                height,
                frame_count,
            }
        }
    }

    /// A transparent window for a single creature
    pub struct PetWindowHandle {
        pub id: PetWindowId,
        hwnd: HWND,
        hbitmap: HBITMAP,
        hdc_mem: HDC,
        hdc_screen: HDC,
        pixel_data_ptr: *mut core::ffi::c_void,
        width: u32,
        height: u32,
        running: Arc<AtomicBool>,
        current_frame: Arc<AtomicUsize>,
        sprite_sheet: Arc<Mutex<Option<SpriteSheetData>>>,
    }

    impl PetWindowHandle {
        /// Update the current animation frame to render
        pub fn set_frame(&self, frame: usize) {
            self.current_frame.store(frame, Ordering::SeqCst);
            unsafe {
                InvalidateRect(self.hwnd, ptr::null(), 1);
            }
        }

        /// Load a new sprite sheet for this pet
        pub fn set_sprite_sheet(&self, sheet: SpriteSheetData) {
            *self.sprite_sheet.lock().unwrap() = Some(sheet);
            unsafe {
                InvalidateRect(self.hwnd, ptr::null(), 1);
            }
        }
    }

    impl Drop for PetWindowHandle {
        fn drop(&mut self) {
            self.running.store(false, Ordering::SeqCst);
            unsafe {
                if !self.hbitmap.is_null() {
                    DeleteObject(self.hbitmap as _);
                }
                if !self.hdc_mem.is_null() {
                    DeleteDC(self.hdc_mem);
                }
                if !self.hwnd.is_null() {
                    DestroyWindow(self.hwnd);
                }
            }
        }
    }

    /// Create a transparent window for a creature
    pub fn create_pet_window(
        id: PetWindowId,
        creature_name: &str,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> Result<PetWindowHandle, String> {
        unsafe {
            let h_instance = winapi::um::libloaderapi::GetModuleHandleW(ptr::null_mut());

            // Register window class
            let class_name = format!("ScreenBuddyPet{}\0", id.0);
            let class_name_wide: Vec<u16> = class_name.encode_utf16().collect();
            let window_title = format!("{} - ScreenBuddy\0", creature_name);
            let window_title_wide: Vec<u16> = window_title.encode_utf16().collect();

            let wc = WNDCLASSEXW {
                cbSize: mem::size_of::<WNDCLASSEXW>() as u32,
                style: CS_HREDRAW | CS_VREDRAW,
                lpfnWndProc: Some(pet_window_proc),
                hInstance: h_instance,
                hCursor: LoadCursorW(ptr::null_mut(), IDC_ARROW),
                lpszClassName: class_name_wide.as_ptr(),
                ..mem::zeroed()
            };

            if RegisterClassExW(&wc) == 0 {
                // Class may already exist, continue
            }

            // Create layered window
            let ex_style = WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW;
            let hwnd = CreateWindowExW(
                ex_style,
                class_name_wide.as_ptr(),
                window_title_wide.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                x, y, width as i32, height as i32,
                ptr::null_mut(),
                ptr::null_mut(),
                h_instance,
                ptr::null_mut(),
            );

            if hwnd.is_null() {
                return Err("Failed to create pet window".to_string());
            }

            // Create memory DC for DIBSection
            let hdc_screen = GetDC(ptr::null_mut());
            let hdc_mem = CreateCompatibleDC(hdc_screen);

            // Create 32-bit DIBSection
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32), // Top-down
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..mem::zeroed()
                },
                ..mem::zeroed()
            };

            let mut pixel_data: *mut core::ffi::c_void = ptr::null_mut();
            let hbitmap = CreateDIBSection(
                hdc_screen,
                &bmi,
                DIB_RGB_COLORS,
                &mut pixel_data,
                ptr::null_mut(),
                0,
            );

            if hbitmap.is_null() {
                DeleteDC(hdc_mem);
                ReleaseDC(ptr::null_mut(), hdc_screen);
                DestroyWindow(hwnd);
                return Err("Failed to create DIBSection".to_string());
            }

            SelectObject(hdc_mem, hbitmap as _);
            ReleaseDC(ptr::null_mut(), hdc_screen);

            // Initial transparency setup
            let mut blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 1, // AC_SRC_ALPHA
            };

            let mut pt_dest = POINT { x: 0, y: 0 };
            let mut size = SIZE {
                cx: width as i32,
                cy: height as i32,
            };
            let mut pt_src = POINT { x: 0, y: 0 };

            UpdateLayeredWindow(
                hwnd,
                ptr::null_mut(),
                &mut pt_dest,
                &mut size,
                hdc_mem,
                &mut pt_src,
                0,
                &mut blend as *mut BLENDFUNCTION,
                ULW_ALPHA,
            );

            let handle = PetWindowHandle {
                id,
                hwnd,
                hbitmap,
                hdc_mem,
                hdc_screen: GetDC(ptr::null_mut()),
                pixel_data_ptr: pixel_data,
                width,
                height,
                running: Arc::new(AtomicBool::new(true)),
                current_frame: Arc::new(AtomicUsize::new(0)),
                sprite_sheet: Arc::new(Mutex::new(None)),
            };

            // Store handle pointer in window for message procedure
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, &handle as *const PetWindowHandle as isize);

            Ok(handle)
        }
    }

    /// Update the pet window's pixel buffer with a rendered frame
    pub fn update_pet_window_pixels(handle: &PetWindowHandle, pixels: &[u8]) -> Result<(), String> {
        unsafe {
            let pixel_count = (handle.width * handle.height * 4) as usize;
            if pixels.len() < pixel_count {
                return Err("Pixel buffer too small".to_string());
            }

            // Copy pixels to DIBSection
            ptr::copy_nonoverlapping(pixels.as_ptr(), handle.pixel_data_ptr as *mut u8, pixel_count);

            // Update layered window
            let mut blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: 1,
            };

            let mut size = SIZE { cx: handle.width as i32, cy: handle.height as i32 };
            let mut pt_src = POINT { x: 0, y: 0 };

            UpdateLayeredWindow(
                handle.hwnd,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
                handle.hdc_mem,
                &mut pt_src,
                0,
                &mut blend as *mut BLENDFUNCTION,
                ULW_ALPHA,
            );

            Ok(())
        }
    }

    /// Render the creature's current animation frame to the pet window
    pub fn render_pet_frame(handle: &PetWindowHandle) {
        unsafe {
            let sheet_opt = handle.sprite_sheet.lock().unwrap().clone();
            let sheet = match sheet_opt {
                Some(s) => s,
                None => return, // No sprite sheet loaded yet
            };

            let frame = handle.current_frame.load(Ordering::SeqCst);
            let fw = sheet.width as usize / sheet.frame_count;
            let fh = sheet.height as usize;
            let ox = frame * fw;

            let win_w = handle.width as usize;
            let win_h = handle.height as usize;
            let buf_size = win_w * win_h * 4;

            // Clear buffer to transparent
            if !handle.pixel_data_ptr.is_null() {
                ptr::write_bytes(handle.pixel_data_ptr as *mut u8, 0, buf_size);
            }

            // Calculate scaling to fit the sprite frame into the window
            let scale_x = handle.width as f32 / fw as f32;
            let scale_y = handle.height as f32 / fh as f32;
            let scale = scale_x.min(scale_y).min(1.0); // Don't upscale beyond 1:1

            let scaled_w = (fw as f32 * scale) as usize;
            let scaled_h = (fh as f32 * scale) as usize;

            // Center the sprite in the window
            let offset_x = (win_w - scaled_w) / 2;
            let offset_y = (win_h - scaled_h) / 2;

            let pixels = &sheet.pixels;
            let src = handle.pixel_data_ptr as *mut u8;

            for dy in 0..scaled_h {
                let target_y = offset_y + dy;
                if target_y >= win_h { continue; }

                for dx in 0..scaled_w {
                    let target_x = offset_x + dx;
                    if target_x >= win_w { continue; }

                    // Sample from sprite sheet
                    let src_x = ((dx as f32 / scale) as usize).min(fw - 1);
                    let src_y = ((dy as f32 / scale) as usize).min(fh - 1);
                    let src_idx = ((ox + src_x) * 4 + src_y * sheet.width as usize * 4) as usize;

                    if src_idx + 3 >= pixels.len() { continue; }

                    let a = pixels[src_idx + 3] as u32;
                    if a == 0 { continue; }

                    let r = pixels[src_idx] as u32;
                    let g = pixels[src_idx + 1] as u32;
                    let b = pixels[src_idx + 2] as u32;

                    let dst_idx = (target_y * win_w + target_x) * 4;
                    if dst_idx + 3 >= buf_size { continue; }

                    // Alpha blending (over existing pixel)
                    let dst_a = *src.add(dst_idx + 3) as u32;
                    let out_a = a + (dst_a * (255 - a)) / 255;
                    if out_a == 0 { continue; }

                    let dst_r = *src.add(dst_idx) as u32;
                    let dst_g = *src.add(dst_idx + 1) as u32;
                    let dst_b = *src.add(dst_idx + 2) as u32;

                    let out_r = (r * a + dst_r * dst_a * (255 - a) / 255) / out_a;
                    let out_g = (g * a + dst_g * dst_a * (255 - a) / 255) / out_a;
                    let out_b = (b * a + dst_b * dst_a * (255 - a) / 255) / out_a;

                    *src.add(dst_idx) = out_r.min(255) as u8;
                    *src.add(dst_idx + 1) = out_g.min(255) as u8;
                    *src.add(dst_idx + 2) = out_b.min(255) as u8;
                    *src.add(dst_idx + 3) = out_a.min(255) as u8;
                }
            }

            // Composite to screen
            let mut blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let mut size = SIZE { cx: handle.width as i32, cy: handle.height as i32 };
            let mut pt_src = POINT { x: 0, y: 0 };
            let mut rect: RECT = mem::zeroed();
            GetWindowRect(handle.hwnd, &mut rect);
            let mut dest_pt = POINT { x: rect.left, y: rect.top };

            UpdateLayeredWindow(
                handle.hwnd,
                ptr::null_mut(),
                &mut dest_pt,
                &mut size,
                handle.hdc_mem,
                &mut pt_src,
                0,
                &mut blend,
                ULW_ALPHA,
            );
        }
    }

    /// Move a pet window
    pub fn move_pet_window(handle: &PetWindowHandle, x: i32, y: i32) -> Result<(), String> {
        unsafe {
            SetWindowPos(
                handle.hwnd,
                ptr::null_mut(),
                x, y,
                0, 0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            Ok(())
        }
    }

    /// Set pet window opacity
    pub fn set_pet_window_opacity(handle: &PetWindowHandle, opacity: f32) -> Result<(), String> {
        unsafe {
            let alpha = (opacity.clamp(0.0, 1.0) * 255.0) as u8;
            let mut blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: 1,
            };

            let mut size = SIZE { cx: handle.width as i32, cy: handle.height as i32 };
            let mut pt_src = POINT { x: 0, y: 0 };

            UpdateLayeredWindow(
                handle.hwnd,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut size,
                handle.hdc_mem,
                &mut pt_src,
                0,
                &mut blend as *mut BLENDFUNCTION,
                ULW_ALPHA,
            );

            Ok(())
        }
    }

    /// Show/hide pet window
    pub fn show_pet_window(handle: &PetWindowHandle, show: bool) -> Result<(), String> {
        unsafe {
            let cmd = if show { SW_SHOW } else { SW_HIDE };
            ShowWindow(handle.hwnd, cmd);
            Ok(())
        }
    }

    /// Run message loop for pet windows (call from dedicated thread)
    pub fn run_pet_message_loop() {
        unsafe {
            let mut msg: MSG = mem::zeroed();
            while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    /// Window procedure for pet windows
    unsafe extern "system" fn pet_window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        match msg {
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            WM_PAINT => {
                // Retrieve handle from window data
                let handle_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const PetWindowHandle;
                if !handle_ptr.is_null() {
                    render_pet_frame(&*handle_ptr);
                }
                ValidateRect(hwnd, ptr::null());
                0
            }
            WM_LBUTTONDOWN => {
                SendMessageW(hwnd, WM_NCLBUTTONDOWN, HTCAPTION as usize, 0);
                0
            }
            WM_RBUTTONDOWN => {
                0
            }
            WM_TIMER => {
                // Animation timer - force repaint
                let handle_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const PetWindowHandle;
                if !handle_ptr.is_null() {
                    render_pet_frame(&*handle_ptr);
                }
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
