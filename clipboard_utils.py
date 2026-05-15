"""剪贴板操作工具（Windows）—— 增强版，支持多种剪贴板格式"""

import ctypes
from ctypes import wintypes
from io import BytesIO
from PIL import Image, ImageGrab
import pyperclip
import os

# ========== 常量 ==========
CF_TEXT = 1
CF_BITMAP = 2
CF_METAFILEPICT = 3
CF_SYLK = 4
CF_DIF = 5
CF_TIFF = 6
CF_OEMTEXT = 7
CF_DIB = 8
CF_PALETTE = 9
CF_PENDATA = 10
CF_RIFF = 11
CF_WAVE = 12
CF_UNICODETEXT = 13
CF_ENHMETAFILE = 14
CF_HDROP = 15
CF_LOCALE = 16
CF_DIBV5 = 17

GMEM_MOVEABLE = 0x0002
GMEM_SHARE = 0x2000

# 标准格式名称映射
CF_NAMES = {
    1: "CF_TEXT", 2: "CF_BITMAP", 3: "CF_METAFILEPICT", 4: "CF_SYLK",
    5: "CF_DIF", 6: "CF_TIFF", 7: "CF_OEMTEXT", 8: "CF_DIB",
    9: "CF_PALETTE", 10: "CF_PENDATA", 11: "CF_RIFF", 12: "CF_WAVE",
    13: "CF_UNICODETEXT", 14: "CF_ENHMETAFILE", 15: "CF_HDROP",
    16: "CF_LOCALE", 17: "CF_DIBV5",
}

user32 = ctypes.windll.user32
kernel32 = ctypes.windll.kernel32
gdi32 = ctypes.windll.gdi32
shell32 = ctypes.windll.shell32
ole32 = ctypes.windll.ole32

# 设置参数类型
user32.OpenClipboard.argtypes = [wintypes.HWND]
user32.OpenClipboard.restype = wintypes.BOOL
user32.CloseClipboard.restype = wintypes.BOOL
user32.EnumClipboardFormats.argtypes = [wintypes.UINT]
user32.EnumClipboardFormats.restype = wintypes.UINT
user32.GetClipboardData.argtypes = [wintypes.UINT]
user32.GetClipboardData.restype = wintypes.HANDLE
user32.IsClipboardFormatAvailable.argtypes = [wintypes.UINT]
user32.IsClipboardFormatAvailable.restype = wintypes.BOOL
user32.GetClipboardFormatNameW.argtypes = [wintypes.UINT, wintypes.LPWSTR, wintypes.INT]
user32.GetClipboardFormatNameW.restype = wintypes.INT
user32.RegisterClipboardFormatW.argtypes = [wintypes.LPCWSTR]
user32.RegisterClipboardFormatW.restype = wintypes.UINT
user32.EmptyClipboard.restype = wintypes.BOOL
user32.SetClipboardData.argtypes = [wintypes.UINT, wintypes.HANDLE]
user32.SetClipboardData.restype = wintypes.HANDLE

kernel32.GlobalAlloc.argtypes = [wintypes.UINT, ctypes.c_size_t]
kernel32.GlobalAlloc.restype = wintypes.HANDLE
kernel32.GlobalLock.argtypes = [wintypes.HANDLE]
kernel32.GlobalLock.restype = wintypes.LPVOID
kernel32.GlobalUnlock.argtypes = [wintypes.HANDLE]
kernel32.GlobalUnlock.restype = wintypes.BOOL
kernel32.GlobalSize.argtypes = [wintypes.HANDLE]
kernel32.GlobalSize.restype = ctypes.c_size_t


# ========== 诊断：收集剪贴板格式信息 ==========

