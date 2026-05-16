import struct
from io import BytesIO
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


def write_ico(images, path):
    """将多个 Pillow Image 写入标准 BMP-based 多尺寸 ICO 文件（32bpp DIB）。"""
    entries = []
    data_blocks = []
    offset = 6 + len(images) * 16  # ICONDIR + ICONDIRENTRYs

    for img in images:
        w, h = img.size
        # 转为 BGRA 像素数据，自底向上
        bgra = bytearray()
        row_size = w * 4
        pad = (4 - (row_size % 4)) % 4
        for y in range(h - 1, -1, -1):
            for x in range(w):
                r, g, b, a = img.getpixel((x, y))
                bgra.extend([b, g, r, a])
            bgra.extend([0] * pad)

        # BITMAPINFOHEADER (40 bytes)
        dib = bytearray()
        dib.extend((40).to_bytes(4, "little"))               # biSize
        dib.extend(w.to_bytes(4, "little", signed=True))     # biWidth
        dib.extend((h * 2).to_bytes(4, "little", signed=True))  # biHeight (XOR + AND)
        dib.extend((1).to_bytes(2, "little"))                # biPlanes
        dib.extend((32).to_bytes(2, "little"))               # biBitCount
        dib.extend((0).to_bytes(4, "little"))                # biCompression = BI_RGB
        dib.extend((0).to_bytes(4, "little"))                # biSizeImage
        dib.extend((0).to_bytes(4, "little", signed=True))   # biXPelsPerMeter
        dib.extend((0).to_bytes(4, "little", signed=True))   # biYPelsPerMeter
        dib.extend((0).to_bytes(4, "little"))                # biClrUsed
        dib.extend((0).to_bytes(4, "little"))                # biClrImportant

        # AND 掩码（1bpp，32bpp 下全 0 即可，靠 alpha 通道透明）
        and_row = ((w + 31) // 32) * 4
        and_mask = bytes(and_row * h)

        icon_data = bytes(dib) + bytes(bgra) + and_mask

        dir_w = w if w < 256 else 0
        dir_h = h if h < 256 else 0
        entries.append(
            struct.pack(
                "<BBBBHHII",
                dir_w,
                dir_h,
                0,  # bColorCount
                0,  # bReserved
                1,  # wPlanes
                32,  # wBitCount
                len(icon_data),
                offset,
            )
        )
        data_blocks.append(icon_data)
        offset += len(icon_data)

    with open(path, "wb") as f:
        f.write(struct.pack("<HHH", 0, 1, len(images)))
        for entry in entries:
            f.write(entry)
        for data in data_blocks:
            f.write(data)


def main():
    ASSETS.mkdir(exist_ok=True)
    # 常用尺寸放前面，避免旧版程序或 Tk iconbitmap 只读取第一帧时使用过小图标
    sizes = [48, 32, 64, 24, 16, 128, 256]

    # PNG 用 256x256
    image_256 = draw_icon(256)
    image_256.save(PNG_PATH)

    # ICO 的每个尺寸独立绘制，避免缩放导致小图标模糊；保持 RGBA 保存 32bpp 图标
    ico_images = [draw_icon(size) for size in sizes]
    write_ico(ico_images, ICO_PATH)
    print(PNG_PATH)
    print(ICO_PATH)


if __name__ == "__main__":
    main()
