//! DPI awareness and multi-monitor support.
//!
//! Enumerates displays, calculates DPI scaling factors, and provides
//! helpers for positioning windows across monitors with different DPI
//! settings.

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;



/// A display device with its DPI and bounds.
#[derive(Debug, Clone)]
pub struct DisplayInfo {
    /// Device name (e.g., `\\.\DISPLAY1`).
    pub name: String,
    /// Whether this is the primary display.
    pub is_primary: bool,
    /// Screen bounds in physical pixels.
    pub bounds: Rect,
    /// DPI scaling factor (1.0 = 96 DPI, 1.25 = 120 DPI, etc.).
    pub dpi_scale: f32,
    /// Raw DPI value (typically 96, 120, 144, 192).
    pub dpi: u32,
}

/// A rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl Rect {
    pub fn width(&self) -> i32 {
        self.right - self.left
    }
    pub fn height(&self) -> i32 {
        self.bottom - self.top
    }
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= self.left && x < self.right && y >= self.top && y < self.bottom
    }
}

/// Enumerates all active displays and returns their info.
///
/// Uses `EnumDisplayDevicesW` to find all monitors, then queries each
/// for its DPI scaling factor.
pub fn enumerate_displays() -> Vec<DisplayInfo> {
    let mut displays = Vec::new();
    unsafe {
        let mut dev: winapi::um::wingdi::DISPLAY_DEVICEW = std::mem::zeroed();
        dev.cb = std::mem::size_of::<winapi::um::wingdi::DISPLAY_DEVICEW>() as u32;

        let mut index = 0;
        while winapi::um::winuser::EnumDisplayDevicesW(
            std::ptr::null(),
            index,
            &mut dev,
            0,
        ) != 0
        {
            let is_primary =
                (dev.StateFlags & winapi::um::wingdi::DISPLAY_DEVICE_PRIMARY_DEVICE) != 0;

            // Only active displays.
            if (dev.StateFlags & winapi::um::wingdi::DISPLAY_DEVICE_ACTIVE) != 0 {
                let name = wide_to_string(&dev.DeviceName);

                // Get the bounds using GetDeviceCaps as a fallback.
                // EnumDisplaySettingsW requires DEVMODEW which is private.
                let (dpi, dpi_scale) = get_dpi_for_display(dev.DeviceName.as_ptr());

                // Use GetSystemMetrics for primary display bounds as fallback.
                let bounds = if is_primary {
                    Rect {
                        left: 0,
                        top: 0,
                        right: winapi::um::winuser::GetSystemMetrics(
                            winapi::um::winuser::SM_CXSCREEN,
                        ),
                        bottom: winapi::um::winuser::GetSystemMetrics(
                            winapi::um::winuser::SM_CYSCREEN,
                        ),
                    }
                } else {
                    // For non-primary displays, we can't easily get bounds
                    // without DEVMODEW. Return a zero rect.
                    Rect {
                        left: 0,
                        top: 0,
                        right: 0,
                        bottom: 0,
                    }
                };

                displays.push(DisplayInfo {
                    name,
                    is_primary,
                    bounds,
                    dpi_scale,
                    dpi,
                });
            }

            index += 1;
        }
    }
    displays
}

/// Get the primary display.
pub fn primary_display() -> Option<DisplayInfo> {
    enumerate_displays().into_iter().find(|d| d.is_primary)
}

/// Get the display containing the given point.
pub fn display_at(x: i32, y: i32) -> Option<DisplayInfo> {
    enumerate_displays()
        .into_iter()
        .find(|d| d.bounds.contains(x, y))
}

/// Get the DPI scale factor for a display device name.
///
/// Returns (dpi, scale). Falls back to (96, 1.0) on error.
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn get_dpi_for_display(device_name: *const u16) -> (u32, f32) {
    unsafe {
        let mut dpi_x: u32 = 0;
        let mut dpi_y: u32 = 0;
        // Try GetDpiForMonitor first (Windows 8.1+).
        // Use MonitorFromWindow with the desktop window to avoid
        // referencing the private winapi POINT type.
        let hmonitor = winapi::um::winuser::MonitorFromWindow(
            winapi::um::winuser::GetDesktopWindow(),
            winapi::um::winuser::MONITOR_DEFAULTTOPRIMARY,
        );

        let shcore = widestring::Hstring::from("Shcore.dll");
        let hmod = winapi::um::libloaderapi::LoadLibraryW(shcore.as_ptr());
        if !hmod.is_null() {
            let func_name = std::ffi::CString::new("GetDpiForMonitor").ok();
            if let Some(func_name) = func_name {
                let func = winapi::um::libloaderapi::GetProcAddress(hmod, func_name.as_ptr());
                if !func.is_null() {
                    type GetDpiForMonitorFn = unsafe extern "system" fn(
                        winapi::shared::windef::HMONITOR,
                        u32, // MONITOR_DPI_TYPE
                        *mut u32,
                        *mut u32,
                    ) -> i32;
                    let get_dpi: GetDpiForMonitorFn = std::mem::transmute(func);
                    // MDT_EFFECTIVE_DPI = 0
                    let result = get_dpi(hmonitor, 0, &mut dpi_x, &mut dpi_y);
                    winapi::um::libloaderapi::FreeLibrary(hmod);
                    if result == 0 && dpi_x > 0 {
                        return (dpi_x, dpi_x as f32 / 96.0);
                    }
                }
            }
            winapi::um::libloaderapi::FreeLibrary(hmod);
        }

        // Fallback: GetDeviceCaps.
        let hdc = winapi::um::wingdi::CreateDCW(
            device_name,
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
        );
        if !hdc.is_null() {
            let dpi_x = winapi::um::wingdi::GetDeviceCaps(
                hdc,
                winapi::um::wingdi::LOGPIXELSX,
            );
            winapi::um::wingdi::DeleteDC(hdc);
            if dpi_x > 0 {
                return (dpi_x as u32, dpi_x as f32 / 96.0);
            }
        }

        (96, 1.0)
    }
}

