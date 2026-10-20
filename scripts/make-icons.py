#!/usr/bin/env python3
"""Draw the PUMZI tray and app icons with a tiny SDF rasterizer."""

import math
import os
import struct
import subprocess
import tempfile
import zlib

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
ASSETS = os.path.join(ROOT, "assets")
BUILD = os.path.join(ROOT, "build")
TAURI_ICONS = os.path.join(ROOT, "src-tauri", "icons")


def clamp(value, low=0.0, high=1.0):
    return max(low, min(high, value))


def sd_segment(px, py, ax, ay, bx, by):
    dx, dy = bx - ax, by - ay
    length = dx * dx + dy * dy
    t = 0.0 if length == 0 else clamp(((px - ax) * dx + (py - ay) * dy) / length)
    qx, qy = ax + t * dx, ay + t * dy
    return math.hypot(px - qx, py - qy)


def sd_circle(px, py, cx, cy, r):
    return math.hypot(px - cx, py - cy) - r


def sd_round_rect(px, py, cx, cy, hw, hh, r):
    qx = abs(px - cx) - (hw - r)
    qy = abs(py - cy) - (hh - r)
    return math.hypot(max(qx, 0), max(qy, 0)) + min(max(qx, qy), 0) - r


def coverage(sd):
    return clamp(0.5 - sd)


def over(dst, src, alpha):
    dr, dg, db, da = dst
    sr, sg, sb = src
    a = alpha + da * (1 - alpha)
    if a <= 0:
        return (0, 0, 0, 0)
    r = (sr * alpha + dr * da * (1 - alpha)) / a
    g = (sg * alpha + dg * da * (1 - alpha)) / a
    b = (sb * alpha + db * da * (1 - alpha)) / a
    return (r, g, b, a)


# Geometry in a 128 x 128 design space.
BOWL_CX, BOWL_CY, BOWL_R = 64, 82, 40
RIM = (64, 81, 44, 5, 5)  # cx, cy, half-width, half-height, radius
W = 8.0
NOODLES = [
    [(46, 80), (42, 68), (50, 58), (44, 48), (48, 36)],
    [(64, 82), (60, 68), (68, 56), (62, 44), (66, 32)],
    [(82, 80), (78, 68), (86, 58), (80, 48), (84, 38)],
]
STEAM = [
    [(34, 28), (37, 22), (33, 16)],
    [(94, 28), (91, 22), (95, 16)],
]


def bowl_sd(x, y):
    rim = sd_round_rect(x, y, *RIM)
    if y >= BOWL_CY:
        bowl = sd_circle(x, y, BOWL_CX, BOWL_CY, BOWL_R)
    else:
        bowl = 1e9
    return min(rim, bowl)


def noodle_sd(x, y):
    best = 1e9
    for line in NOODLES + STEAM:
        for i in range(len(line) - 1):
            ax, ay = line[i]
            bx, by = line[i + 1]
            best = min(best, sd_segment(x, y, ax, ay, bx, by))
    return best - W / 2


def render(size, colored, rounded_bg=False, scale_geometry=1.0):
    pixels = []
    bg_top = (253, 243, 231)
    bg_bottom = (244, 214, 184)
    bowl_color = (138, 74, 42) if colored else (0, 0, 0)
    noodle_color = (224, 138, 103) if colored else (0, 0, 0)
    steam_color = (224, 138, 103) if colored else (0, 0, 0)

    scale = size / 128.0
    for py in range(size):
        row = []
        for px in range(size):
            x = (px + 0.5) / scale
            y = (py + 0.5) / scale
            color = (0.0, 0.0, 0.0, 0.0)

            if rounded_bg:
                sd = sd_round_rect(x, y, 64, 64, 64, 64, 27)
                a = coverage(sd)
                if a > 0:
                    t = clamp(y / 128.0)
                    bg = tuple(bg_top[i] + (bg_bottom[i] - bg_top[i]) * t for i in range(3))
                    color = over(color, bg, a)

            noodle_a = coverage(noodle_sd(x, y))
            if noodle_a > 0:
                color = over(color, noodle_color, noodle_a)

            bowl_a = coverage(bowl_sd(x, y))
            if bowl_a > 0:
                color = over(color, bowl_color, bowl_a)

            # A small green sprout on the app icon, for a bit of life.
            if colored and rounded_bg:
                sprout = sd_segment(x, y, 79, 40, 86, 31) - 1.8
                leaf = sd_circle(x, y, 88, 29, 3.4)
                a = max(coverage(sprout), coverage(leaf))
                if a > 0:
                    color = over(color, (90, 155, 92), a)

            r, g, b, a = color
            row.append((int(round(clamp(r, 0, 255))), int(round(clamp(g, 0, 255))), int(round(clamp(b, 0, 255))), int(round(clamp(a) * 255))))
        pixels.append(row)
    return pixels


