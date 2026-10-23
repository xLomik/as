"""Genera crates/lince-gui/res/lince.ico (lupa blanca sobre fondo ambar).

Uso: python3 tools/gen-icon.py   (requiere Pillow)
"""
from pathlib import Path
from PIL import Image, ImageDraw

S = 256
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
d.rounded_rectangle([8, 8, S - 8, S - 8], radius=52, fill=(214, 128, 26, 255))
d.rounded_rectangle([8, 8, S - 8, S // 2], radius=52, fill=(232, 152, 48, 255))
d.rectangle([8, S // 2 - 52, S - 8, S // 2], fill=(232, 152, 48, 255))
# Lupa: aro y mango.
cx, cy, r = 106, 104, 58
d.ellipse([cx - r, cy - r, cx + r, cy + r], outline=(255, 255, 255, 255), width=22)
d.ellipse([cx - r + 30, cy - r + 26, cx - r + 52, cy - r + 48], fill=(255, 255, 255, 170))
d.line([(cx + 42, cy + 42), (206, 206)], fill=(255, 255, 255, 255), width=34)
d.ellipse([190, 190, 222, 222], fill=(255, 255, 255, 255))

out = Path(__file__).resolve().parent.parent / "crates/lince-gui/res/lince.ico"
img.save(out, sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
print(out)