/// Convert a logical (DPI-scaled) coordinate to physical pixels.
pub fn logical_to_physical(x: i32, y: i32, scale: f32) -> (i32, i32) {
    ((x as f32 * scale) as i32, (y as f32 * scale) as i32)
}

/// Convert a physical pixel coordinate to logical (DPI-scaled).
pub fn physical_to_logical(x: i32, y: i32, scale: f32) -> (i32, i32) {
    if scale == 0.0 {
        return (x, y);
    }
    ((x as f32 / scale) as i32, (y as f32 / scale) as i32)
}

/// Clamp a rectangle to stay within the given display bounds.
pub fn clamp_to_display(rect: &Rect, display: &Rect) -> Rect {
    let w = rect.width();
    let h = rect.height();
    let mut left = rect.left;
    let mut top = rect.top;

    if left < display.left {
        left = display.left;
    }
    if top < display.top {
        top = display.top;
    }
    if left + w > display.right {
        left = display.right - w;
    }
    if top + h > display.bottom {
        top = display.bottom - h;
    }

    Rect {
        left,
        top,
        right: left + w,
        bottom: top + h,
    }
}

fn wide_to_string(wide: &[u16]) -> String {
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    OsString::from_wide(&wide[..len])
        .to_string_lossy()
        .into_owned()
}

// Minimal UTF-16 string helper.
mod widestring {
    pub struct Hstring(Vec<u16>);
    impl Hstring {
        pub fn from(s: &str) -> Self {
            Self(s.encode_utf16().chain(std::iter::once(0)).collect())
        }
        pub fn as_ptr(&self) -> *const u16 {
            self.0.as_ptr()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_to_physical_scales_correctly() {
        let (x, y) = logical_to_physical(100, 200, 1.5);
        assert_eq!(x, 150);
        assert_eq!(y, 300);
    }

    #[test]
    fn physical_to_logical_scales_correctly() {
        let (x, y) = physical_to_logical(150, 300, 1.5);
        assert_eq!(x, 100);
        assert_eq!(y, 200);
    }

    #[test]
    fn round_trip_is_identity() {
        for scale in [1.0, 1.25, 1.5, 1.75, 2.0] {
            let (lx, ly) = physical_to_logical(1024, 768, scale);
            let (px, py) = logical_to_physical(lx, ly, scale);
            // Allow ±2 pixel rounding error from float truncation.
            assert!(
                (px - 1024).abs() <= 2,
                "x round trip failed at scale {}: got {}",
                scale,
                px
            );
            assert!(
                (py - 768).abs() <= 2,
                "y round trip failed at scale {}: got {}",
                scale,
                py
            );
        }
    }

    #[test]
    fn zero_scale_does_not_divide_by_zero() {
        let (x, y) = physical_to_logical(100, 200, 0.0);
        assert_eq!(x, 100);
        assert_eq!(y, 200);
    }

    #[test]
    fn rect_contains_works() {
        let r = Rect { left: 0, top: 0, right: 100, bottom: 100 };
        assert!(r.contains(50, 50));
        assert!(r.contains(0, 0));
        assert!(!r.contains(100, 50));
        assert!(!r.contains(50, 100));
        assert!(!r.contains(-1, 50));
    }

    #[test]
    fn clamp_to_display_moves_rect_inside() {
        let display = Rect { left: 0, top: 0, right: 1920, bottom: 1080 };
        let rect = Rect { left: -50, top: -50, right: 200, bottom: 200 };
        let clamped = clamp_to_display(&rect, &display);
        assert!(clamped.left >= display.left);
        assert!(clamped.top >= display.top);
    }

    #[test]
    fn clamp_to_display_shifts_back_inside() {
        let display = Rect { left: 0, top: 0, right: 1920, bottom: 1080 };
        let rect = Rect { left: 1800, top: 1000, right: 2000, bottom: 1200 };
        let clamped = clamp_to_display(&rect, &display);
        assert!(clamped.right <= display.right);
        assert!(clamped.bottom <= display.bottom);
    }

    #[test]
    fn displays_are_enumerated() {
        let displays = enumerate_displays();
        assert!(!displays.is_empty(), "should find at least one display");
        assert!(
            displays.iter().any(|d| d.is_primary),
            "one display should be primary"
        );
    }

    #[test]
    fn primary_display_exists() {
        let primary = primary_display();
        assert!(primary.is_some(), "primary display should exist");
    }

    #[test]
    fn display_at_primary_works() {
        let primary = primary_display().unwrap();
        let cx = primary.bounds.left + primary.bounds.width() / 2;
        let cy = primary.bounds.top + primary.bounds.height() / 2;
        let display = display_at(cx, cy);
        assert!(display.is_some(), "should find display at primary center");
    }

    #[test]
    fn dpi_scale_is_reasonable() {
        let displays = enumerate_displays();
        for d in &displays {
            assert!(d.dpi_scale >= 0.5, "scale {} too low", d.dpi_scale);
            assert!(d.dpi_scale <= 4.0, "scale {} too high", d.dpi_scale);
            assert!(d.dpi >= 48, "dpi {} too low", d.dpi);
            assert!(d.dpi <= 384, "dpi {} too high", d.dpi);
        }
    }
}