def diagnose_clipboard():
    """
    返回当前剪贴板的所有格式信息列表。
    每个元素为 (format_id, format_name, data_size_or_status)
    """
    results = []
    try:
        if not user32.OpenClipboard(0):
            return [(0, "无法打开剪贴板", "")]

        fmt = 0
        while True:
            fmt = user32.EnumClipboardFormats(fmt)
            if fmt == 0:
                break

            name = CF_NAMES.get(fmt, "")
            if not name:
                buf = ctypes.create_unicode_buffer(256)
                length = user32.GetClipboardFormatNameW(fmt, buf, 256)
                if length > 0:
                    name = buf.value
                else:
                    name = f"未知格式({fmt})"

            # 尝试获取数据大小
            try:
                h_data = user32.GetClipboardData(fmt)
                if h_data:
                    size = kernel32.GlobalSize(h_data)
                    status = f"{size} bytes"
                else:
                    status = "无数据句柄"
            except Exception as e:
                status = f"读取失败: {e}"

            results.append((fmt, name, status))

        user32.CloseClipboard()
    except Exception as e:
        results.append((0, "诊断异常", str(e)))
        try:
            user32.CloseClipboard()
        except Exception:
            pass

    return results


# ========== 获取剪贴板图片 ==========

def get_clipboard_image():
    """
    从剪贴板获取图片，返回 (PIL Image, diagnostic_info) 或 (None, diagnostic_info)。
    diagnostic_info 是诊断信息字符串列表。
    """
    diagnostics = []

    # 1. Pillow 原生方法
    try:
        img = ImageGrab.grabclipboard()
        if isinstance(img, Image.Image):
            diagnostics.append("✅ Pillow ImageGrab 成功")
            return img, diagnostics
        elif isinstance(img, list) and len(img) > 0:
            path = img[0]
            if os.path.isfile(path):
                diagnostics.append(f"✅ Pillow 返回文件路径: {path}")
                return Image.open(path), diagnostics
            else:
                diagnostics.append(f"⚠️ Pillow 返回路径但文件不存在: {path}")
        else:
            diagnostics.append(f"⚠️ Pillow ImageGrab 返回: {type(img).__name__} = {img!r}")
    except Exception as e:
        diagnostics.append(f"❌ Pillow ImageGrab 失败: {e}")

    # 2. Win32 API 读取
    try:
        if not user32.OpenClipboard(0):
            diagnostics.append("❌ 无法打开剪贴板")
            return None, diagnostics

        # 枚举所有格式
        formats = []
        fmt = 0
        while True:
            fmt = user32.EnumClipboardFormats(fmt)
            if fmt == 0:
                break
            formats.append(fmt)

        diagnostics.append(f"📋 剪贴板格式数: {len(formats)}")

        # 注册常见格式
        png_fmt = user32.RegisterClipboardFormatW("PNG")
        image_png_fmt = user32.RegisterClipboardFormatW("image/png")
        image_bmp_fmt = user32.RegisterClipboardFormatW("image/bmp")
        image_jpeg_fmt = user32.RegisterClipboardFormatW("image/jpeg")
        html_fmt = user32.RegisterClipboardFormatW("HTML Format")

        # 尝试各个格式
        for fmt_id, fmt_name in [
            (png_fmt, "PNG"),
            (image_png_fmt, "image/png"),
            (image_bmp_fmt, "image/bmp"),
            (image_jpeg_fmt, "image/jpeg"),
            (CF_DIBV5, "CF_DIBV5"),
            (CF_DIB, "CF_DIB"),
            (CF_BITMAP, "CF_BITMAP"),
            (CF_TIFF, "CF_TIFF"),
            (CF_ENHMETAFILE, "CF_ENHMETAFILE"),
        ]:
            if fmt_id != 0 and fmt_id in formats:
                diagnostics.append(f"🔍 尝试 {fmt_name} (ID={fmt_id})...")
                img = None
                if fmt_name in ("PNG", "image/png", "image/bmp", "image/jpeg", "CF_TIFF"):
                    img = _read_clipboard_raw(fmt_id, diagnostics)
                elif fmt_name == "CF_DIBV5":
                    img = _read_clipboard_dibv5(diagnostics)
                elif fmt_name == "CF_DIB":
                    img = _read_clipboard_dib(diagnostics)
                elif fmt_name == "CF_BITMAP":
                    img = _read_clipboard_bitmap(diagnostics)
                elif fmt_name == "CF_ENHMETAFILE":
                    img = _read_clipboard_emf(diagnostics)

                if img:
                    diagnostics.append(f"✅ {fmt_name} 成功")
                    user32.CloseClipboard()
                    return img, diagnostics
                else:
                    diagnostics.append(f"❌ {fmt_name} 失败")

        # 3. 尝试 CF_HDROP（文件列表）
        if CF_HDROP in formats:
            diagnostics.append("🔍 尝试 CF_HDROP（文件列表）...")
            img = _read_clipboard_hdrop(diagnostics)
            if img:
                diagnostics.append("✅ CF_HDROP 成功")
                user32.CloseClipboard()
                return img, diagnostics
            else:
                diagnostics.append("❌ CF_HDROP 失败")

        # 4. 尝试 CF_UNICODETEXT（可能是文件路径）
        if CF_UNICODETEXT in formats:
            diagnostics.append("🔍 尝试 CF_UNICODETEXT...")
            try:
                h_data = user32.GetClipboardData(CF_UNICODETEXT)
                if h_data:
                    size = kernel32.GlobalSize(h_data)
                    ptr = kernel32.GlobalLock(h_data)
                    text = ctypes.wstring_at(ptr)
                    kernel32.GlobalUnlock(h_data)
                    diagnostics.append(f"   内容: {text[:100]!r}")
                    # 检查是否是图片文件路径
                    if text and os.path.isfile(text.strip().strip('\x00')):
                        path = text.strip().strip('\x00')
                        try:
                            img = Image.open(path)
                            diagnostics.append(f"✅ 从文本路径加载图片成功")
                            user32.CloseClipboard()
                            return img, diagnostics
                        except Exception as e:
                            diagnostics.append(f"❌ 从文本路径加载失败: {e}")
            except Exception as e:
                diagnostics.append(f"❌ CF_UNICODETEXT 读取失败: {e}")

        # 5. 遍历所有未知格式
        diagnostics.append("🔍 遍历所有未知格式...")
        for fmt_id in formats:
            if fmt_id > CF_DIBV5:  # 只尝试自定义格式
                name = CF_NAMES.get(fmt_id, "")
                if not name:
                    buf = ctypes.create_unicode_buffer(256)
                    length = user32.GetClipboardFormatNameW(fmt_id, buf, 256)
                    name = buf.value if length > 0 else f"未知({fmt_id})"

                diagnostics.append(f"   尝试 {name} (ID={fmt_id})...")
                img = _read_clipboard_raw(fmt_id, diagnostics)
                if img:
                    diagnostics.append(f"✅ {name} 成功")
                    user32.CloseClipboard()
                    return img, diagnostics

        diagnostics.append("❌ 所有格式均无法识别为图片")
        user32.CloseClipboard()
    except Exception as e:
        diagnostics.append(f"❌ Win32 API 异常: {e}")
        try:
            user32.CloseClipboard()
        except Exception:
            pass

    return None, diagnostics


