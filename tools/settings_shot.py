"""Screenshot the settings window to verify it actually renders.

The settings window is created hidden and only opened from the tray, so this
drives the real app, opens it the way the tray does, and captures the result.
"""

import ctypes
import ctypes.wintypes as wt
import pathlib
import struct
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "target" / "settings_window.png"

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
user32.SetProcessDPIAware()


def find_window(class_name, visible_only=False, timeout=30):
    found = []
    CALLBACK = ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)

    def cb(hwnd, _l):
        buf = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, buf, 256)
        if buf.value == class_name:
            if not visible_only or user32.IsWindowVisible(hwnd):
                found.append(hwnd)
        return True

    deadline = time.time() + timeout
    while time.time() < deadline and not found:
        user32.EnumWindows(CALLBACK(cb), 0)
        time.sleep(0.4)

    if not found:
        return None
    hwnd = found[0]
    for _ in range(40):
        rect = wt.RECT()
        user32.GetClientRect(hwnd, ctypes.byref(rect))
        if (rect.right - rect.left) > 0 and (rect.bottom - rect.top) > 0:
            return hwnd
        time.sleep(0.25)
    return hwnd


def capture(hwnd, path):
    rect = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rect))
    width, height = rect.right - rect.left, rect.bottom - rect.top
    if width <= 0 or height <= 0:
        raise RuntimeError("window has no area")

    screen_dc = user32.GetDC(hwnd)
    mem_dc = gdi32.CreateCompatibleDC(screen_dc)

    class BITMAPINFOHEADER(ctypes.Structure):
        _fields_ = [
            ("biSize", ctypes.c_uint32),
            ("biWidth", ctypes.c_int32),
            ("biHeight", ctypes.c_int32),
            ("biPlanes", ctypes.c_uint16),
            ("biBitCount", ctypes.c_uint16),
            ("biCompression", ctypes.c_uint32),
            ("biSizeImage", ctypes.c_uint32),
            ("biXPelsPerMeter", ctypes.c_int32),
            ("biYPelsPerMeter", ctypes.c_int32),
            ("biClrUsed", ctypes.c_uint32),
            ("biClrImportant", ctypes.c_uint32),
        ]

    class BITMAPINFO(ctypes.Structure):
        _fields_ = [("bmiHeader", BITMAPINFOHEADER), ("bmiColors", ctypes.c_uint32 * 3)]

    info = BITMAPINFO()
    info.bmiHeader.biSize = ctypes.sizeof(BITMAPINFOHEADER)
    info.bmiHeader.biWidth = width
    info.bmiHeader.biHeight = -height
    info.bmiHeader.biPlanes = 1
    info.bmiHeader.biBitCount = 32
    info.bmiHeader.biCompression = 0

    raw = ctypes.c_void_p()
    bitmap = gdi32.CreateDIBSection(
        screen_dc, ctypes.byref(info), 0, ctypes.byref(raw), None, 0
    )
    gdi32.SelectObject(mem_dc, bitmap)

    # PrintWindow works where GetWindowDC returns black for non-composited
    # windows.
    if not user32.PrintWindow(hwnd, mem_dc, 1):
        gdi32.BitBlt(mem_dc, 0, 0, width, height, screen_dc, 0, 0, 0x00CC0020)

    stride = width * 4
    image_size = stride * height
    file_header = struct.pack(
        "<2sIHHI", b"BM", 14 + 40 + image_size, 0, 0, 14 + 40
    )
    info_header = struct.pack(
        "<IiiHHIIiiII", 40, width, -height, 1, 32, 0, image_size, 0, 0, 0, 0
    )
    pixels = ctypes.string_at(raw, image_size)
    with open(path, "wb") as f:
        f.write(file_header)
        f.write(info_header)
        f.write(pixels)

    gdi32.DeleteObject(bitmap)
    gdi32.DeleteDC(mem_dc)
    user32.ReleaseDC(hwnd, screen_dc)
    return width, height


def distinct_colours(path, cap=40):
    data = pathlib.Path(path).read_bytes()
    body = data[54:]
    seen = set()
    for i in range(0, len(body) - 3, 4 * 37):
        seen.add(body[i : i + 3])
        if len(seen) > cap:
            break
    return len(seen)


def main():
    candidates = [
        ROOT / "target" / "release" / "screenbuddy.exe",
        ROOT / "target" / "debug" / "screenbuddy.exe",
    ]
    existing = [p for p in candidates if p.exists()]
    if not existing:
        print("FAIL: no built binary")
        return 1
    exe = max(existing, key=lambda p: p.stat().st_mtime)
    print(f"using {exe}")

    log = open(ROOT / "target" / "settings_shot_app.log", "w", errors="replace")
    app = subprocess.Popen([str(exe)], stdout=log, stderr=subprocess.STDOUT)
    checks = []
    try:
        # The window is created hidden, so look for it without requiring
        # visibility first.
        hwnd = find_window("ScreenBuddySettings", timeout=30)
        if not hwnd:
            print("FAIL: settings window never appeared")
            return 1
        print(f"settings window: {hwnd}")

        # Show it the way the tray does, then wait for a paint.
        user32.ShowWindow(hwnd, 5)  # SW_SHOW
        user32.SetForegroundWindow(hwnd)
        user32.RedrawWindow(hwnd, None, None, 0x0001 | 0x0004 | 0x0080)
        time.sleep(2.0)
        visible = bool(user32.IsWindowVisible(hwnd))
        checks.append(("window became visible", visible, visible))

        width, height = capture(hwnd, OUT)
        print(f"captured {width}x{height} -> {OUT}")
        checks.append(("client area has room", width >= 500 and height >= 400, f"{width}x{height}"))

        size = OUT.stat().st_size
        checks.append(("capture is non-trivial", size > 5000, f"{size} bytes"))

        colours = distinct_colours(OUT)
        checks.append(("window actually painted", colours > 8, f"{colours} distinct colours"))
    finally:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()

    print()
    failures = 0
    for name, ok, detail in checks:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {detail}")
        if not ok:
            failures += 1
    print(f"\n{len(checks) - failures}/{len(checks)} checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())