"""Turn a large transparent pointer PNG into a TinyPaw pack.

Crops to the visible pixels plus a small margin, scales to a sprite of
SPRITE_HEIGHT pixels (sharp at the 96 px maximum size on 2x displays), finds the
pointing tip (topmost opaque pixel) as the hotspot and the bottom centre as the
wrist, then writes `paw.png` and `manifest.json` into the pack folder.

    python tools/prepare_pack.py <source.png> <pack-dir> <id> <name> <accent-hex>

Hotspot and wrist are a starting point; adjust them in the manifest against the
artwork if the automatic guess misses the claw.
"""

import json
import sys
from pathlib import Path

from PIL import Image

SPRITE_HEIGHT = 256
ALPHA_SOLID = 128
MARGIN = 0.02


def opaque_bbox(img):
    alpha = img.getchannel("A").point(lambda a: 255 if a >= ALPHA_SOLID else 0)
    return alpha.getbbox()


def tip_and_wrist(img):
    """Topmost solid pixel run (centre) and the bottom centre of the shape."""
    w, h = img.size
    px = img.load()
    for y in range(h):
        xs = [x for x in range(w) if px[x, y][3] >= ALPHA_SOLID]
        if xs:
            tip = ((xs[0] + xs[-1]) / 2 + 0.5, y + 0.5)
            break
    for y in range(h - 1, -1, -1):
        xs = [x for x in range(w) if px[x, y][3] >= ALPHA_SOLID]
        if xs:
            wrist = ((xs[0] + xs[-1]) / 2 + 0.5, y + 0.5)
            break
    return tip, wrist


def main():
    src, out, pack_id, name, accent = sys.argv[1:6]
    img = Image.open(src).convert("RGBA")
    left, top, right, bottom = opaque_bbox(img)
    pad = int(max(right - left, bottom - top) * MARGIN)
    img = img.crop((
        max(0, left - pad),
        max(0, top - pad),
        min(img.width, right + pad),
        min(img.height, bottom + pad),
    ))
    scale = SPRITE_HEIGHT / img.height
    img = img.resize((round(img.width * scale), SPRITE_HEIGHT), Image.LANCZOS)

    (tx, ty), (wx, wy) = tip_and_wrist(img)
    w, h = img.size
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    img.save(out / "paw.png", optimize=True)
    manifest = {
        "schemaVersion": 1,
        "id": pack_id,
        "name": name,
        "accent": accent,
        "pointer": {
            "idle": "paw.png",
            "hotspot": [round(tx / w, 4), round(ty / h, 4)],
            "wrist": [round(wx / w, 4), round(wy / h, 4)],
        },
    }
    (out / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"{pack_id}: {w}x{h}, hotspot {manifest['pointer']['hotspot']}, wrist {manifest['pointer']['wrist']}")


if __name__ == "__main__":
    main()
