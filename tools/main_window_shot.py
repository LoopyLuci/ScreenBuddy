"""Capture the live ScreenBuddy main window (client area) to a PNG.

Used to inspect what the window actually shows, since a colour count only proves
*something* drew.

Usage:
    python tools/main_window_shot.py [output.png]
"""

from __future__ import annotations

import ctypes
import ctypes.wintypes as wt
import struct
import sys
import time
from pathlib import Path

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
user32.SetProcessDPIAware()

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = ROOT / "target" / "main_window.png"


def find_window(class_name: str, timeout: float = 10.0) -> int:
    found = []

    @ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)
    def cb(hwnd, _):
        buf = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, buf, 256)
        if buf.value == class_name and user32.IsWindowVisible(hwnd):
            found.append(hwnd)
            return False
        return True

    deadline = time.time() + timeout
    while time.time() < deadline and not found:
        user32.EnumWindows(cb, 0)
        if not found:
            time.sleep(0.3)
    return found[0] if found else 0


class BIH(ctypes.Structure):
    _fields_ = [
        ("biSize", ctypes.c_uint32), ("biWidth", ctypes.c_int32),
        ("biHeight", ctypes.c_int32), ("biPlanes", ctypes.c_uint16),
        ("biBitCount", ctypes.c_uint16), ("biCompression", ctypes.c_uint32),
        ("biSizeImage", ctypes.c_uint32), ("biXPelsPerMeter", ctypes.c_int32),
        ("biYPelsPerMeter", ctypes.c_int32), ("biClrUsed", ctypes.c_uint32),
        ("biClrImportant", ctypes.c_uint32),
    ]


class BI(ctypes.Structure):
    _fields_ = [("bmiHeader", BIH), ("bmiColors", ctypes.c_uint32 * 3)]


def capture_to_png(hwnd: int, path: Path) -> tuple[int, int]:
    r = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(r))
    w, h = r.right, r.bottom
    if w <= 0 or h <= 0:
        raise RuntimeError("window has no client area")

    sdc = user32.GetDC(hwnd)
    mdc = gdi32.CreateCompatibleDC(sdc)
    info = BI()
    info.bmiHeader.biSize = ctypes.sizeof(BIH)
    info.bmiHeader.biWidth = w
    info.bmiHeader.biHeight = -h
    info.bmiHeader.biPlanes = 1
    info.bmiHeader.biBitCount = 32
    info.bmiHeader.biCompression = 0
    raw = ctypes.c_void_p()
    bmp = gdi32.CreateDIBSection(sdc, ctypes.byref(info), 0, ctypes.byref(raw), None, 0)
    gdi32.SelectObject(mdc, bmp)
    if not user32.PrintWindow(hwnd, mdc, 1):
        gdi32.BitBlt(mdc, 0, 0, w, h, sdc, 0, 0, 0x00CC0020)
    data = ctypes.string_at(raw, w * h * 4)
    gdi32.DeleteObject(bmp)
    gdi32.DeleteDC(mdc)
    user32.ReleaseDC(hwnd, sdc)

    stride = w * 4
    image_size = stride * h
    header = struct.pack("<2sIHHI", b"BM", 14 + 40 + image_size, 0, 0, 14 + 40)
    info_header = struct.pack("<IiiHHIIiiII", 40, w, -h, 1, 32, 0, image_size, 0, 0, 0, 0)
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as f:
        f.write(header)
        f.write(info_header)
        f.write(data)
    return w, h


def main() -> int:
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else DEFAULT_OUT
    hwnd = find_window("ScreenBuddyMain")
    if not hwnd:
        print("no visible ScreenBuddyMain window")
        return 1
    w, h = capture_to_png(hwnd, out)
    print(f"captured {w}x{h} -> {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
