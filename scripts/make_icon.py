"""Builds the app icon and README logos from the transparent source logo.

Usage: python scripts/make_icon.py <source.png>
"""
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
SIZE = 1024


def trimmed(src: Path) -> Image.Image:
    img = Image.open(src).convert("RGBA")
    return img.crop(img.getchannel("A").getbbox())


def recolor(img: Image.Image, rgb: tuple[int, int, int]) -> Image.Image:
    solid = Image.new("RGBA", img.size, rgb + (255,))
    solid.putalpha(img.getchannel("A"))
    return solid


def app_icon(owl: Image.Image) -> Image.Image:
    # Vertical gradient inside a rounded square, owl centered on top.
    bg = Image.new("RGBA", (SIZE, SIZE))
    top, bottom = (28, 33, 46), (9, 11, 16)
    draw = ImageDraw.Draw(bg)
    for y in range(SIZE):
        t = y / (SIZE - 1)
        draw.line([(0, y), (SIZE, y)], fill=tuple(round(a + (b - a) * t) for a, b in zip(top, bottom)) + (255,))
    mask = Image.new("L", (SIZE, SIZE))
    ImageDraw.Draw(mask).rounded_rectangle([(40, 40), (SIZE - 40, SIZE - 40)], radius=200, fill=255)
    icon = Image.new("RGBA", (SIZE, SIZE))
    icon.paste(bg, (0, 0), mask)

    target = int(SIZE * 0.62)
    scale = target / max(owl.size)
    fitted = owl.resize((round(owl.width * scale), round(owl.height * scale)), Image.LANCZOS)
    icon.alpha_composite(fitted, ((SIZE - fitted.width) // 2, (SIZE - fitted.height) // 2))
    return icon


def main() -> None:
    owl = trimmed(Path(sys.argv[1]))
    white = recolor(owl, (255, 255, 255))
    app_icon(white).save(ROOT / "assets" / "app-icon.png")
    white.save(ROOT / "assets" / "logo-light.png")
    recolor(owl, (17, 20, 28)).save(ROOT / "assets" / "logo-dark.png")
    favicon = app_icon(white).resize((256, 256), Image.LANCZOS)
    favicon.save(ROOT / "public" / "favicon.png")
    favicon.save(ROOT / "public" / "logo.png")


if __name__ == "__main__":
    main()
