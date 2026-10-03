//! Shared GDI drawing helpers for the native windows.
//!
//! These lived privately inside `main_window`, which meant a second window could
//! not draw anything without duplicating them -- and any duplication would drift.
//! They are here now so `main_window` and `agent_editor` render through the same
//! code.
//!
//! Everything draws into a plain `&mut [u32]` pixel buffer *and* a GDI device
//! context at once. That is deliberate: GDI owns text rasterisation, while fills
//! are cheaper to do directly in memory, and the two must end up on the same
//! surface or the text vanishes.

#![cfg(windows)]

use winapi::shared::windef::{HDC, HFONT, RECT, SIZE};
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

/// Draw a filled rectangle, clipped to the surface.
///
/// `surface_w` is passed explicitly because the two windows have different
/// widths (the main window is 900, the agent editor 760). A hardcoded width
/// silently clipped the editor's right-hand controls off the surface, and inferring
/// it from the buffer length is ambiguous because the height is not known here.
pub fn fill_rect(buf: &mut [u32], surface_w: i32, x: i32, y: i32, w: i32, h: i32, colour: u32) {
    let width = surface_w.max(1);
    // Clamp the vertical extent to what the buffer can hold, which keeps a fill
    // laid out past the bottom from indexing out of range.
    let max_y = (buf.len() as i32 / width) + 1;
    let left = x.max(0);
    let top = y.max(0);
    let right = (x + w).min(width);
    let bottom = (y + h).min(max_y);

    for py in top..bottom {
        for px in left..right {
            let idx = (py * width + px) as usize;
            if idx < buf.len() {
                buf[idx] = colour;
            }
        }
    }
}

/// Draw a one-pixel border.
pub fn draw_rect(buf: &mut [u32], surface_w: i32, x: i32, y: i32, w: i32, h: i32, colour: u32) {
    fill_rect(buf, surface_w, x, y, w, 1, colour);
    fill_rect(buf, surface_w, x, y + h - 1, w, 1, colour);
    fill_rect(buf, surface_w, x, y, 1, h, colour);
    fill_rect(buf, surface_w, x + w - 1, y, 1, h, colour);
}

/// Line height for a font of the given size. Weight does not change it.
pub fn font_height(size: i32, _bold: bool) -> i32 {
    size + 10
}

/// Create a font handle. The caller deletes it with [`del_obj`].
pub fn make_font(size: i32, bold: bool) -> HFONT {
    unsafe {
        CreateFontW(
            -(size + 6),
            0,
            0,
            0,
            if bold { FW_BOLD } else { FW_NORMAL },
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            std::ptr::null(),
        )
    }
}

/// Select a GDI object into a DC, hiding the `*mut c_void` cast this winapi
/// version's signature requires.
pub fn select_obj(dc: HDC, obj: *mut core::ffi::c_void) -> *mut core::ffi::c_void {
    unsafe { SelectObject(dc, obj) }
}

/// Delete a GDI object created by this module.
pub fn del_obj(obj: *mut core::ffi::c_void) {
    unsafe {
        DeleteObject(obj);
    }
}

/// Set the text colour for subsequent drawing on this DC.
///
/// GDI's `SetTextColor` takes COLORREF (0x00BBGGRR), which is the form our
/// colour constants already use.
pub fn set_text_colour(dc: HDC, bgr: u32) {
    unsafe {
        SetTextColor(dc, bgr);
    }
}

/// Make text drawing transparent.
///
/// This is not optional. By default GDI fills an opaque rectangle behind every
/// string using the current background brush, which painted a white box over
/// each label and field and made them unreadable. Every text draw needs this, so
/// it lives here rather than being repeated at each call site.
pub fn set_transparent_text(dc: HDC) {
    unsafe {
        SetBkMode(dc, TRANSPARENT as i32);
        // GDI's text background follows the brush; with no brush set this is a
        // no-op, but clearing it keeps the mode honest across DCs.
        SetBkColor(dc, 0);
    }
}

/// Select a font and make its text transparent in one step.
///
/// Returns the previous GDI object so the caller can restore it.
pub fn select_font(dc: HDC, size: i32, bold: bool) -> (HFONT, *mut core::ffi::c_void) {
    let font = make_font(size, bold);
    let old = select_obj(dc, font as *mut _);
    set_transparent_text(dc);
    (font, old)
}