def _read_clipboard_raw(fmt_id, diagnostics=None):
    """读取剪贴板原始字节，尝试用 Pillow 打开"""
    try:
        h_data = user32.GetClipboardData(fmt_id)
        if not h_data:
            return None
        size = kernel32.GlobalSize(h_data)
        if size == 0:
            return None
        ptr = kernel32.GlobalLock(h_data)
        if not ptr:
            return None
        data = ctypes.string_at(ptr, size)
        kernel32.GlobalUnlock(h_data)

        if diagnostics:
            diagnostics.append(f"   原始数据大小: {size} bytes")
            # 显示前几个字节用于调试
            if size >= 8:
                header = data[:8]
                diagnostics.append(f"   前8字节: {header.hex()}")

        # 尝试多种图片格式
        for fmt_name in ["PNG", "JPEG", "BMP", "GIF", "TIFF", "WEBP"]:
            try:
                img = Image.open(BytesIO(data))
                img.load()  # 强制加载验证
                return img
            except Exception:
                continue

        return None
    except Exception:
        return None


def _read_clipboard_dib(diagnostics=None):
    """读取 CF_DIB 格式"""
    try:
        h_data = user32.GetClipboardData(CF_DIB)
        if not h_data:
            return None
        size = kernel32.GlobalSize(h_data)
        ptr = kernel32.GlobalLock(h_data)
        data = ctypes.string_at(ptr, size)
        kernel32.GlobalUnlock(h_data)

        bmp_header = b'BM'
        bmp_header += (14 + len(data)).to_bytes(4, 'little')
        bmp_header += b'\x00\x00\x00\x00'
        bmp_header += (14).to_bytes(4, 'little')
        full_bmp = bmp_header + data
        return Image.open(BytesIO(full_bmp))
    except Exception as e:
        if diagnostics:
            diagnostics.append(f"   DIB 错误: {e}")
        return None


