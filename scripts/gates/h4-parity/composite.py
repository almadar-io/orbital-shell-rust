"""composite.py web.png desktop.png out.png: side by side (web left, desktop right), labelled, plus a mean pixel delta."""
import os
import sys
from PIL import Image, ImageChops, ImageDraw, ImageStat

web, desk, out = sys.argv[1], sys.argv[2], sys.argv[3]
W, H = 1280, 900


def load(path):
    if os.path.exists(path):
        return Image.open(path).convert('RGB').resize((W, H))
    img = Image.new('RGB', (W, H), (255, 0, 255))
    ImageDraw.Draw(img).text((20, 20), f'MISSING {os.path.basename(path)}', fill=(0, 0, 0))
    return img


a, b = load(web), load(desk)
canvas = Image.new('RGB', (W * 2 + 10, H + 30), (40, 40, 40))
canvas.paste(a, (0, 30))
canvas.paste(b, (W + 10, 30))
d = ImageDraw.Draw(canvas)
d.text((10, 8), 'WEB', fill=(255, 255, 255))
d.text((W + 20, 8), 'DESKTOP', fill=(255, 255, 255))
delta = sum(ImageStat.Stat(ImageChops.difference(a, b)).mean) / 3
d.text((W - 200, 8), f'mean delta {delta:.1f}', fill=(255, 220, 0))
canvas = canvas.resize((canvas.width // 2, canvas.height // 2))
canvas.save(out)
print(f'{os.path.basename(out)} delta={delta:.1f}')
