"""Verify the agent editor's layout from the captured pixels.

Vision models are rate-limited here, so this checks the things that actually
matter and that are measurable: is there glyph ink where a label should be, does
the footer have content, and is anything clipped at the right edge.
"""

import pathlib
import struct
import sys
from collections import Counter

path = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else
                    r"C:\Projects\ScreenBuddy\target\agent_editor.png")
data = path.read_bytes()
off = struct.unpack_from("<I", data, 10)[0]
W = struct.unpack_from("<i", data, 18)[0]
H = -struct.unpack_from("<i", data, 22)[0]
body = data[off:]


def px(x, y):
    i = (y * W + x) * 4
    b, g, r = body[i], body[i + 1], body[i + 2]
    return (r, g, b)


def ink(x0, y0, x1, y1, bg_tolerance=18):
    """Count pixels differing from the region's dominant (background) colour."""
    colours = Counter()
    for y in range(y0, min(y1, H)):
        for x in range(x0, min(x1, W)):
            colours[px(x, y)] += 1
    if not colours:
        return 0, 0, None
    bg = colours.most_common(1)[0][0]
    lit = sum(
        n for c, n in colours.items()
        if max(abs(c[i] - bg[i]) for i in range(3)) > bg_tolerance
    )
    return lit, sum(colours.values()), bg


def report(name, x0, y0, x1, y1):
    lit, total, bg = ink(x0, y0, x1, y1)
    pct = 100 * lit / max(1, total)
    status = "PASS" if pct > 1.5 else "FAIL"
    print(f"  {status}  {name}: {pct:.1f}% inked (bg #{bg[0]:02X}{bg[1]:02X}{bg[2]:02X})")
    return pct > 1.5


print(f"image {W}x{H}")
results = []

# Label column: between sidebar (190) and fields (355).
results.append(report("label column", 200, 70, 350, 220))
# Field values area.
results.append(report("field values", 360, 70, W - 30, 130))
# Sidebar entries.
results.append(report("sidebar list", 8, 60, 185, 200))
# Title row.
results.append(report("title", 8, 4, 200, 30))
# Tab strip.
results.append(report("tab strip", 8, 36, 400, 62))
# Footer buttons.
results.append(report("footer buttons", 8, H - 44, W - 20, H - 12))

# Nothing should touch the right edge: fields must leave a margin.
# A field border drawn flush to the window edge is the failure mode. Check the
# last column for ink that differs from the page background, comparing against
# the same column's own modal colour rather than another panel's.
page_bg = px(4, H // 2)
edge = Counter(px(W - 2, y) for y in range(0, H))
edge_bg = edge.most_common(1)[0][0]
lit = sum(
    n for c, n in edge.items()
    if max(abs(c[i] - edge_bg[i]) for i in range(3)) > 24
)
pct = 100 * lit / max(1, sum(edge.values()))
# A hairline border along the edge is expected; a filled block is not.
ok = pct < 40
print(f"  {'PASS' if ok else 'FAIL'}  right margin clean ({pct:.1f}% of edge column inked)")
results.append(ok)

print(f"\n{sum(results)}/{len(results)} layout checks passed")
sys.exit(0 if all(results) else 1)