def _read_clipboard_dibv5(diagnostics=None):
    """读取 CF_DIBV5 格式"""
    try:
        h_data = user32.GetClipboardData(CF_DIBV5)
        if not h_data:
            return None
        size = kernel32.GlobalSize(h_data)
        ptr = kernel32.GlobalLock(h_data)
        data = bytes(ctypes.string_at(ptr, size))
        kernel32.GlobalUnlock(h_data)

        header_size = int.from_bytes(data[:4], 'little')
        bit_count = int.from_bytes(data[14:16], 'little')
        offset = 14 + header_size
        if bit_count <= 8:
            colors_used = int.from_bytes(data[32:36], 'little')
            if colors_used == 0:
                colors_used = 1 << bit_count
            offset += colors_used * 4
        elif bit_count in (16, 32):
            compression = int.from_bytes(data[16:20], 'little')
            if compression == 3:
                offset += 12

        bmp_header = b'BM'
        bmp_header += (14 + len(data)).to_bytes(4, 'little')
        bmp_header += b'\x00\x00\x00\x00'
        bmp_header += offset.to_bytes(4, 'little')
        full_bmp = bmp_header + data
        return Image.open(BytesIO(full_bmp))
    except Exception as e:
        if diagnostics:
            diagnostics.append(f"   DIBV5 错误: {e}")
        return None


def _read_clipboard_bitmap(diagnostics=None):
    """读取 CF_BITMAP（GDI 位图句柄）"""
    try:
        h_bitmap = user32.GetClipboardData(CF_BITMAP)
        if not h_bitmap:
            return None

        bm = ctypes.create_string_buffer(24)
        gdi32.GetObjectW(h_bitmap, 24, bm)
        width = int.from_bytes(bm[4:8], 'little', signed=True)
        height = int.from_bytes(bm[8:12], 'little', signed=True)

        hdc_screen = user32.GetDC(0)
        hdc_mem = gdi32.CreateCompatibleDC(hdc_screen)
        gdi32.SelectObject(hdc_mem, h_bitmap)

        bmi = ctypes.create_string_buffer(40)
        ctypes.memset(bmi, 0, 40)
        ctypes.memmove(bmi, ctypes.c_int32(40), 4)
        ctypes.memmove(ctypes.addressof(bmi) + 4, ctypes.c_int32(width), 4)
        ctypes.memmove(ctypes.addressof(bmi) + 8, ctypes.c_int32(-height), 4)
        ctypes.memmove(ctypes.addressof(bmi) + 12, ctypes.c_int16(1), 2)
        ctypes.memmove(ctypes.addressof(bmi) + 14, ctypes.c_int16(32), 2)
        ctypes.memmove(ctypes.addressof(bmi) + 16, ctypes.c_int32(0), 4)

        buf_size = width * abs(height) * 4
        buf = ctypes.create_string_buffer(buf_size)
        gdi32.GetDIBits(hdc_mem, h_bitmap, 0, abs(height), buf, bmi, 0)

        gdi32.DeleteDC(hdc_mem)
        user32.ReleaseDC(0, hdc_screen)

        bmp_header = b'BM'
        bmp_header += (14 + 40 + buf_size).to_bytes(4, 'little')
        bmp_header += b'\x00\x00\x00\x00'
        bmp_header += (14 + 40).to_bytes(4, 'little')
        full_bmp = bmp_header + bytes(bmi) + bytes(buf)
        return Image.open(BytesIO(full_bmp))
    except Exception as e:
        if diagnostics:
            diagnostics.append(f"   BITMAP 错误: {e}")
        return None


