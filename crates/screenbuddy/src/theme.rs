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

    /// A high-contrast theme for users who need maximum legibility: pure black
    /// background, pure white body text and yellow accents, with strong borders.
    /// Contrast ratios here are far above the WCAG AA floor.
    pub fn high_contrast() -> Self {
        Self {
            name: "high_contrast",
            bg: 0x00_00_00_00,
            header_bg: 0x00_00_00_00,
            sidebar_bg: 0x00_00_00_00,
            row_bg: 0x00_00_00_00,
            row_active_bg: 0x00_00_40_60,
            input_bg: 0x00_00_00_00,
            // Bright, saturated borders so every panel edge is unambiguous.
            border: 0x00_FF_FF_FF,
            text: 0x00_FF_FF_FF,
            muted_text: 0x00_FF_FF_80,
            accent: 0x00_00_FF_FF,
            error: 0x00_00_00_FF,
            font_family: "Segoe UI",
            font_size_normal: 15,
            font_size_small: 12,
            font_size_large: 19,
            padding: 10,
            row_height: 54,
        }
    }

    /// Resolve a theme by the name stored in the `theme` setting. An unknown
    /// name falls back to dark rather than leaving the window unstyled.
    pub fn from_name(name: &str) -> Self {
        match name.trim().to_ascii_lowercase().as_str() {
            "light" => Self::light(),
            "high_contrast" | "high-contrast" | "contrast" => Self::high_contrast(),
            _ => Self::dark(),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Relative luminance, used to check contrast rather than eyeball it.
    fn luminance(c: u32) -> f64 {
        let f = |shift: u32| {
            let v = ((c >> shift) & 0xFF) as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * f(0) + 0.7152 * f(8) + 0.0722 * f(16)
    }

    fn contrast(a: u32, b: u32) -> f64 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    #[test]
    fn from_name_resolves_every_offered_option() {
        assert_eq!(Theme::from_name("dark").name, "dark");
        assert_eq!(Theme::from_name("light").name, "light");
        assert_eq!(Theme::from_name("high_contrast").name, "high_contrast");
    }

    #[test]
    fn from_name_is_case_and_whitespace_insensitive() {
        assert_eq!(Theme::from_name("  LIGHT ").name, "light");
        assert_eq!(Theme::from_name("High-Contrast").name, "high_contrast");
    }

    #[test]
    fn an_unknown_theme_name_falls_back_to_dark() {
        assert_eq!(Theme::from_name("chartreuse").name, "dark");
        assert_eq!(Theme::from_name("").name, "dark");
    }

    #[test]
    fn high_contrast_clears_the_wcag_aa_threshold_for_body_text() {
        let hc = Theme::high_contrast();
        let ratio = contrast(hc.text, hc.bg);
        assert!(
            ratio >= 7.0,
            "body text contrast {ratio:.2}:1 is below the AAA floor of 7:1"
        );
    }

    #[test]
    fn high_contrast_beats_the_dark_theme_on_body_text() {
        let hc = Theme::high_contrast();
        let dark = Theme::dark();
        assert!(
            contrast(hc.text, hc.bg) > contrast(dark.text, dark.bg),
            "the high-contrast theme must actually be higher contrast"
        );
    }

    #[test]
    fn high_contrast_muted_text_is_still_readable() {
        let hc = Theme::high_contrast();
        let ratio = contrast(hc.muted_text, hc.bg);
        assert!(ratio >= 4.5, "muted text contrast {ratio:.2}:1 is below AA");
    }
}