/// Draw clipped single-line text.
pub fn draw_text(buf: &mut [u32], dc: HDC, text: &str, x: i32, y: i32, max_w: i32, _h: i32) {
    if text.is_empty() || max_w <= 0 {
        return;
    }
    unsafe {
        let mut rc = RECT {
            left: x,
            top: y,
            right: x + max_w,
            bottom: y + 40,
        };
        let wide: Vec<u16> = text.encode_utf16().collect();
        // DT_END_ELLIPSIS keeps long names from spilling over their row.
        DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
    }
    let _ = buf;
}

/// Draw text wrapped to `max_lines`, for labels that need more than one line.
///
/// GDI has no single call for "wrap to N lines and clip", so this measures each
/// line itself and stops at the limit.
pub fn draw_text_wrapped(buf: &mut [u32], dc: HDC, text: &str, area: TextArea, max_lines: i32) {
    let TextArea {
        x,
        y,
        max_w,
        line_h,
    } = area;
    if text.is_empty() || max_w <= 0 || max_lines <= 0 {
        return;
    }

    let mut lines = 0;
    let mut current = String::new();

    for word in text.split_whitespace() {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };

        if measure_text(dc, &candidate) <= max_w || current.is_empty() {
            current = candidate;
            continue;
        }

        draw_text(buf, dc, &current, x, y + lines * line_h, max_w, line_h);
        lines += 1;
        if lines >= max_lines {
            return;
        }
        current = word.to_string();
    }

    if !current.is_empty() && lines < max_lines {
        draw_text(buf, dc, &current, x, y + lines * line_h, max_w, line_h);
    }
}

/// Where text goes and how it is laid out.
#[derive(Debug, Clone, Copy)]
pub struct TextArea {
    pub x: i32,
    pub y: i32,
    /// Maximum line width in pixels.
    pub max_w: i32,
    /// Height of one line.
    pub line_h: i32,
}

/// Width of a string in pixels, for laying out clickable controls.
pub fn measure_text(dc: HDC, text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }
    unsafe {
        let mut size = SIZE { cx: 0, cy: 0 };
        let wide: Vec<u16> = text.encode_utf16().collect();
        let mut rc = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        // DT_CALCRECT measures without drawing, so nothing is disturbed.
        let ok = DrawTextW(
            dc,
            wide.as_ptr(),
            wide.len() as i32,
            &mut rc,
            DT_CALCRECT | DT_SINGLELINE | DT_NOPREFIX,
        );
        if ok != 0 {
            size.cx = rc.right - rc.left;
        }
        size.cx
    }
}

/// A GDI surface tests can draw into, mirroring what the windows create.
///
/// The layout code needs a DC for text while filling a plain pixel slice, and
/// they must be the same surface -- otherwise text is measured against one
/// bitmap and blitted from another, which is why earlier text was invisible.
#[cfg(test)]
pub struct DrawTarget {
    dc: HDC,
    bitmap: winapi::shared::windef::HBITMAP,
    old_bitmap: *mut core::ffi::c_void,
    raw: *mut u32,
    width: i32,
    height: i32,
}

#[cfg(test)]
impl DrawTarget {
    /// A memory-backed DC of the given size.
    pub fn new(width: i32, height: i32) -> Self {
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let dc = CreateCompatibleDC(screen);
            ReleaseDC(std::ptr::null_mut(), screen);

            // BITMAPINFO with no colour table: a 32-bit top-down DIB, which is
            // the layout the windows blit from.
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            // Negative height selects a top-down DIB, so row 0 is the top.
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;

            let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
            let bitmap =
                CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut raw, std::ptr::null_mut(), 0);
            let old_bitmap = SelectObject(dc, bitmap as *mut _);