def write_png(path, pixels):
    height = len(pixels)
    width = len(pixels[0])
    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for r, g, b, a in row:
            raw.extend((r, g, b, a))

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body) & 0xFFFFFFFF)

    png = b"\x89PNG\r\n\x1a\n"
    png += chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
    png += chunk(b"IDAT", zlib.compress(bytes(raw), 9))
    png += chunk(b"IEND", b"")
    with open(path, "wb") as handle:
        handle.write(png)
    print("wrote", os.path.relpath(path, ROOT), f"{width}x{height}")


def write_ico(path, sizes):
    images = []
    for size in sizes:
        tmp = tempfile.NamedTemporaryFile(suffix=".png", delete=False)
        tmp.close()
        write_png(tmp.name, render(size, colored=True, rounded_bg=True))
        with open(tmp.name, "rb") as handle:
            images.append((size, handle.read()))
        os.unlink(tmp.name)

    header = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = bytearray()
    payload = bytearray()
    for size, data in images:
        dim = 0 if size >= 256 else size
        entries.extend(struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset))
        payload.extend(data)
        offset += len(data)
    with open(path, "wb") as handle:
        handle.write(header + bytes(entries) + bytes(payload))
    print("wrote", os.path.relpath(path, ROOT), f"{len(images)} sizes")


def make_icns(png_1024, out):
    with tempfile.TemporaryDirectory() as tmp:
        iconset = os.path.join(tmp, "icon.iconset")
        os.makedirs(iconset)
        pairs = [
            (16, "icon_16x16.png"), (32, "icon_16x16@2x.png"),
            (32, "icon_32x32.png"), (64, "icon_32x32@2x.png"),
            (128, "icon_128x128.png"), (256, "icon_128x128@2x.png"),
            (256, "icon_256x256.png"), (512, "icon_256x256@2x.png"),
            (512, "icon_512x512.png"), (1024, "icon_512x512@2x.png"),
        ]
        for size, name in pairs:
            subprocess.run(["sips", "-z", str(size), str(size), png_1024, "--out", os.path.join(iconset, name)], check=True, capture_output=True)
        subprocess.run(["iconutil", "-c", "icns", iconset, "-o", out], check=True)
    print("wrote", os.path.relpath(out, ROOT))


def main():
    os.makedirs(ASSETS, exist_ok=True)
    os.makedirs(BUILD, exist_ok=True)
    os.makedirs(TAURI_ICONS, exist_ok=True)

    write_png(os.path.join(ASSETS, "trayTemplate.png"), render(16, colored=False))
    write_png(os.path.join(ASSETS, "trayTemplate@2x.png"), render(32, colored=False))
    write_png(os.path.join(ASSETS, "tray.png"), render(32, colored=True))
    write_png(os.path.join(ASSETS, "icon.png"), render(256, colored=True, rounded_bg=True))
    write_png(os.path.join(BUILD, "icon.png"), render(512, colored=True, rounded_bg=True))

    icon_1024 = os.path.join(TAURI_ICONS, "icon-1024.png")
    write_png(icon_1024, render(1024, colored=True, rounded_bg=True))
    write_png(os.path.join(TAURI_ICONS, "icon.png"), render(256, colored=True, rounded_bg=True))
    make_icns(icon_1024, os.path.join(TAURI_ICONS, "icon.icns"))
    write_ico(os.path.join(TAURI_ICONS, "icon.ico"), [16, 32, 48, 64, 128, 256])
    os.unlink(icon_1024)

    for name in ("trayTemplate.png", "trayTemplate@2x.png", "tray.png"):
        with open(os.path.join(ASSETS, name), "rb") as src, open(os.path.join(TAURI_ICONS, name), "wb") as dst:
            dst.write(src.read())


if __name__ == "__main__":
    main()
