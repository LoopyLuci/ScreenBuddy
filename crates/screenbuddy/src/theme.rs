//! Theme system for ScreenBuddy.
//!
//! Centralizes all visual constants so the palette can be switched between
//! dark and light modes, and so changing a color touches one file instead of
//! every window proc.

/// A complete visual theme: colors, fonts, and spacing.
#[derive(Clone, Debug)]
pub struct Theme {
    pub name: &'static str,
    pub bg: u32,
    pub header_bg: u32,
    pub sidebar_bg: u32,
    pub row_bg: u32,
    pub row_active_bg: u32,
    pub input_bg: u32,
    pub border: u32,
    pub text: u32,
    pub muted_text: u32,
    pub accent: u32,
    pub error: u32,
    pub font_family: &'static str,
    pub font_size_normal: i32,
    pub font_size_small: i32,
    pub font_size_large: i32,
    pub padding: i32,
    pub row_height: i32,
}

impl Theme {
    /// The default dark theme.
    pub fn dark() -> Self {
        Self {
            name: "dark",
            bg: 0x00_14_12_12,
            header_bg: 0x00_23_1F_1F,
            sidebar_bg: 0x00_1A_18_18,
            row_bg: 0x00_1E_1C_1C,
            row_active_bg: 0x00_30_4A_50,
            input_bg: 0x00_22_22_33,
            border: 0x00_3A_3A_4A,
            text: 0x00_F0_E6_D8,
            muted_text: 0x00_A8_C0_C8,
            accent: 0x00_C0_86_5A,
            error: 0x00_F0_40_40,
            font_family: "Segoe UI",
            font_size_normal: 14,
            font_size_small: 11,
            font_size_large: 18,
            padding: 10,
            row_height: 54,
        }
    }

    /// A light theme for bright environments.
    pub fn light() -> Self {
        Self {
            name: "light",
            bg: 0x00_F5_F5_F5,
            header_bg: 0x00_E8_E8_E8,
            sidebar_bg: 0x00_EC_EC_EC,
            row_bg: 0x00_F0_F0_F0,
            row_active_bg: 0x00_D0_E8_F0,
            input_bg: 0x00_FF_FF_FF,
            border: 0x00_C0_C0_C0,
            text: 0x00_20_20_20,
            muted_text: 0x00_60_60_60,
            accent: 0x00_20_60_A0,
            error: 0x00_C0_20_20,
            font_family: "Segoe UI",
            font_size_normal: 14,
            font_size_small: 11,
            font_size_large: 18,
            padding: 10,
            row_height: 54,
        }
    }

    /// Get the active theme based on the setting value.
    pub fn from_setting(dark_mode: bool) -> Self {
        if dark_mode {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Background color as a GDI COLORREF (0x00BBGGRR).
    pub fn bg_colorref(&self) -> u32 {
        self.bg
    }

    /// Text color as a GDI COLORREF.
    pub fn text_colorref(&self) -> u32 {
        self.text
    }

    /// Create a GDI font at the given size in points.
    pub fn create_font(&self, size: i32) -> Option<*mut core::ffi::c_void> {
        unsafe {
            let family: Vec<u16> = self.font_family
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let hfont = winapi::um::wingdi::CreateFontW(
                size, 0, 0, 0,
                winapi::um::wingdi::FW_NORMAL as i32,
                0, 0, 0,
                winapi::um::wingdi::DEFAULT_CHARSET as u32,
                winapi::um::wingdi::OUT_DEFAULT_PRECIS as u32,
                winapi::um::wingdi::CLIP_DEFAULT_PRECIS as u32,
                winapi::um::wingdi::CLEARTYPE_QUALITY as u32,
                (winapi::um::wingdi::DEFAULT_PITCH | winapi::um::wingdi::FF_SWISS) as u32,
                family.as_ptr(),
            );
            if hfont.is_null() {
                None
            } else {
                Some(hfont as *mut core::ffi::c_void)
            }
        }
    }

    /// Create a solid brush from a theme color.
    pub fn create_brush(&self, color: u32) -> *mut core::ffi::c_void {
        unsafe {
            winapi::um::wingdi::CreateSolidBrush(color) as *mut core::ffi::c_void
        }
    }
}