def _read_clipboard_emf(diagnostics=None):
    """尝试读取增强型图元文件"""
    try:
        h_emf = user32.GetClipboardData(CF_ENHMETAFILE)
        if not h_emf:
            return None
        # EMF 转图片较复杂，暂时返回 None
        return None
    except Exception:
        return None


# DROPFILES 结构
type
class DROPFILES(ctypes.Structure):
    _fields_ = [
        ("pFiles", wintypes.DWORD),
        ("pt", wintypes.POINT),
        ("fNC", wintypes.BOOL),
        ("fWide", wintypes.BOOL),
    ]


def _read_clipboard_hdrop(diagnostics=None):
    """读取 CF_HDROP 文件列表，尝试加载其中的图片文件"""
    try:
        h_data = user32.GetClipboardData(CF_HDROP)
        if not h_data:
            return None

        # 使用 shell32.DragQueryFile 读取文件列表
        drop = kernel32.GlobalLock(h_data)
        file_count = shell32.DragQueryFileW(drop, 0xFFFFFFFF, None, 0)

        for i in range(file_count):
            buf = ctypes.create_unicode_buffer(260)
            shell32.DragQueryFileW(drop, i, buf, 260)
            path = buf.value
            if path and os.path.isfile(path):
                # 检查是否是图片
                ext = os.path.splitext(path)[1].lower()
                if ext in ('.png', '.jpg', '.jpeg', '.gif', '.bmp', '.tiff', '.webp'):
                    try:
                        img = Image.open(path)
                        img.load()
                        kernel32.GlobalUnlock(h_data)
                        return img
                    except Exception:
                        continue

        kernel32.GlobalUnlock(h_data)
        return None
    except Exception as e:
        if diagnostics:
            diagnostics.append(f"   HDROP 错误: {e}")
        return None


# ========== 复制到剪贴板 ==========

def copy_text_to_clipboard(text):
    if text:
        pyperclip.copy(text)


def copy_image_to_clipboard(image_path):
    image = Image.open(image_path)
    output = BytesIO()
    image.convert("RGB").save(output, "BMP")
    data = output.getvalue()[14:]
    output.close()

    user32.OpenClipboard(0)
    user32.EmptyClipboard()
    h_global = kernel32.GlobalAlloc(GMEM_MOVEABLE, len(data))
    ptr = kernel32.GlobalLock(h_global)
    ctypes.memmove(ptr, data, len(data))
    kernel32.GlobalUnlock(h_global)
    user32.SetClipboardData(CF_DIB, h_global)
    user32.CloseClipboard()


# ========== 模拟按键 ==========
VK_CONTROL = 0x11
VK_V = 0x56
KEYEVENTF_KEYUP = 0x0002


def send_ctrl_v():
    user32.keybd_event(VK_CONTROL, 0, 0, 0)
    user32.keybd_event(VK_V, 0, 0, 0)
    user32.keybd_event(VK_V, 0, KEYEVENTF_KEYUP, 0)
    user32.keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0)
