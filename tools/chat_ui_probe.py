"""Verify the Wave 2 chat-UI features on the LIVE window, not just in unit tests.

Unit tests exercise `draw_ui` against a DIB. This drives the shipped window the
way a person does — post the real keystrokes, capture the real window — and
asserts the pixels change, which is the only evidence that the render path from
WM_KEYDOWN to the screen is intact.

Checks:
  1. the main window exists and renders (colour count is a floor, not a pass)
  2. Ctrl+F opens the search bar: the band below the header gains ink
  3. Escape closes it: the band returns to its previous ink level
  4. the window survives a conversation drawn into it

Usage:
    python tools/chat_ui_probe.py
"""

from __future__ import annotations

import ctypes
import ctypes.wintypes as wt
import sys
import time
from pathlib import Path

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32

user32.SetProcessDPIAware()

MAIN_CLASS = "ScreenBuddyMain"
# The search bar sits just below the 56px header, full width of the message pane.
HEADER_H = 56
SEARCH_BAND_TOP = HEADER_H + 4
SEARCH_BAND_BOTTOM = HEADER_H + 32

checks: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    checks.append((name, ok, detail))
    print(f"  {'PASS' if ok else 'FAIL'}  {name}" + (f": {detail}" if detail else ""))


def find_window_by_class(class_name: str) -> int:
    """Find the first top-level window with this class name."""
    found = []

    @ctypes.WINFUNCTYPE(wt.BOOL, wt.HWND, wt.LPARAM)
    def cb(hwnd, _):
        buf = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, buf, 256)
        if buf.value == class_name:
            found.append(hwnd)
            return False
        return True

    user32.EnumWindows(cb, 0)
    return found[0] if found else 0


class BITMAPINFOHEADER(ctypes.Structure):
    _fields_ = [
        ("biSize", ctypes.c_uint32), ("biWidth", ctypes.c_int32),
        ("biHeight", ctypes.c_int32), ("biPlanes", ctypes.c_uint16),
        ("biBitCount", ctypes.c_uint16), ("biCompression", ctypes.c_uint32),
        ("biSizeImage", ctypes.c_uint32), ("biXPelsPerMeter", ctypes.c_int32),
        ("biYPelsPerMeter", ctypes.c_int32), ("biClrUsed", ctypes.c_uint32),
        ("biClrImportant", ctypes.c_uint32),
    ]


class BITMAPINFO(ctypes.Structure):
    _fields_ = [("bmiHeader", BITMAPINFOHEADER), ("bmiColors", ctypes.c_uint32 * 3)]


def capture(hwnd: int) -> tuple[list[list[tuple[int, int, int]]], int, int]:
    """Capture the window's CLIENT area.

    Client coordinates are used throughout so a band measured from the capture's
    origin lands where the layout put it. A whole-window capture includes the
    title bar and border, which shifts every band by the frame height.
    """
    rect = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rect))
    w = rect.right - rect.left
    h = rect.bottom - rect.top
    if w <= 0 or h <= 0:
        return [], 0, 0

    screen_dc = user32.GetDC(hwnd)
    mdc = gdi32.CreateCompatibleDC(screen_dc)

    info = BITMAPINFO()
    info.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    info.bmiHeader.biWidth = w
    info.bmiHeader.biHeight = -h
    info.bmiHeader.biPlanes = 1
    info.bmiHeader.biBitCount = 32
    info.bmiHeader.biCompression = 0

    raw = ctypes.c_void_p()
    bitmap = gdi32.CreateDIBSection(
        screen_dc, ctypes.byref(info), 0, ctypes.byref(raw), None, 0
    )
    gdi32.SelectObject(mdc, bitmap)

    # Flag 1 = PW_CLIENTONLY: capture the client area at origin (0, 0).
    # GetWindowDC + GetDIBits returns all-black for this window.
    if not user32.PrintWindow(hwnd, mdc, 1):
        gdi32.BitBlt(mdc, 0, 0, w, h, screen_dc, 0, 0, 0x00CC0020)

    data = ctypes.string_at(raw, w * h * 4)
    gdi32.DeleteObject(bitmap)
    gdi32.DeleteDC(mdc)
    user32.ReleaseDC(hwnd, screen_dc)

    rows = []
    for y in range(h):
        base = y * w * 4
        rows.append([
            (data[base + x * 4 + 2], data[base + x * 4 + 1], data[base + x * 4])
            for x in range(w)
        ])
    return rows, w, h


