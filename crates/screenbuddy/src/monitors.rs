//! Monitor enumeration and window placement.
//!
//! The main window was created with `CW_USEDEFAULT`, which puts it on the
//! primary monitor whatever the user is actually looking at. On a multi-monitor
//! desk that means the window opens on a screen the user is not using.
//!
//! `EnumDisplayMonitors` is the API that reports the real layout, including
//! negative coordinates for monitors placed left of or above the primary one.

use winapi::shared::minwindef::{BOOL, LPARAM};
use winapi::shared::windef::{HDC, HMONITOR, LPRECT, RECT};
use winapi::um::winuser::{EnumDisplayMonitors, GetMonitorInfoW, MONITORINFO};

/// One monitor's bounds in virtual-screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorInfo {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
    pub is_primary: bool,
}

impl MonitorInfo {
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

/// Every monitor currently attached, primary first.
///
/// Returns an empty vector if the OS reports none, which callers must treat as
/// "no opinion" rather than an error.
pub fn enumerate() -> Vec<MonitorInfo> {
    let mut out: Vec<MonitorInfo> = Vec::new();
    let ptr = &mut out as *mut Vec<MonitorInfo> as LPARAM;

    unsafe extern "system" fn cb(
        _monitor: HMONITOR,
        _dc: HDC,
        _rect: LPRECT,
        data: LPARAM,
    ) -> BOOL {
        let out = &mut *(data as *mut Vec<MonitorInfo>);
        let hmon = _monitor;
        let mut info: MONITORINFO = std::mem::zeroed();
        info.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(hmon, &mut info) != 0 {
            out.push(MonitorInfo {
                left: info.rcMonitor.left,
                top: info.rcMonitor.top,
                right: info.rcMonitor.right,
                bottom: info.rcMonitor.bottom,
                is_primary: info.dwFlags & 1 != 0, // MONITORINFOF_PRIMARY
            });
        }
        1
    }

    unsafe {
        EnumDisplayMonitors(std::ptr::null_mut(), std::ptr::null(), Some(cb), ptr);
    }
    // Primary first so callers can use `.first()` for a sensible default.
    out.sort_by_key(|m| !m.is_primary);
    out
}

/// The monitor containing a point, falling back to the primary, then to the
/// first reported monitor. `None` only when no monitor was reported at all.
pub fn monitor_at(x: i32, y: i32) -> Option<MonitorInfo> {
    let monitors = enumerate();
    pick_monitor(&monitors, x, y)
}

/// Pure selection over a known monitor list, so the placement rule is testable
/// without a display.
pub fn pick_monitor(monitors: &[MonitorInfo], x: i32, y: i32) -> Option<MonitorInfo> {
    if monitors.is_empty() {
        return None;
    }
    if let Some(m) = monitors.iter().find(|m| m.contains(x, y)) {
        return Some(*m);
    }
    monitors
        .iter()
        .find(|m| m.is_primary)
        .copied()
        .or_else(|| monitors.first().copied())
}

/// Top-left corner that centres a `w` x `h` window on `monitor`, kept on screen.
pub fn centre_on(monitor: &MonitorInfo, w: i32, h: i32) -> (i32, i32) {
    let x = monitor.left + (monitor.width() - w) / 2;
    let y = monitor.top + (monitor.height() - h) / 2;
    // Never place the window above or left of the monitor's own origin.
    (x.max(monitor.left), y.max(monitor.top))
}

/// Whether a window rectangle is fully inside any of `monitors`.
///
/// Used to decide whether a saved position is still valid after a monitor is
/// unplugged: a window restored onto a display that no longer exists is
/// invisible and looks like a crash.
#[allow(dead_code)]
pub fn is_on_some_monitor(monitors: &[MonitorInfo], rect: &RECT) -> bool {
    monitors.iter().any(|m| {
        rect.left >= m.left && rect.top >= m.top && rect.right <= m.right && rect.bottom <= m.bottom
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mon(left: i32, top: i32, right: i32, bottom: i32, primary: bool) -> MonitorInfo {
        MonitorInfo {
            left,
            top,
            right,
            bottom,
            is_primary: primary,
        }
    }

    #[test]
    fn a_point_picks_the_monitor_that_contains_it() {
        let monitors = vec![mon(0, 0, 1920, 1080, true), mon(1920, 0, 3840, 1080, false)];
        assert_eq!(pick_monitor(&monitors, 100, 100).unwrap().left, 0);
        assert_eq!(pick_monitor(&monitors, 2500, 500).unwrap().left, 1920);
    }

    #[test]
    fn a_point_outside_every_monitor_falls_back_to_the_primary() {
        let monitors = vec![mon(0, 0, 1920, 1080, true), mon(1920, 0, 3840, 1080, false)];
        // Far off any display, e.g. a cursor left on a monitor just unplugged.
        assert!(pick_monitor(&monitors, -5000, -5000).unwrap().is_primary);
    }

    #[test]
    fn monitors_to_the_left_of_primary_use_negative_coordinates() {
        // A monitor placed left of the primary has a negative left edge; a
        // contains() that assumed non-negative bounds would misplace the window.
        let monitors = vec![mon(-1920, 0, 0, 1080, false), mon(0, 0, 1920, 1080, true)];
        let picked = pick_monitor(&monitors, -1000, 500).unwrap();
        assert_eq!(picked.left, -1920);
        assert!(!picked.is_primary);
    }

    #[test]
    fn no_monitors_is_none_rather_than_a_panic() {
        assert!(pick_monitor(&[], 0, 0).is_none());
    }

    #[test]
    fn centring_keeps_the_window_inside_the_monitor() {
        let m = mon(1920, 0, 3840, 1080, false);
        let (x, y) = centre_on(&m, 900, 640);
        assert!(x >= m.left && x + 900 <= m.right);
        assert!(y >= m.top && y + 640 <= m.bottom);
        // Centred, not pinned to a corner.
        assert!(x > m.left);
    }

    #[test]
    fn centring_a_window_larger_than_the_monitor_pins_to_the_origin() {
        let m = mon(100, 50, 400, 300, true);
        let (x, y) = centre_on(&m, 1920, 1080);
        assert_eq!((x, y), (100, 50), "cannot centre; must not go off-screen");
    }

    #[test]
    fn a_window_inside_a_monitor_is_recognised() {
        let monitors = vec![mon(0, 0, 1920, 1080, true)];
        let inside = RECT {
            left: 100,
            top: 100,
            right: 1000,
            bottom: 700,
        };
        assert!(is_on_some_monitor(&monitors, &inside));
    }

    #[test]
    fn a_window_off_every_monitor_is_recognised_as_orphaned() {
        let monitors = vec![mon(0, 0, 1920, 1080, true)];
        let orphan = RECT {
            left: 3000,
            top: 100,
            right: 3900,
            bottom: 700,
        };
        assert!(!is_on_some_monitor(&monitors, &orphan));
    }

    #[test]
    fn enumeration_does_not_panic_and_is_primary_first() {
        // On a headless CI box this can be empty; the contract is no panic, and
        // primary first when anything is reported.
        let monitors = enumerate();
        if monitors.len() > 1 {
            assert!(monitors[0].is_primary || !monitors.iter().any(|m| m.is_primary));
        }
    }
}