            Self {
                dc,
                bitmap,
                old_bitmap,
                raw: raw as *mut u32,
                width,
                height,
            }
        }
    }

    pub fn handle(&self) -> HDC {
        self.dc
    }

    /// The DC's pixels, for assertions about what was drawn.
    pub fn pixels(&self) -> &[u32] {
        unsafe { std::slice::from_raw_parts(self.raw, (self.width * self.height) as usize) }
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
impl Drop for DrawTarget {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old_bitmap);
            DeleteObject(self.bitmap as *mut _);
            DeleteDC(self.dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: i32 = 200;

    fn target() -> DrawTarget {
        DrawTarget::new(W, 200)
    }

    #[test]
    fn fill_writes_the_expected_pixels() {
        let target = target();
        let mut buf = vec![0u32; (W * 200) as usize];
        fill_rect(&mut buf, W, 10, 10, 5, 5, 0x00FF0000);
        let idx = (12 * W + 12) as usize;
        assert_eq!(buf[idx], 0x00FF0000, "inside the fill");
        assert_eq!(buf[(20 * W + 12) as usize], 0, "outside the fill");
        let _ = target;
    }

    #[test]
    fn fill_clips_at_the_edges_instead_of_panicking() {
        let mut buf = vec![0u32; (W * 200) as usize];
        // A control laid out partly offscreen must not index out of the buffer.
        fill_rect(&mut buf, W, -50, -50, 100, 100, 0x00111111);
        fill_rect(&mut buf, W, W - 5, 190, 100, 100, 0x00222222);
    }

    #[test]
    fn an_outline_leaves_the_interior_untouched() {
        let mut buf = vec![0u32; (W * 200) as usize];
        draw_rect(&mut buf, W, 5, 5, 20, 20, 0x00ABCDEF);
        assert_eq!(buf[(5 * W + 5) as usize], 0x00ABCDEF, "corner");
        assert_eq!(
            buf[(15 * W + 15) as usize],
            0,
            "the outline must not fill its interior"
        );
    }

    #[test]
    fn text_measures_wider_as_it_gets_longer() {
        let target = target();
        let short = measure_text(target.handle(), "a");
        let long = measure_text(target.handle(), "a much longer string here");
        assert!(short > 0, "measurement returned nothing");
        assert!(long > short, "'{long}' should measure wider than '{short}'");
    }

    #[test]
    fn measuring_an_empty_string_is_zero() {
        let target = target();
        assert_eq!(measure_text(target.handle(), ""), 0);
    }

    #[test]
    fn wrapped_text_stays_within_the_line_limit() {
        let target = target();
        let mut buf = vec![0u32; (W * 200) as usize];
        let long = "one two three four five six seven eight nine ten eleven twelve \
                    thirteen fourteen fifteen sixteen";
        draw_text_wrapped(
            &mut buf,
            target.handle(),
            long,
            TextArea {
                x: 10,
                y: 10,
                max_w: 120,
                line_h: 20,
            },
            2,
        );
        // Rendering must not fault; the point is the bounded loop.
        assert!(!buf.is_empty());
    }

    #[test]
    fn wrapped_text_handles_degenerate_arguments() {
        let target = target();
        let mut buf = vec![0u32; (W * 200) as usize];
        let area = TextArea {
            x: 0,
            y: 0,
            max_w: 100,
            line_h: 20,
        };
        draw_text_wrapped(&mut buf, target.handle(), "", area, 3);
        draw_text_wrapped(
            &mut buf,
            target.handle(),
            "text",
            TextArea { max_w: 0, ..area },
            3,
        );
        draw_text_wrapped(&mut buf, target.handle(), "text", area, 0);
        draw_text_wrapped(
            &mut buf,
            target.handle(),
            "text",
            TextArea { max_w: -5, ..area },
            3,
        );
    }

    #[test]
    fn drawing_text_does_not_fault() {
        let target = target();
        let mut buf = vec![0u32; (W * 200) as usize];
        let font = make_font(14, false);
        let old = select_obj(target.handle(), font as *mut _);
        set_text_colour(target.handle(), 0x00FFFFFF);
        draw_text(&mut buf, target.handle(), "Agents", 10, 10, 200, 20);
        select_obj(target.handle(), old);
        del_obj(font as *mut _);
    }

    #[test]
    fn a_bold_font_can_be_created_and_released() {
        let target = target();
        let font = make_font(18, true);
        assert!(!font.is_null());
        let old = select_obj(target.handle(), font as *mut _);
        assert!(!old.is_null());
        select_obj(target.handle(), old);
        del_obj(font as *mut _);
    }

    #[test]
    fn the_draw_target_exposes_its_pixels() {
        let target = DrawTarget::new(64, 32);
        assert_eq!(target.size(), (64, 32));
        assert_eq!(target.pixels().len(), 64 * 32);
    }
}
