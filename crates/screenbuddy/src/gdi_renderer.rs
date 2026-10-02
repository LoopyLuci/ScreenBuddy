use image::RgbaImage;
use std::mem;
use std::ptr;
use winapi::shared::windef::{HBITMAP, HDC, POINT, RECT, SIZE};
use winapi::um::wingdi::{BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION};

pub struct GdiRenderer {
    width: u32,
    height: u32,
    pixel_buffer: Vec<u8>,
    h_bitmap: HBITMAP,
    h_dc: HDC,
    window_hwnd: *mut core::ffi::c_void,
    sprite_sheet: Option<RgbaImage>,
    frame_count: usize,
}

impl GdiRenderer {
    pub fn new(window_hwnd: *mut core::ffi::c_void, width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixel_buffer: vec![0u8; (width * height * 4) as usize],
            h_bitmap: ptr::null_mut(),
            h_dc: ptr::null_mut(),
            window_hwnd,
            sprite_sheet: None,
            frame_count: 1,
        }
    }

    pub fn load_sprite_sheet(&mut self, sprite_sheet: &RgbaImage, frame_count: usize) {
        self.sprite_sheet = Some(sprite_sheet.clone());
        self.frame_count = frame_count;
    }

    pub fn render_frame(&mut self, frame: usize) {
        let sprite_sheet = match &self.sprite_sheet {
            Some(s) => s, None => return,
        };
        let fw = sprite_sheet.width() as usize / self.frame_count;
        let fh = sprite_sheet.height() as usize;
        let ox = frame * fw;

        for i in (0..self.pixel_buffer.len()).step_by(4) {
            self.pixel_buffer[i] = 0;
            self.pixel_buffer[i+1] = 0;
            self.pixel_buffer[i+2] = 0;
            self.pixel_buffer[i+3] = 0;
        }

        let cw = fw.min(self.width as usize);
        let ch = fh.min(self.height as usize);
        for y in 0..ch {
            for x in 0..cw {
                let px = sprite_sheet.get_pixel((ox + x) as u32, y as u32);
                let a = px[3] as u32;
                if a == 0 { continue; }
                let idx = (y * self.width as usize + x) * 4;
                if idx + 3 >= self.pixel_buffer.len() { continue; }
                self.pixel_buffer[idx] = ((px[2] as u32 * a) / 255) as u8;
                self.pixel_buffer[idx+1] = ((px[1] as u32 * a) / 255) as u8;
                self.pixel_buffer[idx+2] = ((px[0] as u32 * a) / 255) as u8;
                self.pixel_buffer[idx+3] = a as u8;
            }
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
                self.h_dc, &bmi, winapi::um::wingdi::DIB_RGB_COLORS, &mut bits, ptr::null_mut(), 0,
            );
            if self.h_bitmap.is_null() { return; }
            ptr::copy_nonoverlapping(self.pixel_buffer.as_ptr(), bits as *mut u8, (self.width * self.height * 4) as usize);
            let old = winapi::um::wingdi::SelectObject(self.h_dc, self.h_bitmap as *mut _);
            let mut blend = BLENDFUNCTION {
                BlendOp: winapi::um::wingdi::AC_SRC_OVER as u8, BlendFlags: 0,
                SourceConstantAlpha: 255, AlphaFormat: winapi::um::wingdi::AC_SRC_ALPHA as u8,
            };
            let mut rect: RECT = mem::zeroed();
            winapi::um::winuser::GetWindowRect(self.window_hwnd as *mut _, &mut rect);
            let mut size = SIZE { cx: self.width as i32, cy: self.height as i32 };
            let mut src = POINT { x: 0, y: 0 };
            let mut dst = POINT { x: rect.left, y: rect.top };
            winapi::um::winuser::UpdateLayeredWindow(
                self.window_hwnd as *mut _, ptr::null_mut(), &mut dst, &mut size,
                self.h_dc, &mut src, 0, &mut blend, winapi::um::winuser::ULW_ALPHA,
            );
            winapi::um::wingdi::SelectObject(self.h_dc, old as *mut _);
        }
    }
}

impl Drop for GdiRenderer {
    fn drop(&mut self) {
        unsafe {
            if !self.h_bitmap.is_null() { winapi::um::wingdi::DeleteObject(self.h_bitmap as *mut _); }
            if !self.h_dc.is_null() { winapi::um::wingdi::DeleteDC(self.h_dc); }
        }
    }
}