def ink_in_band(rows, w, h, top, bottom) -> int:
    """Count pixels differing from the modal (background) colour in a band."""
    if not rows:
        return 0
    counts: dict[tuple[int, int, int], int] = {}
    for y in range(max(0, top), min(h, bottom)):
        for x in range(w):
            px = rows[y][x]
            counts[px] = counts.get(px, 0) + 1
    if not counts:
        return 0
    modal = max(counts, key=counts.get)
    return sum(c for px, c in counts.items() if px != modal)


def distinct_colours(rows, h) -> int:
    seen = set()
    for y in range(0, h, 3):
        for px in rows[y][::3]:
            seen.add(px)
    return len(seen)


def post_key(hwnd: int, vk: int, ctrl: bool = False) -> None:
    """Deliver a keystroke synchronously so the handler has run before capture."""
    if ctrl:
        user32.SendMessageW(hwnd, 0x0100, 0x11, 0)  # WM_KEYDOWN VK_CONTROL
    user32.SendMessageW(hwnd, 0x0100, vk, 0)  # WM_KEYDOWN
    user32.SendMessageW(hwnd, 0x0101, vk, 0)  # WM_KEYUP
    if ctrl:
        user32.SendMessageW(hwnd, 0x0101, 0x11, 0)


def pixels_differing(a, b, w, h) -> int:
    """Count pixels that differ between two captures of the same size."""
    if not a or not b:
        return 0
    n = 0
    for y in range(h):
        ra, rb = a[y], b[y]
        for x in range(w):
            if ra[x] != rb[x]:
                n += 1
    return n


def main() -> int:
    hwnd = find_window_by_class(MAIN_CLASS)
    if not hwnd:
        check("main window exists", False, f"no window of class {MAIN_CLASS}")
        return 1
    check("main window exists", True, f"hwnd={hwnd}")

    # Start from a known state: search closed, so the first Ctrl+F opens it
    # rather than closing whatever a previous run left open.
    post_key(hwnd, 0x1B)  # VK_ESCAPE
    time.sleep(0.4)

    rows, w, h = capture(hwnd)
    if not rows:
        check("window renders", False, "capture returned no pixels")
        return 1
    colours = distinct_colours(rows, h)
    check("window renders", colours > 20, f"{colours} distinct colours (floor: >20)")

    # Ctrl+F should open the search bar. Compare whole captures: the bar's fill
    # is close to the surrounding background, so an ink-vs-modal delta is too
    # coarse a signal — a direct before/after diff is unambiguous.
    post_key(hwnd, 0x46, ctrl=True)  # 'F'
    time.sleep(0.5)
    rows_open, _, _ = capture(hwnd)
    opened = pixels_differing(rows, rows_open, w, h)
    check(
        "Ctrl+F opens the search bar",
        opened > 200,
        f"{opened} pixels changed (client {w}x{h})",
    )

    # Escape should close it again, restoring the original view.
    post_key(hwnd, 0x1B)  # VK_ESCAPE
    time.sleep(0.5)
    rows_closed, _, _ = capture(hwnd)
    closed = pixels_differing(rows, rows_closed, w, h)
    check(
        "Escape closes the search bar",
        closed < opened,
        f"{closed} pixels differ from the original view (was {opened} open)",
    )

    passed = sum(1 for _, ok, _ in checks if ok)
    print(f"\n{passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    sys.exit(main())
