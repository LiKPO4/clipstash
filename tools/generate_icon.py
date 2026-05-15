from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "assets"
PNG_PATH = ASSETS / "app_icon.png"
ICO_PATH = ASSETS / "app_icon.ico"


def rounded_rectangle(draw, box, radius, fill, outline=None, width=1):
    draw.rounded_rectangle(box, radius=radius, fill=fill, outline=outline, width=width)


def draw_icon(size):
    scale = size / 256
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))

    shadow = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    sd = ImageDraw.Draw(shadow)
    rounded_rectangle(
        sd,
        [int(22 * scale), int(26 * scale), int(234 * scale), int(238 * scale)],
        int(52 * scale),
        (15, 23, 42, 58),
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(max(1, int(7 * scale))))
    img.alpha_composite(shadow)

    draw = ImageDraw.Draw(img)
    rounded_rectangle(
        draw,
        [int(18 * scale), int(16 * scale), int(238 * scale), int(236 * scale)],
        int(54 * scale),
        (37, 99, 235, 255),
    )
    rounded_rectangle(
        draw,
        [int(28 * scale), int(26 * scale), int(228 * scale), int(226 * scale)],
        int(44 * scale),
        (59, 130, 246, 255),
    )

    # Clipboard sheet.
    rounded_rectangle(
        draw,
        [int(66 * scale), int(54 * scale), int(190 * scale), int(198 * scale)],
        int(22 * scale),
        (248, 250, 252, 255),
        (219, 234, 254, 255),
        max(1, int(4 * scale)),
    )

    # Top clip.
    rounded_rectangle(
        draw,
        [int(91 * scale), int(38 * scale), int(165 * scale), int(76 * scale)],
        int(16 * scale),
        (226, 232, 240, 255),
    )
    rounded_rectangle(
        draw,
        [int(106 * scale), int(31 * scale), int(150 * scale), int(55 * scale)],
        int(12 * scale),
        (15, 23, 42, 255),
    )

    # Stash layers.
    accent = (20, 184, 166, 255)
    rounded_rectangle(
        draw,
        [int(86 * scale), int(105 * scale), int(170 * scale), int(124 * scale)],
        int(9 * scale),
        accent,
    )
    rounded_rectangle(
        draw,
        [int(86 * scale), int(137 * scale), int(154 * scale), int(154 * scale)],
        int(8 * scale),
        (148, 163, 184, 255),
    )
    rounded_rectangle(
        draw,
        [int(86 * scale), int(166 * scale), int(138 * scale), int(183 * scale)],
        int(8 * scale),
        (148, 163, 184, 255),
    )

    # Small plus marker for quick capture.
    cx, cy = int(184 * scale), int(184 * scale)
    r = int(31 * scale)
    draw.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(15, 23, 42, 255))
    bar = max(3, int(8 * scale))
    length = int(32 * scale)
    rounded_rectangle(
        draw,
        [cx - length // 2, cy - bar // 2, cx + length // 2, cy + bar // 2],
        bar // 2,
        (255, 255, 255, 255),
    )
    rounded_rectangle(
        draw,
        [cx - bar // 2, cy - length // 2, cx + bar // 2, cy + length // 2],
        bar // 2,
        (255, 255, 255, 255),
    )

    return img


def main():
    ASSETS.mkdir(exist_ok=True)
    sizes = [16, 24, 32, 48, 64, 128, 256]
    image = draw_icon(256)
    image.save(PNG_PATH)
    image.save(ICO_PATH, format="ICO", sizes=[(size, size) for size in sizes])
    print(PNG_PATH)
    print(ICO_PATH)


if __name__ == "__main__":
    main()
