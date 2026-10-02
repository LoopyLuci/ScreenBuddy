//! Multi-Creature Composite Renderer for ScreenBuddy
//!
//! Renders multiple creatures into a single pixel buffer, then composites to screen.

use image::RgbaImage;
use std::mem;
use std::ptr;
use std::sync::Arc;
use winapi::shared::windef::{HBITMAP, HDC, POINT, RECT, SIZE};
use winapi::um::wingdi::{BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION};

/// A renderable creature instance (uses Arc to avoid per-frame clones)
pub struct RenderableCreature {
    pub sprite_sheet: Arc<RgbaImage>,
    pub frame_count: usize,
    pub x: f32,
    pub y: f32,
    pub scale: f32,
}

/// Renders multiple creatures into a single composite frame
pub struct CompositeRenderer {
    width: u32,
    height: u32,
    composite_buffer: Vec<u8>,
    h_bitmap: HBITMAP,
    h_dc: HDC,
    window_hwnd: *mut core::ffi::c_void,
}

impl CompositeRenderer {
    pub fn new(window_hwnd: *mut core::ffi::c_void, width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            composite_buffer: vec![0u8; (width * height * 4) as usize],
            h_bitmap: ptr::null_mut(),
            h_dc: ptr::null_mut(),
            window_hwnd,
        }
    }

    /// Clear the composite buffer to transparent
    fn clear_buffer(&mut self) {
        for i in (0..self.composite_buffer.len()).step_by(4) {
            self.composite_buffer[i] = 0;
            self.composite_buffer[i + 1] = 0;
            self.composite_buffer[i + 2] = 0;
            self.composite_buffer[i + 3] = 0;
        }
    }

    /// Draw a single creature into the composite buffer
    pub fn draw_creature(&mut self, creature: &RenderableCreature, frame: usize) {
        let sprite_sheet: &RgbaImage = &creature.sprite_sheet;
        let fw = sprite_sheet.width() as usize / creature.frame_count;
        let fh = sprite_sheet.height() as usize;
        let ox = frame * fw;

        let creature_w = (fw as f32 * creature.scale) as usize;
        let creature_h = (fh as f32 * creature.scale) as usize;

        let start_x = creature.x as i32;
        let start_y = creature.y as i32;

        for dy in 0..creature_h {
            let target_y = start_y + dy as i32;
            if target_y < 0 || target_y >= self.height as i32 {
                continue;
            }
            for dx in 0..creature_w {
                let target_x = start_x + dx as i32;
                if target_x < 0 || target_x >= self.width as i32 {
                    continue;
                }

                let src_x = ((dx as f32 / creature.scale) as usize).min(fw - 1);
                let src_y = ((dy as f32 / creature.scale) as usize).min(fh - 1);

                let px = sprite_sheet.get_pixel((ox + src_x) as u32, src_y as u32);
                let a = px[3] as u32;
                if a == 0 {
                    continue;
                }

                let idx = (target_y as usize * self.width as usize + target_x as usize) * 4;
                if idx + 3 >= self.composite_buffer.len() {
                    continue;
                }

                let src_r = px[0] as u32;
                let src_g = px[1] as u32;
                let src_b = px[2] as u32;

                let dst_r = self.composite_buffer[idx] as u32;
                let dst_g = self.composite_buffer[idx + 1] as u32;
                let dst_b = self.composite_buffer[idx + 2] as u32;
                let dst_a = self.composite_buffer[idx + 3] as u32;

                let out_a = a + (dst_a * (255 - a)) / 255;
                if out_a == 0 {
                    continue;
                }

                let out_r = (src_r * a + dst_r * dst_a * (255 - a) / 255) / out_a;
                let out_g = (src_g * a + dst_g * dst_a * (255 - a) / 255) / out_a;
                let out_b = (src_b * a + dst_b * dst_a * (255 - a) / 255) / out_a;

                self.composite_buffer[idx] = out_r.min(255) as u8;
                self.composite_buffer[idx + 1] = out_g.min(255) as u8;
                self.composite_buffer[idx + 2] = out_b.min(255) as u8;
                self.composite_buffer[idx + 3] = out_a.min(255) as u8;
            }
        }
    }

    /// Composited all creatures and present to screen
    pub fn present(&mut self, creatures: &[RenderableCreature], frames: &[usize]) {
        self.clear_buffer();

        for (creature, frame) in creatures.iter().zip(frames.iter()) {
            self.draw_creature(creature, *frame);
        }

        self.update_window();
    }

    fn update_window(&mut self) {
        unsafe {
            if self.h_dc.is_null() {
                self.h_dc = winapi::um::wingdi::CreateCompatibleDC(ptr::null_mut());
            }
            if !self.h_bitmap.is_null() {
                winapi::um::wingdi::DeleteObject(self.h_bitmap as *mut _);
            }
            let mut bmi: BITMAPINFO = mem::zeroed();
            bmi.bmiHeader.biSize = mem::size_of::<BITMAPINFOHEADER>() as u32;
            bmi.bmiHeader.biWidth = self.width as i32;
            bmi.bmiHeader.biHeight = -(self.height as i32);
            bmi.bmiHeader.biPlanes = 1;
            bmi.bmiHeader.biBitCount = 32;
            bmi.bmiHeader.biCompression = winapi::um::wingdi::BI_RGB;
            let mut bits: *mut core::ffi::c_void = ptr::null_mut();
            self.h_bitmap = winapi::um::wingdi::CreateDIBSection(
                self.h_dc,
                &bmi,
                winapi::um::wingdi::DIB_RGB_COLORS,
                &mut bits,
                ptr::null_mut(),
                0,
            );
            if self.h_bitmap.is_null() {
                return;
            }
            ptr::copy_nonoverlapping(
                self.composite_buffer.as_ptr(),
                bits as *mut u8,
                (self.width * self.height * 4) as usize,
            );
            let old = winapi::um::wingdi::SelectObject(self.h_dc, self.h_bitmap as *mut _);
            let mut blend = BLENDFUNCTION {
                BlendOp: winapi::um::wingdi::AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: winapi::um::wingdi::AC_SRC_ALPHA as u8,
            };
            let mut rect: RECT = mem::zeroed();
            winapi::um::winuser::GetWindowRect(self.window_hwnd as *mut _, &mut rect);
            let mut size = SIZE {
                cx: self.width as i32,
                cy: self.height as i32,
            };
            let mut src = POINT { x: 0, y: 0 };
            let mut dst = POINT {
                x: rect.left,
                y: rect.top,
            };
            winapi::um::winuser::UpdateLayeredWindow(
                self.window_hwnd as *mut _,
                ptr::null_mut(),
                &mut dst,
                &mut size,
                self.h_dc,
                &mut src,
                0,
                &mut blend,
                winapi::um::winuser::ULW_ALPHA,
            );
            winapi::um::wingdi::SelectObject(self.h_dc, old as *mut _);
        }
    }
}

impl Drop for CompositeRenderer {
    fn drop(&mut self) {
        unsafe {
            if !self.h_bitmap.is_null() {
                winapi::um::wingdi::DeleteObject(self.h_bitmap as *mut _);
            }
            if !self.h_dc.is_null() {
                winapi::um::wingdi::DeleteDC(self.h_dc);
            }
        }
    }
}
