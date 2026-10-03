"""Screenshot the agent editor to verify it actually renders.

The editor is the primary way a user configures an agent, so "it compiles" is not
evidence it is usable. This launches the real binary, finds the editor window,
and captures it with PrintWindow (which works for windows that are not
composited, unlike GetWindowDC).
"""

import ctypes
import ctypes.wintypes as wt
import pathlib
import struct
import subprocess
import sys
import time

ROOT = pathlib.Path(__file__).resolve().parent.parent
OUT = ROOT / "target" / "agent_editor.png"

user32 = ctypes.windll.user32
gdi32 = ctypes.windll.gdi32
user32.SetProcessDPIAware()


def find_window(class_name, timeout=30):
    found = []

    CALLBACK = ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)

    def cb(hwnd, _l):
        buf = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(hwnd, buf, 256)
        if buf.value == class_name:
            found.append(hwnd)
        return True

    deadline = time.time() + timeout
    while time.time() < deadline and not found:
        user32.EnumWindows(CALLBACK(cb), 0)
        if found:
            break
        time.sleep(0.5)

    if not found:
        return None

    hwnd = found[0]
    # A window can be enumerated before it is ready to paint; wait for a real
    # client area, otherwise the capture comes back blank.
    for _ in range(40):
        rect = wt.RECT()
        user32.GetClientRect(hwnd, ctypes.byref(rect))
        if (rect.right - rect.left) > 0 and (rect.bottom - rect.top) > 0:
            if user32.IsWindowVisible(hwnd):
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
    info.bmiHeader.biHeight = -height  # top-down
    info.bmiHeader.biPlanes = 1
    info.bmiHeader.biBitCount = 32
    info.bmiHeader.biCompression = 0  # BI_RGB

    raw = ctypes.c_void_p()
    bitmap = gdi32.CreateDIBSection(
        screen_dc, ctypes.byref(info), 0, ctypes.byref(raw), None, 0
    )
    gdi32.SelectObject(mem_dc, bitmap)

    # PW_CLIENTONLY: the window paints its own client area, and PrintWindow is
    # what works where GetWindowDC returns black for non-composited windows.
    ok = user32.PrintWindow(hwnd, mem_dc, 1)
    if not ok:
        gdi32.BitBlt(
            mem_dc, 0, 0, width, height, screen_dc, 0, 0, 0x00CC0020
        )

    # Write a BMP by hand. Packing a ctypes struct for BITMAPFILEHEADER pads it
    # to 8 bytes, which produced a malformed file whose offsets did not line up
    # -- so the "capture" was unreadable and proved nothing.
    stride = width * 4
    image_size = stride * height
    file_header_size = 14
    info_header_size = 40
    pixel_offset = file_header_size + info_header_size

    file_header = struct.pack(
        "<2sIHHI",
        b"BM",
        pixel_offset + image_size,
        0,
        0,
        pixel_offset,
    )
    # biHeight is already negative (top-down); store it as-is.
    info_header = struct.pack(
        "<IiiHHIIiiII",
        info_header_size,
        width,
        -height,
        1,
        32,
        0,  # BI_RGB
        image_size,
        0,
        0,
        0,
        0,
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


def count_distinct_colours(path):
    """A blank or single-colour capture means the window did not paint."""
    data = pathlib.Path(path).read_bytes()
    offset = 54  # BMP header
    body = data[offset:]
    seen = set()
    for i in range(0, len(body) - 3, 4 * 37):  # sample every 37th pixel
        seen.add(body[i : i + 3])
        if len(seen) > 40:
            break
    return len(seen)


def main():
    # Use whichever build is newest: a stale binary makes this report a missing
    # window when the code is actually present in a newer build.
    candidates = [
        ROOT / "target" / "release" / "screenbuddy.exe",
        ROOT / "target" / "debug" / "screenbuddy.exe",
    ]
    existing = [p for p in candidates if p.exists()]
    if not existing:
        print("FAIL: no built binary")
        return 1
    exe = max(existing, key=lambda p: p.stat().st_mtime)
    print(f"using {exe} ({exe.stat().st_mtime})")

    app = subprocess.Popen([str(exe)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    failures = 0
    try:
        hwnd = find_window("ScreenBuddyAgentEditor")
        if not hwnd:
            print("FAIL: editor window never appeared")
            return 1
        print(f"editor window: {hwnd}")

        time.sleep(2.0)
        user32.SetForegroundWindow(hwnd)
        time.sleep(0.5)

        width, height = capture(hwnd, OUT)
        print(f"captured {width}x{height} -> {OUT}")

        size = OUT.stat().st_size
        print(f"  {'PASS' if size > 5000 else 'FAIL'}  file size {size} bytes")
        if size <= 5000:
            failures += 1

        colours = count_distinct_colours(OUT)
        print(f"  {'PASS' if colours > 8 else 'FAIL'}  distinct colours {colours}")
        if colours <= 8:
            failures += 1

        # The window must be big enough to lay the editor out in.
        ok = width >= 700 and height >= 500
        print(f"  {'PASS' if ok else 'FAIL'}  size {width}x{height} suits the layout")
        if not ok:
            failures += 1
    finally:
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()

    print(f"\n{4 - failures}/4 checks passed")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())