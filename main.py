import customtkinter as ctk
from PIL import Image, ImageDraw
import db
import os
import sys
import threading
import ctypes
import json
import subprocess
import textwrap
import time
from datetime import datetime, timezone
import urllib.error
import urllib.request
import winreg
from io import BytesIO
from clipboard_utils import (
    get_clipboard_image, get_clipboard_text, copy_text_to_clipboard,
    copy_image_to_clipboard, send_ctrl_v, diagnose_clipboard
)
from config import (
    load_settings, save_settings,
    get_hover_delay_ms, get_auto_archive_after_import, get_sort_order,
    get_launch_on_startup, get_show_hotkey, get_capture_hotkey,
    get_scroll_speed, get_app_font_size_delta
)

APP_NAME = "需求暂存站"
APP_VERSION = "v1.3.12"
APP_REPOSITORY = "LiKPO4/clipstash"
LATEST_RELEASE_API = f"https://api.github.com/repos/{APP_REPOSITORY}/releases/latest"
WINDOWS_APP_ID = f"LiKPO4.ClipStash.{APP_VERSION.lstrip('v')}"
STARTUP_REG_NAME = "ClipStash"
SORT_LABELS = {
    "newest": "最新优先",
    "oldest": "最早优先",
}
SORT_VALUES = {label: value for value, label in SORT_LABELS.items()}
SAFETY_NOTICE = (
    "避坑提醒：购买 Plus 会员请务必远离账号贩子，如xu166295906。"
    "所谓质保、低价、共享号风险很高，失效后可能拒绝补号、退款甚至拉黑；建议只通过官方渠道购买。"
)
pystray = None
keyboard = None
HAS_TRAY = None
_THUMBNAIL_CACHE = {}


def _parse_version(version_text):
    text = str(version_text or "").strip().lstrip("vV")
    parts = []
    for part in text.split("."):
        digits = ""
        for char in part:
            if char.isdigit():
                digits += char
            else:
                break
        parts.append(int(digits or 0))
    while len(parts) < 3:
        parts.append(0)
    return tuple(parts[:3])


def _fetch_latest_release():
    request = urllib.request.Request(
        LATEST_RELEASE_API,
        headers={
            "Accept": "application/vnd.github+json",
            "User-Agent": f"ClipStash/{APP_VERSION}",
        },
    )
    with urllib.request.urlopen(request, timeout=8) as response:
        return json.loads(response.read().decode("utf-8"))

def _resource_path(relative_path):
    base_path = getattr(sys, "_MEIPASS", os.path.dirname(os.path.abspath(__file__)))
    return os.path.join(base_path, relative_path)


# ========== 应用图标 ==========
def _ensure_app_icon() -> str:
    """返回窗口和任务栏使用的 ICO 图标路径。"""
    return _resource_path(os.path.join("assets", "app_icon.ico"))


def _load_app_icon_image(size=64):
    """返回托盘使用的 PNG 图标。"""
    icon_path = _resource_path(os.path.join("assets", "app_icon.png"))
    return Image.open(icon_path).convert("RGBA").resize((size, size), Image.Resampling.LANCZOS)


def _set_windows_app_id():
    try:
        ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID(WINDOWS_APP_ID)
    except Exception:
        pass


def _hicon_to_pil(hicon, size=64):
    """将 Windows HICON 转换为 Pillow RGBA Image"""
    try:
        import win32gui
        from PIL import Image
        import ctypes

        hdc = win32gui.GetDC(0)
        hdcMem = win32gui.CreateCompatibleDC(hdc)

        bmi = ctypes.create_string_buffer(40)
        ctypes.memset(bmi, 0, 40)
        ctypes.memmove(bmi, ctypes.c_int32(40), 4)
        ctypes.memmove(ctypes.addressof(bmi) + 4, ctypes.c_int32(size), 4)
        ctypes.memmove(ctypes.addressof(bmi) + 8, ctypes.c_int32(-size), 4)
        ctypes.memmove(ctypes.addressof(bmi) + 12, ctypes.c_int16(1), 2)
        ctypes.memmove(ctypes.addressof(bmi) + 14, ctypes.c_int16(32), 2)

        ppvBits = ctypes.c_void_p()
        hBitmap = ctypes.windll.gdi32.CreateDIBSection(
            hdcMem, bmi, 0, ctypes.byref(ppvBits), None, 0
        )
        ctypes.windll.gdi32.SelectObject(hdcMem, hBitmap)
        ctypes.windll.user32.DrawIconEx(hdcMem, 0, 0, hicon, size, size, 0, 0, 0x0003)

        buf = ctypes.string_at(ppvBits.value, size * size * 4)
        img = Image.frombuffer('RGBA', (size, size), buf, 'raw', 'BGRA', 0, 1)

        ctypes.windll.gdi32.DeleteObject(hBitmap)
        win32gui.DeleteDC(hdcMem)
        win32gui.ReleaseDC(0, hdc)
        return img
    except Exception:
        return None


def _get_icon_from_shfileinfo(path, large=True):
    """使用 SHGetFileInfo 获取 Windows 资源管理器显示的图标句柄"""
    try:
        shell32 = ctypes.windll.shell32
        SHGFI_ICON = 0x00000100
        SHGFI_LARGEICON = 0x00000000
        SHGFI_SMALLICON = 0x00000001
        
        class SHFILEINFO(ctypes.Structure):
            _fields_ = [
                ("hIcon", ctypes.c_void_p),
                ("iIcon", ctypes.c_int),
                ("dwAttributes", ctypes.c_uint),
                ("szDisplayName", ctypes.c_wchar * 260),
                ("szTypeName", ctypes.c_wchar * 80),
            ]
        
        sfi = SHFILEINFO()
        flags = SHGFI_ICON | (SHGFI_LARGEICON if large else SHGFI_SMALLICON)
        result = shell32.SHGetFileInfoW(path, 0, ctypes.byref(sfi), ctypes.sizeof(sfi), flags)
        if result and sfi.hIcon:
            return sfi.hIcon
    except Exception:
        pass
    return None


def _set_taskbar_icon(hwnd, icon_path):
    """设置任务栏图标：优先用 exe 内嵌资源，fallback 从 ICO 文件加载"""
    log_lines = ["_set_taskbar_icon called"]
    log_lines.append(f"icon_path: {icon_path}")
    log_lines.append(f"exists: {os.path.exists(icon_path)}")
    log_lines.append(f"sys.executable: {sys.executable}")
    hicon = None
    try:
        user32 = ctypes.windll.user32
        kernel32 = ctypes.windll.kernel32
        IMAGE_ICON = 1
        LR_LOADFROMFILE = 0x00000010
        WM_SETICON = 0x0080
        GCLP_HICON = -14
        GCLP_HICONSM = -34

        # 方法1: 从 exe 内嵌资源加载（最可靠，确保和文件资源管理器一致）
        try:
            import win32gui
            large_icons, small_icons = win32gui.ExtractIconEx(sys.executable, 0)
            log_lines.append(f"ExtractIconEx: large={len(large_icons)}, small={len(small_icons)}")
            if large_icons:
                hicon = large_icons[0]
                log_lines.append(f"Using ExtractIconEx large[0] = {hicon}")
        except Exception as e:
            log_lines.append(f"ExtractIconEx error: {e}")

        # 方法2: 从 ICO 文件加载
        if not hicon and os.path.exists(icon_path):
            hicon = user32.LoadImageW(None, icon_path, IMAGE_ICON, 0, 0, LR_LOADFROMFILE)
            log_lines.append(f"LoadImageW result: {hicon}")

        # 方法3: SHGetFileInfo fallback
        if not hicon:
            hicon = _get_icon_from_shfileinfo(sys.executable, large=True)
            log_lines.append(f"SHGetFileInfo result: {hicon}")

        if hicon:
            # WM_SETICON 设置窗口自身图标
            user32.SendMessageW(hwnd, WM_SETICON, 1, hicon)
            user32.SendMessageW(hwnd, WM_SETICON, 0, hicon)
            log_lines.append("WM_SETICON sent")
            # SetClassLongPtr 设置窗口类图标，影响任务栏
            try:
                user32.SetClassLongPtrW(hwnd, GCLP_HICON, hicon)
                user32.SetClassLongPtrW(hwnd, GCLP_HICONSM, hicon)
                log_lines.append("SetClassLongPtrW sent")
            except Exception as e:
                log_lines.append(f"SetClassLongPtrW error: {e}")
    except Exception as e:
        log_lines.append(f"Exception: {e}")
        import traceback
        log_lines.append(traceback.format_exc())
    finally:
        log_path = os.path.join(os.environ.get("TEMP", "C:\\temp"), "clipstash_icon.log")
        try:
            with open(log_path, "w", encoding="utf-8") as f:
                f.write("\n".join(log_lines))
        except Exception:
            pass


def _startup_command():
    if getattr(sys, "frozen", False):
        return subprocess.list2cmdline([sys.executable])
    python_exe = sys.executable
    pythonw_exe = os.path.join(os.path.dirname(python_exe), "pythonw.exe")
    if os.path.exists(pythonw_exe):
        python_exe = pythonw_exe
    return subprocess.list2cmdline([python_exe, os.path.abspath(__file__)])


def _set_launch_on_startup(enabled):
    try:
        with winreg.OpenKey(
            winreg.HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            0,
            winreg.KEY_SET_VALUE,
        ) as key:
            if enabled:
                winreg.SetValueEx(key, STARTUP_REG_NAME, 0, winreg.REG_SZ, _startup_command())
            else:
                try:
                    winreg.DeleteValue(key, STARTUP_REG_NAME)
                except FileNotFoundError:
                    pass
    except Exception:
        pass


def _normalize_hotkey(text):
    aliases = {
        "ctrl": "<ctrl>",
        "control": "<ctrl>",
        "<control>": "<ctrl>",
        "shift": "<shift>",
        "alt": "<alt>",
        "win": "<cmd>",
        "windows": "<cmd>",
        "cmd": "<cmd>",
        "space": "<space>",
        "enter": "<enter>",
        "esc": "<esc>",
        "escape": "<esc>",
    }
    parts = []
    for raw_part in str(text or "").replace(" ", "").split("+"):
        if not raw_part:
            continue
        part = raw_part.lower()
        parts.append(aliases.get(part, part if part.startswith("<") else part))
    return "+".join(parts)


MODIFIER_KEYSYMS = {
    "control_l", "control_r", "shift_l", "shift_r",
    "alt_l", "alt_r", "win_l", "win_r", "super_l", "super_r",
    "meta_l", "meta_r", "caps_lock", "num_lock", "scroll_lock",
}
MODIFIER_KEYCODES = {16, 17, 18, 91, 92}
KEY_ALIASES = {
    "return": "<enter>",
    "escape": "<esc>",
    "space": "<space>",
    "backspace": "<backspace>",
    "delete": "<delete>",
    "tab": "<tab>",
    "insert": "<insert>",
    "home": "<home>",
    "end": "<end>",
    "prior": "<page_up>",
    "next": "<page_down>",
    "left": "<left>",
    "right": "<right>",
    "up": "<up>",
    "down": "<down>",
}
VK_ALIASES = {
    8: "<backspace>",
    9: "<tab>",
    13: "<enter>",
    27: "<esc>",
    32: "<space>",
    33: "<page_up>",
    34: "<page_down>",
    35: "<end>",
    36: "<home>",
    37: "<left>",
    38: "<up>",
    39: "<right>",
    40: "<down>",
    45: "<insert>",
    46: "<delete>",
}


def _event_keycode(event):
    try:
        return int(getattr(event, "keycode", 0) or 0)
    except (TypeError, ValueError):
        return 0


def _main_key_from_event(event):
    key = str(getattr(event, "keysym", "") or "").lower()
    if key and key not in MODIFIER_KEYSYMS and key not in {"??", "unknown"}:
        if len(key) == 1:
            return key
        if key.startswith("f") and key[1:].isdigit():
            return f"<{key}>"
        return KEY_ALIASES.get(key, key)

    keycode = _event_keycode(event)
    if keycode in MODIFIER_KEYCODES:
        return ""
    if 65 <= keycode <= 90:
        return chr(keycode).lower()
    if 48 <= keycode <= 57:
        return chr(keycode)
    if 112 <= keycode <= 123:
        return f"<f{keycode - 111}>"
    if 96 <= keycode <= 105:
        return str(keycode - 96)
    return VK_ALIASES.get(keycode, "")


def _hotkey_from_event(event):
    key_text = _main_key_from_event(event)
    if not key_text:
        return ""
    state = getattr(event, "state", 0)
    parts = []
    if state & 0x0004:
        parts.append("<ctrl>")
    if state & 0x0001:
        parts.append("<shift>")
    if state & 0x20000 or state & 0x40000:
        parts.append("<alt>")
    if state & 0x0040:
        parts.append("<cmd>")
    parts.append(key_text)
    return "+".join(parts)


def _get_foreground_hwnd():
    try:
        return ctypes.windll.user32.GetForegroundWindow()
    except Exception:
        return None


def _app_dir():
    if getattr(sys, "frozen", False):
        return os.path.dirname(os.path.abspath(sys.executable))
    return os.getcwd()


def _is_onefile_bundle():
    """检测当前是否为 PyInstaller --onefile 模式。"""
    if not getattr(sys, "frozen", False):
        return False
    meipass = getattr(sys, "_MEIPASS", None)
    if not meipass:
        return False
    exe_dir = os.path.dirname(os.path.abspath(sys.executable))
    return os.path.normcase(meipass) != os.path.normcase(exe_dir)


def _load_tray_modules():
    global pystray, keyboard, HAS_TRAY
    if HAS_TRAY is not None:
        return HAS_TRAY
    try:
        import pystray as _pystray
        from pynput import keyboard as _keyboard
        pystray = _pystray
        keyboard = _keyboard
        HAS_TRAY = True
    except ImportError:
        HAS_TRAY = False
    return HAS_TRAY


def _looks_like_app_title(title):
    return (
        title.startswith(f"{APP_NAME} ")
        or title.startswith("ClipStash ")
        or "需求暂存站" in title
    ) and "@linjianglu" in title


def _version_from_title(title):
    for token in str(title or "").replace("@", " ").split():
        if token.lower().startswith("v"):
            parsed = _parse_version(token)
            if parsed != (0, 0, 0):
                return parsed
    return (0, 0, 0)


def _get_window_text(hwnd):
    user32 = ctypes.windll.user32
    length = user32.GetWindowTextLengthW(hwnd)
    if length <= 0:
        return ""
    buffer = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buffer, length + 1)
    return buffer.value


def _find_running_app_windows():
    user32 = ctypes.windll.user32
    windows = []
    enum_proc_type = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_void_p, ctypes.c_void_p)

    def enum_proc(hwnd, lparam):
        title = _get_window_text(hwnd)
        if title and _looks_like_app_title(title):
            pid = ctypes.c_ulong()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            windows.append({
                "hwnd": hwnd,
                "pid": pid.value,
                "title": title,
                "version": _version_from_title(title),
            })
        return True

    user32.EnumWindows(enum_proc_type(enum_proc), 0)
    windows.sort(key=lambda item: item["version"], reverse=True)
    return windows


def _show_window(hwnd):
    user32 = ctypes.windll.user32
    user32.ShowWindow(hwnd, 9)  # SW_RESTORE
    user32.SetForegroundWindow(hwnd)


def _terminate_window_process(hwnd):
    user32 = ctypes.windll.user32
    kernel32 = ctypes.windll.kernel32
    pid = ctypes.c_ulong()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
    if not pid.value:
        return False

    PROCESS_TERMINATE = 0x0001
    handle = kernel32.OpenProcess(PROCESS_TERMINATE, False, pid.value)
    if not handle:
        return False
    try:
        return bool(kernel32.TerminateProcess(handle, 0))
    finally:
        kernel32.CloseHandle(handle)


def _window_process_id(hwnd):
    if not hwnd:
        return 0
    try:
        pid = ctypes.c_ulong()
        ctypes.windll.user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        return pid.value
    except Exception:
        return 0


def _is_own_window(hwnd):
    return bool(hwnd) and _window_process_id(hwnd) == os.getpid()


def _kill_other_clipstash_processes():
    """通过可执行文件名匹配，终止所有其他 ClipStash / 需求暂存站进程。
    用于兜底杀死无窗口、无托盘的后台残留实例。"""
    try:
        import ctypes
        from ctypes import wintypes

        kernel32 = ctypes.windll.kernel32
        psapi = ctypes.windll.psapi

        current_pid = kernel32.GetCurrentProcessId()

        cb_needed = wintypes.DWORD()
        pids = (wintypes.DWORD * 1024)()
        if not psapi.EnumProcesses(pids, ctypes.sizeof(pids), ctypes.byref(cb_needed)):
            return

        num_pids = cb_needed.value // ctypes.sizeof(wintypes.DWORD)
        killed = 0

        for i in range(num_pids):
            pid = pids[i]
            if pid == 0 or pid == current_pid:
                continue

            h_process = kernel32.OpenProcess(0x0410, False, pid)  # PROCESS_QUERY_INFORMATION | PROCESS_VM_READ
            if not h_process:
                continue

            try:
                filename = ctypes.create_unicode_buffer(512)
                size = wintypes.DWORD(512)
                if kernel32.QueryFullProcessImageNameW(h_process, 0, filename, ctypes.byref(size)):
                    path = filename.value
                    name = os.path.basename(path).lower()
                    if "clipstash" in name or "需求暂存站" in name:
                        h_terminate = kernel32.OpenProcess(0x0001, False, pid)  # PROCESS_TERMINATE
                        if h_terminate:
                            kernel32.TerminateProcess(h_terminate, 0)
                            kernel32.CloseHandle(h_terminate)
                            killed += 1
            finally:
                kernel32.CloseHandle(h_process)

    except Exception:
        pass


# ========== 单例锁（Windows 命名互斥量）==========
_mutex_handle = None

def _ensure_single_instance():
    """确保只有一个实例运行；检测到旧实例时先全部杀死，再启动新实例。"""
    global _mutex_handle
    kernel32 = ctypes.windll.kernel32
    kernel32.CreateMutexW.argtypes = [ctypes.c_void_p, ctypes.c_bool, ctypes.c_wchar_p]
    kernel32.CreateMutexW.restype = ctypes.c_void_p
    kernel32.GetLastError.restype = ctypes.c_uint32
    kernel32.ReleaseMutex.argtypes = [ctypes.c_void_p]
    kernel32.CloseHandle.argtypes = [ctypes.c_void_p]

    _mutex_handle = kernel32.CreateMutexW(None, False, "ClipStash_SingleInstance_Mutex")
    if kernel32.GetLastError() == 183:  # ERROR_ALREADY_EXISTS
        # 1) 先通过窗口句柄杀（快速路径）
        running_windows = _find_running_app_windows()
        for window in running_windows:
            _terminate_window_process(window["hwnd"])
        # 2) 再兜底：通过进程名杀死所有遗漏的旧进程（无窗口/无托盘的后台实例）
        _kill_other_clipstash_processes()

        if _mutex_handle:
            kernel32.CloseHandle(_mutex_handle)
            _mutex_handle = None

        deadline = time.time() + 1.0
        while True:
            _mutex_handle = kernel32.CreateMutexW(None, False, "ClipStash_SingleInstance_Mutex")
            if kernel32.GetLastError() != 183:
                return True
            if _mutex_handle:
                kernel32.CloseHandle(_mutex_handle)
                _mutex_handle = None
            if time.time() >= deadline:
                break
            time.sleep(0.05)

        if _mutex_handle:
            kernel32.CloseHandle(_mutex_handle)
            _mutex_handle = None
        return False
    return True


# ========== 主题配置 ==========
COLORS = {
    "primary": "#3B82F6",
    "primary_hover": "#2563EB",
    "danger": "#EF4444",
    "danger_hover": "#DC2626",
    "bg": "#F8FAFC",
    "card": "#FFFFFF",
    "text": "#0F172A",
    "text_secondary": "#475569",
    "text_hint": "#64748B",
    "border": "#CBD5E1",
    "tag_bg": "#F1F5F9",
    "tab_hover": "#E8F0FE",
}

APP_FONT_FAMILY = "Microsoft YaHei UI"


def _font(size=12, weight=None, family=APP_FONT_FAMILY):
    adjusted_size = max(8, int(size) + int(get_app_font_size_delta()))
    kwargs = {"family": family, "size": adjusted_size}
    if weight:
        kwargs["weight"] = weight
    return ctk.CTkFont(**kwargs)


# ========== 工具函数 ==========
def _format_local_time(utc_str):
    """将 UTC 时间字符串转换为本地时区字符串"""
    if not utc_str:
        return ""
    try:
        dt = datetime.strptime(str(utc_str).split(".")[0], "%Y-%m-%d %H:%M:%S")
        dt = dt.replace(tzinfo=timezone.utc)
        local_dt = dt.astimezone()
        return local_dt.strftime("%Y-%m-%d %H:%M:%S")
    except Exception:
        return str(utc_str)


def _resize_keep_ratio(pil_image, max_w, max_h):
    w, h = pil_image.size
    ratio = min(max_w / w, max_h / h, 1.0)
    return pil_image.resize((int(w * ratio), int(h * ratio)), Image.Resampling.LANCZOS)


def _pil_to_ctk(pil_image, max_w=100, max_h=100):
    if pil_image.mode == "RGBA":
        bg = Image.new("RGB", pil_image.size, (255, 255, 255))
        bg.paste(pil_image, mask=pil_image.split()[3])
        pil_image = bg
    elif pil_image.mode != "RGB":
        pil_image = pil_image.convert("RGB")
    thumb = _resize_keep_ratio(pil_image, max_w, max_h)
    w, h = thumb.size
    return ctk.CTkImage(light_image=thumb, size=(w, h))


def _cached_ctk_image(image_path, max_w=100, max_h=100):
    """缓存列表缩略图，避免切换队列时重复解码和缩放图片。"""
    try:
        stat = os.stat(image_path)
        key = (image_path, max_w, max_h, stat.st_mtime_ns, stat.st_size)
        cached = _THUMBNAIL_CACHE.get(key)
        if cached:
            return cached
        with Image.open(image_path) as pil_image:
            ctk_img = _pil_to_ctk(pil_image, max_w, max_h)
        if len(_THUMBNAIL_CACHE) > 128:
            _THUMBNAIL_CACHE.pop(next(iter(_THUMBNAIL_CACHE)))
        _THUMBNAIL_CACHE[key] = ctk_img
        return ctk_img
    except Exception:
        return None


def center_window(window, parent):
    """将窗口居中到父窗口，无闪烁版本"""
    window.update_idletasks()
    pw, ph = parent.winfo_width(), parent.winfo_height()
    px, py = parent.winfo_x(), parent.winfo_y()
    ww, wh = window.winfo_width(), window.winfo_height()
    x = px + (pw - ww) // 2
    y = py + (ph - wh) // 2
    window.geometry(f"+{x}+{y}")


def _wrap_preview_text(text, width=58, max_lines=5):
    lines = []
    for raw_line in str(text or "").splitlines() or [""]:
        wrapped = textwrap.wrap(
            raw_line,
            width=width,
            break_long_words=True,
            break_on_hyphens=False,
            replace_whitespace=False,
            drop_whitespace=False,
        )
        lines.extend(wrapped or [""])
        if len(lines) >= max_lines:
            break
    if len(lines) > max_lines:
        lines = lines[:max_lines]
    return "\n".join(lines)


# ========== 悬浮预览 ==========
class HoverPreview(ctk.CTkToplevel):
    def __init__(self, parent, trigger_widget, image_path):
        super().__init__(parent)
        self.overrideredirect(True)
        self.attributes("-topmost", True)
        self.configure(fg_color=COLORS["card"])
        self.trigger_widget = trigger_widget

        try:
            pil_img = Image.open(image_path)
            if pil_img.mode != "RGB":
                pil_img = pil_img.convert("RGB")
            # 原尺寸预览，最大不超过 1000x1000
            w, h = pil_img.size
            if w <= 1000 and h <= 1000:
                thumb = pil_img
            else:
                thumb = _resize_keep_ratio(pil_img, 1000, 1000)
            tw, th = thumb.size
            ctk_img = ctk.CTkImage(light_image=thumb, size=(tw, th))
            lbl = ctk.CTkLabel(self, image=ctk_img, text="")
            lbl.pack(padx=4, pady=4)
            lbl.image = ctk_img
            self.geometry(f"{tw + 8}x{th + 8}")
        except Exception:
            self.destroy()
            return

        self._position_near(trigger_widget)

    def _position_near(self, widget):
        self.update_idletasks()
        pw = self.winfo_width()
        ph = self.winfo_height()
        wx = widget.winfo_rootx()
        wy = widget.winfo_rooty()
        ww = widget.winfo_width()
        wh = widget.winfo_height()

        user32 = ctypes.windll.user32
        sw, sh = user32.GetSystemMetrics(0), user32.GetSystemMetrics(1)

        nx = wx + ww + 10
        ny = wy
        if nx + pw > sw:
            nx = wx
            ny = wy + wh + 10
            if ny + ph > sh:
                ny = wy - ph - 10
        if ny < 0: ny = 10
        if nx < 0: nx = 10
        self.geometry(f"+{nx}+{ny}")


class HoverPreviewMixin:
    def __init__(self):
        self._hover_preview = None
        self._hover_after_id = None
        self._hover_delay_ms = get_hover_delay_ms()

    def bind_hover_preview(self, widget, image_path):
        widget.bind("<Enter>", lambda e, p=image_path, w=widget: self._on_hover_enter(e, w, p))
        widget.bind("<Leave>", lambda e: self._on_hover_leave())

    def _on_hover_enter(self, event, widget, image_path):
        self._hover_delay_ms = get_hover_delay_ms()
        self._hover_image_path = image_path
        self._hover_widget = widget
        self._hover_after_id = widget.after(
            self._hover_delay_ms, lambda: self._show_preview()
        )

    def _on_hover_leave(self):
        if self._hover_after_id:
            try:
                self.after_cancel(self._hover_after_id)
            except Exception:
                pass
            self._hover_after_id = None
        self._hide_preview()

    def _show_preview(self):
        self._hover_after_id = None
        if hasattr(self, '_hover_image_path') and self._hover_image_path:
            self._hide_preview()
            try:
                self._hover_preview = HoverPreview(
                    self, self._hover_widget, self._hover_image_path
                )
            except Exception:
                pass

    def _hide_preview(self):
        if self._hover_preview:
            try:
                if self._hover_preview.winfo_exists():
                    self._hover_preview.destroy()
            except Exception:
                pass
            self._hover_preview = None


# ========== 诊断弹窗 ==========
class DiagnoseDialog(ctk.CTkToplevel):
    def __init__(self, parent, diagnostics):
        super().__init__(parent)
        self.title("剪贴板诊断")
        self.geometry("480x360")
        self.transient(parent)
        self.grab_set()
        self.configure(fg_color=COLORS["bg"])
        self._parent = parent

        frame = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=12)
        frame.pack(fill="both", expand=True, padx=12, pady=12)

        ctk.CTkLabel(
            frame, text="剪贴板诊断信息",
            font=_font(15, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=12, pady=(12, 8), anchor="w")
        ctk.CTkLabel(
            frame, text="未能识别剪贴板中的图片，以下是详细诊断：",
            font=_font(12), text_color=COLORS["text_secondary"]
        ).pack(padx=12, anchor="w")

        textbox = ctk.CTkTextbox(
            frame, wrap="word", font=_font(11),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"]
        )
        textbox.pack(fill="both", expand=True, padx=12, pady=8)
        textbox.insert("1.0", "\n".join(diagnostics))
        textbox.configure(state="disabled")

        ctk.CTkButton(
            frame, text="关闭", width=80, height=32,
            font=_font(12),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._close
        ).pack(pady=(0, 12))

        self.protocol("WM_DELETE_WINDOW", self._close)
        self._place_centered()

    def _close(self):
        self.grab_release()
        self.destroy()

    def _place_centered(self):
        """先隐藏计算位置再显示，避免闪烁"""
        self.withdraw()
        self.update_idletasks()
        center_window(self, self._parent)
        self.deiconify()


# ========== 设置对话框 ==========
class SettingsDialog(ctk.CTkToplevel):
    def __init__(self, parent, on_save=None):
        super().__init__(parent)
        self.title("设置")
        self.geometry("420x680")
        self.resizable(False, False)
        self.transient(parent)
        self.configure(fg_color=COLORS["bg"])
        self.on_save = on_save
        self._parent = parent

        self.settings = load_settings()
        self._hotkey_capture_key = None
        self._hotkey_capture_listener = None
        self._hotkey_capture_modifiers = set()

        outer_frame = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=12)
        outer_frame.pack(fill="both", expand=True, padx=16, pady=16)
        self._settings_outer_frame = outer_frame

        frame = ctk.CTkScrollableFrame(
            outer_frame,
            fg_color="transparent",
            scrollbar_button_color=COLORS["primary"],
            scrollbar_button_hover_color=COLORS["primary_hover"],
        )
        frame.pack(fill="both", expand=True, padx=0, pady=(0, 4))

        ctk.CTkLabel(
            frame, text="设置",
            font=_font(16, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=16, pady=(16, 0), anchor="w")

        # 悬浮延迟
        delay_frame = ctk.CTkFrame(frame, fg_color="transparent")
        delay_frame.pack(fill="x", padx=16, pady=(8, 2))
        ctk.CTkLabel(
            delay_frame, text="悬浮预览延迟",
            font=_font(13), text_color=COLORS["text"]
        ).pack(side="left")
        initial_delay = self.settings.get("hover_delay_ms", 800) / 1000.0
        self.delay_label = ctk.CTkLabel(
            delay_frame, text=f"{initial_delay:.1f} 秒",
            font=_font(13, weight="bold"),
            text_color=COLORS["primary"], width=60
        )
        self.delay_label.pack(side="right")

        self.slider = ctk.CTkSlider(
            frame, from_=0.2, to=3.0, number_of_steps=28,
            command=self._on_slider_change,
            height=18, button_length=18,
            fg_color=COLORS["border"],
            progress_color=COLORS["primary"],
            button_color=COLORS["primary"],
            button_hover_color=COLORS["primary_hover"],
        )
        self.slider.set(initial_delay)
        self.slider.pack(fill="x", padx=16, pady=(0, 2))
        ctk.CTkLabel(
            frame, text="鼠标放在图片上多久后显示预览",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        # 滚动速度
        speed_frame = ctk.CTkFrame(frame, fg_color="transparent")
        speed_frame.pack(fill="x", padx=16, pady=(8, 2))
        ctk.CTkLabel(
            speed_frame, text="滚动速度",
            font=_font(13), text_color=COLORS["text"]
        ).pack(side="left")
        self.speed_label = ctk.CTkLabel(
            speed_frame, text="",
            font=_font(13, weight="bold"),
            text_color=COLORS["primary"], width=60
        )
        self.speed_label.pack(side="right")

        self.speed_slider = ctk.CTkSlider(
            frame, from_=1, to=5, number_of_steps=4,
            command=self._on_speed_slider_change,
            height=18, button_length=18,
            fg_color=COLORS["border"],
            progress_color=COLORS["primary"],
            button_color=COLORS["primary"],
            button_hover_color=COLORS["primary_hover"],
        )
        initial_speed = self.settings.get("scroll_speed", 2)
        self.speed_slider.set(initial_speed)
        self._on_speed_slider_change(initial_speed)
        self.speed_slider.pack(fill="x", padx=16, pady=(0, 2))
        ctk.CTkLabel(
            frame, text="鼠标滚轮每次滚动的行数",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        # 应用内文字大小
        font_frame = ctk.CTkFrame(frame, fg_color="transparent")
        font_frame.pack(fill="x", padx=16, pady=(8, 2))
        ctk.CTkLabel(
            font_frame, text="应用内文字大小",
            font=_font(13), text_color=COLORS["text"]
        ).pack(side="left")
        self.font_size_label = ctk.CTkLabel(
            font_frame, text="",
            font=_font(13, weight="bold"),
            text_color=COLORS["primary"], width=60
        )
        self.font_size_label.pack(side="right")

        self.font_size_slider = ctk.CTkSlider(
            frame, from_=-2, to=4, number_of_steps=6,
            command=self._on_font_size_slider_change,
            height=18, button_length=18,
            fg_color=COLORS["border"],
            progress_color=COLORS["primary"],
            button_color=COLORS["primary"],
            button_hover_color=COLORS["primary_hover"],
        )
        initial_font_delta = self.settings.get("app_font_size_delta", 0)
        self.font_size_slider.set(initial_font_delta)
        self._on_font_size_slider_change(initial_font_delta)
        self.font_size_slider.pack(fill="x", padx=16, pady=(0, 2))
        ctk.CTkLabel(
            frame, text="调整消息列表、按钮和状态栏文字",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        # 排序
        sort_frame = ctk.CTkFrame(frame, fg_color="transparent")
        sort_frame.pack(fill="x", padx=16, pady=(8, 2))
        ctk.CTkLabel(
            sort_frame, text="消息排序",
            font=_font(13), text_color=COLORS["text"]
        ).pack(side="left")
        sort_value = self.settings.get("sort_order", "newest")
        self.sort_var = ctk.StringVar(value=SORT_LABELS.get(sort_value, "最新优先"))
        sort_menu = ctk.CTkOptionMenu(
            sort_frame, variable=self.sort_var,
            values=list(SORT_VALUES.keys()), width=120, height=28,
            font=_font(12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"],
            button_color=COLORS["primary"], button_hover_color=COLORS["primary_hover"],
            dropdown_fg_color=COLORS["card"],
            dropdown_text_color=COLORS["text"],
            dropdown_hover_color=COLORS["tag_bg"],
        )
        sort_menu.pack(side="right")

        # 自动归档
        self.archive_var = ctk.BooleanVar(
            value=self.settings.get("auto_archive_after_import", False)
        )
        archive_switch = ctk.CTkSwitch(
            frame, text="快速导入后自动归档",
            variable=self.archive_var,
            font=_font(13), text_color=COLORS["text"],
            progress_color=COLORS["primary"],
            button_color=COLORS["primary"],
            button_hover_color=COLORS["primary_hover"],
        )
        archive_switch.pack(fill="x", padx=16, pady=(8, 0))
        ctk.CTkLabel(
            frame, text="导入完成后自动将消息移入已归档",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        self.startup_var = ctk.BooleanVar(
            value=self.settings.get("launch_on_startup", False)
        )
        startup_switch = ctk.CTkSwitch(
            frame, text="开机自启动",
            variable=self.startup_var,
            font=_font(13), text_color=COLORS["text"],
            progress_color=COLORS["primary"],
            button_color=COLORS["primary"],
            button_hover_color=COLORS["primary_hover"],
        )
        startup_switch.pack(fill="x", padx=16, pady=(8, 0))
        ctk.CTkLabel(
            frame, text="登录 Windows 后自动启动需求暂存站",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        ctk.CTkLabel(
            frame, text="呼出界面快捷键",
            font=_font(13), text_color=COLORS["text"]
        ).pack(fill="x", padx=16, pady=(8, 4), anchor="w")
        self.show_hotkey_var = ctk.StringVar(
            value=self.settings.get("show_hotkey", "<ctrl>+<shift>+v")
        )
        self.show_hotkey_entry = ctk.CTkEntry(
            frame, textvariable=self.show_hotkey_var,
            font=_font(12), height=30,
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"],
            border_color=COLORS["border"], corner_radius=8,
        )
        self.show_hotkey_entry.pack(fill="x", padx=16)
        self._bind_hotkey_entry(self.show_hotkey_entry, self.show_hotkey_var, "show_hotkey")

        ctk.CTkLabel(
            frame, text="导入当前剪切板快捷键",
            font=_font(13), text_color=COLORS["text"]
        ).pack(fill="x", padx=16, pady=(8, 4), anchor="w")
        self.capture_hotkey_var = ctk.StringVar(
            value=self.settings.get("capture_hotkey", "<ctrl>+<alt>+v")
        )
        self.capture_hotkey_entry = ctk.CTkEntry(
            frame, textvariable=self.capture_hotkey_var,
            font=_font(12), height=30,
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"],
            border_color=COLORS["border"], corner_radius=8,
        )
        self.capture_hotkey_entry.pack(fill="x", padx=16)
        self._bind_hotkey_entry(self.capture_hotkey_entry, self.capture_hotkey_var, "capture_hotkey")
        ctk.CTkLabel(
            frame, text="点击输入框后直接按下组合键，会立即保存",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        self.update_button = ctk.CTkButton(
            frame, text="检查更新", height=32,
            font=_font(12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._check_updates
        )
        self.update_button.pack(fill="x", padx=16, pady=(8, 0))
        self.update_status_label = ctk.CTkLabel(
            frame, text="",
            font=_font(11), text_color=COLORS["text_hint"],
        )
        self.update_status_label.pack(fill="x", padx=16, pady=(2, 0), anchor="w")

        # 按钮
        btn_frame = ctk.CTkFrame(self._settings_outer_frame, fg_color="transparent")
        btn_frame.pack(fill="x", padx=16, pady=(4, 12))
        ctk.CTkButton(
            btn_frame, text="取消", width=68, height=32,
            font=_font(12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._close
        ).pack(side="right", padx=(6, 0))
        ctk.CTkButton(
            btn_frame, text="保存", width=68, height=32,
            font=_font(12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._save
        ).pack(side="right")

        self.protocol("WM_DELETE_WINDOW", self._close)
        self._place_centered()
        self.grab_set()  # 在窗口显示并居中后再 grab，避免 withdraw/deiconify 破坏 grab 状态

    def _on_slider_change(self, value):
        self.delay_label.configure(text=f"{value:.1f} 秒")

    def _on_speed_slider_change(self, value):
        self.speed_label.configure(text=f"{int(value)} 行")

    def _on_font_size_slider_change(self, value):
        delta = int(value)
        if delta == 0:
            text = "标准"
        elif delta > 0:
            text = f"+{delta}"
        else:
            text = str(delta)
        self.font_size_label.configure(text=text)

    def _bind_hotkey_entry(self, entry, variable, setting_key):
        def arm_capture(event=None):
            self._start_hotkey_capture(entry, variable, setting_key)
            return "break"

        entry.bind("<Button-1>", arm_capture)
        entry.bind("<FocusIn>", arm_capture)
        entry.bind("<KeyPress>", lambda event: "break")

    def _start_hotkey_capture(self, entry, variable, setting_key):
        self._stop_hotkey_capture()
        if not _load_tray_modules():
            variable.set("快捷键模块不可用")
            return

        self._hotkey_capture_key = setting_key
        self._hotkey_capture_modifiers = set()
        variable.set("请按下快捷键...")
        entry.focus_set()

        def finish(hotkey):
            if self._hotkey_capture_key != setting_key:
                return
            self._hotkey_capture_key = None
            self._hotkey_capture_listener = None
            self._hotkey_capture_modifiers = set()
            variable.set(hotkey)
            self.settings[setting_key] = hotkey
            save_settings(self.settings)
            if self.on_save:
                self.on_save()
            self.focus_set()

        def on_press(key):
            modifier = self._pynput_modifier_name(key)
            if modifier:
                self._hotkey_capture_modifiers.add(modifier)
                return

            main_key = self._pynput_main_key(key)
            if not main_key:
                return

            parts = []
            for modifier_name in ("<ctrl>", "<shift>", "<alt>", "<cmd>"):
                if modifier_name in self._hotkey_capture_modifiers:
                    parts.append(modifier_name)
            parts.append(main_key)
            hotkey = "+".join(parts)
            self.after(0, lambda: finish(hotkey))
            return False

        def on_release(key):
            modifier = self._pynput_modifier_name(key)
            if modifier:
                self._hotkey_capture_modifiers.discard(modifier)

        self._hotkey_capture_listener = keyboard.Listener(
            on_press=on_press,
            on_release=on_release,
            suppress=False
        )
        self._hotkey_capture_listener.start()

    def _stop_hotkey_capture(self):
        listener = self._hotkey_capture_listener
        self._hotkey_capture_listener = None
        self._hotkey_capture_key = None
        self._hotkey_capture_modifiers = set()
        if listener:
            try:
                listener.stop()
            except Exception:
                pass

    def _pynput_modifier_name(self, key):
        modifier_map = {
            keyboard.Key.ctrl: "<ctrl>",
            keyboard.Key.ctrl_l: "<ctrl>",
            keyboard.Key.ctrl_r: "<ctrl>",
            keyboard.Key.shift: "<shift>",
            keyboard.Key.shift_l: "<shift>",
            keyboard.Key.shift_r: "<shift>",
            keyboard.Key.alt: "<alt>",
            keyboard.Key.alt_l: "<alt>",
            keyboard.Key.alt_r: "<alt>",
            keyboard.Key.alt_gr: "<alt>",
            keyboard.Key.cmd: "<cmd>",
            keyboard.Key.cmd_l: "<cmd>",
            keyboard.Key.cmd_r: "<cmd>",
        }
        return modifier_map.get(key, "")

    def _pynput_main_key(self, key):
        vk = getattr(key, "vk", None)
        if isinstance(vk, int):
            if 65 <= vk <= 90:
                return chr(vk).lower()
            if 48 <= vk <= 57:
                return chr(vk)
            if 96 <= vk <= 105:
                return str(vk - 96)
            if 112 <= vk <= 123:
                return f"<f{vk - 111}>"

        char = getattr(key, "char", None)
        if char and char.isprintable() and len(char) == 1:
            return char.lower()

        key_name = getattr(key, "name", "")
        aliases = {
            "space": "<space>",
            "enter": "<enter>",
            "esc": "<esc>",
            "backspace": "<backspace>",
            "delete": "<delete>",
            "tab": "<tab>",
            "insert": "<insert>",
            "home": "<home>",
            "end": "<end>",
            "page_up": "<page_up>",
            "page_down": "<page_down>",
            "left": "<left>",
            "right": "<right>",
            "up": "<up>",
            "down": "<down>",
        }
        if key_name.startswith("f") and key_name[1:].isdigit():
            return f"<{key_name}>"
        return aliases.get(key_name, "")

    def _check_updates(self):
        if hasattr(self._parent, "_check_for_updates"):
            self._set_update_status("正在检查更新...", checking=True)
            self._parent._check_for_updates(parent=self, status_callback=self._set_update_status)

    def _set_update_status(self, message, checking=False):
        try:
            if not self.winfo_exists():
                return
            self.update_status_label.configure(text=message or "")
            self.update_button.configure(
                text="正在检查..." if checking else "检查更新",
                state="disabled" if checking else "normal"
            )
        except Exception:
            return

    def _save(self):
        self.settings["hover_delay_ms"] = int(self.slider.get() * 1000)
        self.settings["scroll_speed"] = int(self.speed_slider.get())
        self.settings["app_font_size_delta"] = int(self.font_size_slider.get())
        self.settings["auto_archive_after_import"] = self.archive_var.get()
        self.settings["sort_order"] = SORT_VALUES.get(self.sort_var.get(), "newest")
        self.settings["launch_on_startup"] = self.startup_var.get()
        self.settings["show_hotkey"] = _normalize_hotkey(self.show_hotkey_var.get())
        self.settings["capture_hotkey"] = _normalize_hotkey(self.capture_hotkey_var.get())
        save_settings(self.settings)
        _set_launch_on_startup(self.settings["launch_on_startup"])
        if self.on_save:
            self.on_save()
        self._close()

    def _close(self):
        self._stop_hotkey_capture()
        self.grab_release()
        self.destroy()

    def _place_centered(self):
        self.withdraw()
        self.update_idletasks()
        center_window(self, self._parent)
        self.deiconify()


# ========== 更新检查对话框 ==========
class UpdateDialog(ctk.CTkToplevel):
    def __init__(self, parent, release):
        super().__init__(parent)
        latest_version = release.get("tag_name", "未知版本")
        self.title("发现新版本")
        self.geometry("380x320")
        self.resizable(False, False)
        self.transient(parent)
        self.configure(fg_color=COLORS["bg"])
        self._parent = parent
        self.release = release
        self.latest_version = latest_version
        self.asset_url = self._find_windows_asset_url(release)

        frame = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=12)
        frame.pack(fill="both", expand=True, padx=16, pady=16)

        ctk.CTkLabel(
            frame, text=f"发现新版本 {latest_version}",
            font=_font(16, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=16, pady=(16, 6), anchor="w")

        self._is_onefile = _is_onefile_bundle()
        if self._is_onefile:
            hint = f"当前版本 {APP_VERSION}，点击一键安装会下载新版到应用所在目录。"
            install_text = "一键安装"
            install_cmd = self._install_update
        else:
            hint = f"当前版本 {APP_VERSION}。安装版请前往下载页面获取最新安装包。"
            install_text = "去下载页面"
            install_cmd = self._open_release_page

        ctk.CTkLabel(
            frame, text=hint,
            font=_font(12), text_color=COLORS["text_secondary"],
            wraplength=320, justify="left"
        ).pack(padx=16, anchor="w")

        notes = (release.get("body") or "该版本没有填写更新说明。").strip()
        textbox = ctk.CTkTextbox(
            frame, height=80, wrap="word", font=_font(11),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"],
            border_width=0, corner_radius=8
        )
        textbox.pack(fill="x", padx=16, pady=(12, 0))
        textbox.insert("1.0", notes[:800])
        textbox.configure(state="disabled")

        self.status_label = ctk.CTkLabel(
            frame, text="",
            font=_font(11), text_color=COLORS["text_hint"]
        )
        self.status_label.pack(fill="x", padx=16, pady=(8, 0), anchor="w")

        btn_frame = ctk.CTkFrame(frame, fg_color="transparent")
        btn_frame.pack(fill="x", padx=16, pady=(8, 12))
        ctk.CTkButton(
            btn_frame, text="稍后", width=72, height=32,
            font=_font(12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._close
        ).pack(side="right", padx=(6, 0))
        self.install_btn = ctk.CTkButton(
            btn_frame, text=install_text, width=108, height=32,
            font=_font(12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=install_cmd
        )
        self.install_btn.pack(side="right")

        self.protocol("WM_DELETE_WINDOW", self._close)
        self._place_centered()
        self.grab_set()

    def _find_windows_asset_url(self, release):
        for asset in release.get("assets", []):
            name = asset.get("name", "")
            if name.lower().endswith(".exe"):
                return asset.get("browser_download_url")
        return None

    def _install_update(self):
        if not self.asset_url:
            self.status_label.configure(text="未找到可下载的 exe 附件")
            return
        self.install_btn.configure(state="disabled", text="下载中...")
        self.status_label.configure(text="正在下载新版...")

        def worker():
            try:
                target_name = f"ClipStash-{self.latest_version}.exe"
                target_path = os.path.join(_app_dir(), target_name)
                request = urllib.request.Request(
                    self.asset_url,
                    headers={"User-Agent": f"ClipStash/{APP_VERSION}"},
                )
                with urllib.request.urlopen(request, timeout=60) as response:
                    data = response.read()
                with open(target_path, "wb") as f:
                    f.write(data)
                self.after(0, lambda: self._finish_install(target_path))
            except Exception as e:
                message = f"下载失败: {e}"
                self.after(0, lambda: self._install_failed(message))

        threading.Thread(target=worker, daemon=True).start()

    def _finish_install(self, target_path):
        self.status_label.configure(text=f"已下载到 {target_path}")
        try:
            subprocess.Popen([target_path], cwd=os.path.dirname(target_path))
            self._close()
            app = self._get_app()
            if app:
                app._quit_app()
        except Exception as e:
            self._install_failed(f"启动新版失败: {e}")

    def _get_app(self):
        parent = self._parent
        while parent is not None:
            if hasattr(parent, "_quit_app"):
                return parent
            parent = getattr(parent, "_parent", None)
        return None

    def _install_failed(self, message):
        self.status_label.configure(text=message)
        self.install_btn.configure(state="normal", text="一键安装")

    def _open_release_page(self):
        """安装版/onedir：打开浏览器到 GitHub Release 页面，让用户手动下载安装包。"""
        import webbrowser
        url = self.release.get("html_url", f"https://github.com/{APP_REPOSITORY}/releases")
        try:
            webbrowser.open(url)
            self.status_label.configure(text="已在浏览器中打开下载页面")
        except Exception as e:
            self.status_label.configure(text=f"无法打开浏览器: {e}")

    def _close(self):
        self.grab_release()
        self.destroy()

    def _place_centered(self):
        self.withdraw()
        self.update_idletasks()
        center_window(self, self._parent)
        self.deiconify()


# ========== 新建/编辑消息对话框 ==========
class MessageEditorDialog(ctk.CTkToplevel, HoverPreviewMixin):
    """通用消息编辑对话框：新建或编辑消息"""

    def __init__(self, parent, on_save, on_close=None, title="新建消息", text_content="", images=None):
        HoverPreviewMixin.__init__(self)
        super().__init__(parent)
        self.on_save = on_save
        self._on_close_cb = on_close
        self.title(title)
        self._parent = parent
        self.geometry("500x520")
        self.minsize(420, 400)
        self.transient(parent)
        self.grab_set()
        self.configure(fg_color=COLORS["bg"])
        self.protocol("WM_DELETE_WINDOW", self._on_close)
        self._saving = False

        self.images = []
        if images:
            for img_bytes in images:
                try:
                    pil = Image.open(BytesIO(img_bytes))
                    self.images.append((pil, img_bytes))
                except Exception:
                    pass

        main = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=12)
        main.pack(fill="both", expand=True, padx=16, pady=16)

        ctk.CTkLabel(
            main, text=title,
            font=_font(16, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=16, pady=(16, 8), anchor="w")

        # 图片区域（上方）
        self.img_frame = ctk.CTkFrame(main, fg_color="transparent")
        self.img_frame.pack(fill="x", padx=12, pady=(0, 8))
        if not self.images:
            self.img_frame.pack_forget()

        # 文字输入（下方）
        self.textbox = ctk.CTkTextbox(
            main, height=150, wrap="word",
            font=_font(13),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"],
            corner_radius=8
        )
        self.textbox.pack(fill="both", expand=True, padx=12, pady=(0, 8))
        if text_content:
            self.textbox.insert("1.0", text_content)

        # 底部工具栏
        toolbar = ctk.CTkFrame(main, fg_color="transparent")
        toolbar.pack(fill="x", padx=12, pady=(0, 12))

        self.hint_label = ctk.CTkLabel(
            toolbar, text="Ctrl+V 或 Shift+Insert 粘贴图片",
            font=_font(11), text_color=COLORS["text_hint"]
        )
        self.hint_label.pack(side="left")

        ctk.CTkButton(
            toolbar, text="取消", width=68, height=34,
            font=_font(12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._on_close
        ).pack(side="right", padx=(6, 0))
        ctk.CTkButton(
            toolbar, text="保存", width=68, height=34,
            font=_font(12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._save
        ).pack(side="right")

        self.bind("<Control-v>", self._on_paste)
        self.bind("<Control-V>", self._on_paste)
        self.bind("<Shift-Insert>", self._on_paste)
        self.bind("<Control-s>", self._on_save_shortcut)
        self.bind("<Control-S>", self._on_save_shortcut)
        self.textbox.bind("<Control-v>", self._on_paste)
        self.textbox.bind("<Control-V>", self._on_paste)
        self.textbox.bind("<Shift-Insert>", self._on_paste)
        self.textbox.bind("<Control-s>", self._on_save_shortcut)
        self.textbox.bind("<Control-S>", self._on_save_shortcut)

        if self.images:
            self._render_thumbnails()
        self.after(100, lambda: self.textbox.focus_set())
        self._place_centered()

    def _on_close(self):
        self._hide_preview()
        try:
            self.grab_release()
        except Exception:
            pass
        if self._on_close_cb:
            try:
                self._on_close_cb()
            except Exception:
                pass
        self.destroy()

    def _place_centered(self):
        """隐藏→计算位置→显示，彻底避免闪烁"""
        self.withdraw()
        self.update_idletasks()
        center_window(self, self._parent)
        self.deiconify()

    def _on_paste(self, event=None):
        img, diagnostics = get_clipboard_image()
        if img is None:
            if get_clipboard_text():
                return None
            DiagnoseDialog(self, diagnostics)
            return "break"
        buf = BytesIO()
        img.save(buf, format="PNG")
        self.images.append((img, buf.getvalue()))
        self._render_thumbnails()
        return "break"

    def _render_thumbnails(self):
        self.img_frame.pack(fill="x", padx=12, pady=(0, 8))
        # 彻底清理旧组件，避免白图残留
        for w in list(self.img_frame.winfo_children()):
            w.pack_forget()
            for child in w.winfo_children():
                child.destroy()
            w.destroy()
        self.img_frame.update_idletasks()

        header = ctk.CTkFrame(self.img_frame, fg_color="transparent")
        header.pack(fill="x", pady=(0, 6))
        ctk.CTkLabel(
            header, text=f"已添加 {len(self.images)} 张图片",
            font=_font(12, weight="bold"), text_color=COLORS["text"]
        ).pack(side="left")
        ctk.CTkLabel(
            header, text="Ctrl+V 继续添加",
            font=_font(11), text_color=COLORS["text_hint"]
        ).pack(side="right")

        container = ctk.CTkFrame(self.img_frame, fg_color=COLORS["tag_bg"], corner_radius=8)
        container.pack(fill="x")

        for idx, (pil_img, img_bytes) in enumerate(self.images):
            # 图片+删除按钮的整体容器（带边框）
            item = ctk.CTkFrame(
                container,
                fg_color=COLORS["tag_bg"],
                corner_radius=8,
                border_width=1,
                border_color=COLORS["border"],
                width=90, height=90
            )
            item.pack(side="left", padx=6, pady=6)
            item.pack_propagate(False)

            # 图片标签
            ctk_img = _pil_to_ctk(pil_img, 70, 70)
            lbl = ctk.CTkLabel(item, image=ctk_img, text="")
            lbl.place(relx=0.5, rely=0.5, anchor="center")
            lbl.image = ctk_img

            # 悬浮预览
            import tempfile
            with tempfile.NamedTemporaryFile(suffix=".png", delete=False) as f:
                f.write(img_bytes)
                temp_path = f.name
            self.bind_hover_preview(lbl, temp_path)

            # 删除按钮 —— 黑色正圆，放在右上角内侧
            del_bg = ctk.CTkFrame(
                item, width=18, height=18,
                fg_color="#2D2D2D",
                corner_radius=9,
                cursor="hand2"
            )
            del_bg.place(relx=1.0, rely=0.0, anchor="ne", x=-4, y=4)
            del_bg.pack_propagate(False)

            del_lbl = ctk.CTkLabel(
                del_bg, text="×",
                font=_font(11, weight="bold"),
                text_color="white", width=18, height=18
            )
            del_lbl.place(relx=0.5, rely=0.5, anchor="center")

            # 绑定点击和悬停变色
            def _on_del_enter(e, bg=del_bg):
                bg.configure(fg_color=COLORS["danger"])
            def _on_del_leave(e, bg=del_bg):
                bg.configure(fg_color="#2D2D2D")
            def _on_del_click(e, i=idx):
                self._remove_image(i)

            for w in (del_bg, del_lbl):
                w.bind("<Button-1>", _on_del_click)
                w.bind("<Enter>", _on_del_enter)
                w.bind("<Leave>", _on_del_leave)

        self.hint_label.configure(
            text=f"已添加 {len(self.images)} 张图片" if self.images else "Ctrl+V 粘贴图片"
        )

    def _remove_image(self, index):
        if 0 <= index < len(self.images):
            self.images.pop(index)
            if self.images:
                self._render_thumbnails()
            else:
                self.img_frame.pack_forget()
                self.hint_label.configure(text="Ctrl+V 粘贴图片")

    def _save(self):
        """保存并关闭，延迟执行以避免鼠标释放事件传播到新渲染的按钮上"""
        if self._saving:
            return "break"
        self._saving = True
        text = self.textbox.get("1.0", "end-1c").strip()
        images_data = [data for _, data in self.images]
        self._saved_text = text if text else None
        self._saved_images = images_data
        # 先禁用按钮防止重复点击
        self.after(50, self._do_save_and_close)
        return "break"

    def _on_save_shortcut(self, event=None):
        return self._save()

    def _do_save_and_close(self):
        """实际执行保存和关闭"""
        on_save = self.on_save
        saved_text = self._saved_text
        saved_images = self._saved_images
        parent = self._parent
        self._on_close()
        parent.after_idle(lambda: on_save(saved_text, saved_images))


# ========== 消息卡片 ==========
class MessageCard(ctk.CTkFrame, HoverPreviewMixin):
    CONTENT_PAD_X = 14
    CONTENT_TOP_PAD = 10
    CONTENT_GAP_Y = 10

    def __init__(self, parent, item, view_mode, callbacks):
        HoverPreviewMixin.__init__(self)
        msg_id, text_content, image_filenames, created_at = item
        self.msg_id = msg_id
        self.text_content = text_content
        self.image_filenames = image_filenames
        self.created_at = created_at
        self.callbacks = callbacks

        super().__init__(
            parent, fg_color=COLORS["card"],
            corner_radius=10, border_width=1, border_color=COLORS["border"]
        )
        self.pack(fill="x", pady=6, padx=4)

        rendered_images = self._render_images() if image_filenames else False
        if text_content:
            self._render_text()
        elif not rendered_images:
            ctk.CTkLabel(
                self, text="（空消息）",
                font=_font(12), text_color=COLORS["text_hint"]
            ).pack(
                padx=self.CONTENT_PAD_X,
                pady=(self.CONTENT_TOP_PAD, 4),
                anchor="w"
            )

        self._render_footer(view_mode)

    def _render_images(self):
        """网格布局：一行三个，最多三行"""
        max_per_row = 3
        max_rows = 3
        max_display = max_per_row * max_rows
        renderable_images = []
        for img_file in self.image_filenames[:max_display]:
            image_path = db.get_image_path(img_file)
            if not image_path or not os.path.exists(image_path):
                continue
            ctk_img = _cached_ctk_image(image_path, 120, 100)
            if ctk_img:
                renderable_images.append((image_path, ctk_img))
        if not renderable_images:
            return False

        img_container = ctk.CTkFrame(self, fg_color="transparent")
        img_container.pack(
            anchor="w",
            padx=self.CONTENT_PAD_X,
            pady=(self.CONTENT_TOP_PAD, 0)
        )
        rows = [
            renderable_images[i:i + max_per_row]
            for i in range(0, len(renderable_images), max_per_row)
        ]

        for row_idx, row_items in enumerate(rows):
            row_frame = ctk.CTkFrame(img_container, fg_color="transparent")
            row_frame.pack(anchor="w", pady=(0, 6) if row_idx < len(rows) - 1 else 0)

            for col_idx, (image_path, ctk_img) in enumerate(row_items):
                frame = ctk.CTkFrame(
                    row_frame,
                    fg_color=COLORS["tag_bg"],
                    corner_radius=8,
                    border_width=1,
                    border_color=COLORS["border"],
                    width=132,
                    height=82
                )
                frame.pack(side="left", padx=(0, 6) if col_idx < len(row_items) - 1 else 0)
                frame.pack_propagate(False)

                lbl = ctk.CTkLabel(frame, image=ctk_img, text="", cursor="hand2")
                lbl.pack(padx=4, pady=4)
                lbl.image = ctk_img
                self.bind_hover_preview(lbl, image_path)
                lbl.bind(
                    "<Button-1>",
                    lambda e, p=image_path: self.callbacks["copy_image"](p)
                )

        if len(self.image_filenames) > max_display:
            ctk.CTkLabel(
                img_container,
                text=f"还有 {len(self.image_filenames) - max_display} 张图片...",
                font=_font(11), text_color=COLORS["text_hint"]
            ).grid(row=max_rows, column=0, columnspan=max_per_row, sticky="w", pady=(2, 0))
        return True

    def _render_text(self):
        text_bg = ctk.CTkFrame(self, fg_color=COLORS["tag_bg"], corner_radius=6)
        text_bg.pack(
            fill="x",
            padx=self.CONTENT_PAD_X,
            pady=(self.CONTENT_GAP_Y, 10)
        )
        preview_text = _wrap_preview_text(self.text_content)
        text_label = ctk.CTkLabel(
            text_bg, text=preview_text,
            wraplength=450, justify="left", anchor="w",
            font=_font(13), text_color=COLORS["text"],
            cursor="hand2", width=1
        )
        text_label.pack(fill="x", padx=12, pady=8, anchor="w")

        def on_click(e, txt=self.text_content):
            self.callbacks["copy_text"](txt)
        text_label.bind("<Button-1>", on_click)
        text_bg.bind("<Button-1>", on_click)

    def _render_footer(self, view_mode):
        footer = ctk.CTkFrame(self, fg_color="transparent", height=28)
        footer.pack(fill="x", padx=self.CONTENT_PAD_X, pady=(0, 8))
        footer.pack_propagate(False)

        time_str = _format_local_time(self.created_at)
        ctk.CTkLabel(
            footer, text=time_str,
            font=_font(10), text_color=COLORS["text_hint"]
        ).pack(side="left")

        archive_text = "恢复" if view_mode == "archived" else "归档"

        # 删除按钮仅在归档页面显示
        if view_mode == "archived":
            ctk.CTkButton(
                footer, text="×", width=24, height=24,
                font=_font(12, weight="bold"),
                fg_color="transparent", text_color=COLORS["text_hint"],
                hover_color=COLORS["tag_bg"], corner_radius=6,
                command=lambda: self.callbacks["delete"](self.msg_id)
            ).pack(side="right", padx=(4, 0))

        ctk.CTkButton(
            footer, text=archive_text, width=52, height=24,
            font=_font(11),
            fg_color="transparent", text_color=COLORS["text_secondary"],
            hover_color=COLORS["tab_hover"], corner_radius=6,
            command=lambda: self.callbacks["archive"](self.msg_id)
        ).pack(side="right")

        if view_mode == "active":
            ctk.CTkButton(
                footer, text="编辑", width=42, height=24,
                font=_font(11),
                fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
                hover_color=COLORS["border"], corner_radius=6,
                command=lambda: self.callbacks["edit"](self.msg_id)
            ).pack(side="right", padx=(0, 4))

            ctk.CTkButton(
                footer, text="导入", width=42, height=24,
                font=_font(11),
                fg_color=COLORS["primary"], text_color="white",
                hover_color=COLORS["primary_hover"], corner_radius=6,
                command=lambda: self.callbacks["import_message"](self.msg_id)
            ).pack(side="right", padx=(0, 4))


# ========== 主窗口 ==========
class DemandStashApp(ctk.CTk):
    def __init__(self):
        ctk.set_appearance_mode("System")
        ctk.set_default_color_theme("blue")
        super().__init__()
        self.title(f"{APP_NAME} {APP_VERSION}  @linjianglu")
        self.geometry("420x720")
        self.minsize(380, 520)

        self.configure(fg_color=COLORS["bg"])

        self.view_mode = "active"
        self._always_on_top = False
        self._tray_icon = None
        self._import_queue = []
        self._import_msg_id = None
        self._editor_dialog = None
        self._checking_update = False
        self._hotkey_listener = None
        self._return_hwnd = None
        self._last_external_hwnd = None
        self._restore_after_import = False
        self._load_items_after_id = None
        self._scroll_region_after_ids = []
        self._scroll_speed = get_scroll_speed()
        self._view_frames = {}
        self._view_dirty = {"active": True, "archived": True}
        self._view_counts = {"active": 0, "archived": 0}
        self._view_build_tokens = {"active": 0, "archived": 0}
        self._render_batch_after_ids = []
        self._view_items = {"active": [], "archived": []}
        self._view_callbacks = {}
        self._view_rendered_count = {"active": 0, "archived": 0}
        self._view_render_complete = {"active": False, "archived": False}

        self.bind("<Control-v>", self._on_paste)
        self.bind("<Control-V>", self._on_paste)
        self.bind("<Shift-Insert>", self._on_paste)

        self._create_header()
        self._create_content()
        self._create_footer()

        self.protocol("WM_DELETE_WINDOW", self._hide_to_tray)
        self.after_idle(self._do_show)
        self.after(120, lambda: self.load_items(immediate=True))
        self.after(700, self._apply_icon)
        self.after(1500, self._track_foreground_window)
        self.after(2000, lambda: _set_launch_on_startup(get_launch_on_startup()))
        self.after(2500, self._setup_background_features)

    def _apply_icon(self):
        """延后应用图标，避免图标提取和 Win32 图标设置阻塞首屏。"""
        try:
            icon_path = _ensure_app_icon()
            self.iconbitmap(icon_path)
        except Exception:
            return
        self.after(500, lambda: self._apply_taskbar_icon(icon_path))

    def _apply_taskbar_icon(self, icon_path):
        try:
            hwnd = self.winfo_id()
            if hwnd:
                _set_taskbar_icon(hwnd, icon_path)
        except Exception:
            pass

    # ---------- 托盘 & 快捷键 ----------
    def _setup_background_features(self):
        if _load_tray_modules():
            self._setup_tray()
            self._start_hotkey_listener()

    def _create_tray_image(self):
        # 优先从当前 exe 自身提取图标，避免 onefile 运行时 assets 路径问题
        try:
            import win32gui
            large, small = win32gui.ExtractIconEx(sys.executable, 0)
            if large:
                img = _hicon_to_pil(large[0], 64)
                # 释放多余的句柄
                for h in large[1:]:
                    win32gui.DestroyIcon(h)
                if small:
                    for h in small:
                        win32gui.DestroyIcon(h)
                if img:
                    return img
        except Exception:
            pass
        return _load_app_icon_image(256)

    def _setup_tray(self):
        log_lines = ["_setup_tray called"]
        try:
            # Monkey-patch pystray to avoid PNG-in-ICO serialization issue
            # pystray internally saves PIL Image as ICO using Pillow,
            # which produces PNG-in-ICO that Windows LoadImage cannot load.
            # We patch _assert_icon_handle to extract HICON directly from exe.
            import pystray._win32 as pw
            _orig_assert = pw.Icon._assert_icon_handle

            def _patched_assert_icon_handle(self2):
                if self2._icon_handle:
                    return
                try:
                    # 使用 ctypes 调用 ExtractIconExW，获取纯整数句柄，
                    # 避免 pywin32 的 PyGdiHANDLE 与 ctypes 结构体不兼容的问题
                    shell32 = ctypes.windll.shell32
                    large_arr = (ctypes.c_void_p * 1)()
                    small_arr = (ctypes.c_void_p * 1)()
                    count = shell32.ExtractIconExW(sys.executable, 0, large_arr, small_arr, 1)
                    log_lines.append(f"ExtractIconExW: count={count}, large=0x{large_arr[0]:x}, small=0x{small_arr[0]:x}")
                    if count > 0 and large_arr[0]:
                        self2._icon_handle = large_arr[0]
                        log_lines.append("Set _icon_handle from ExtractIconExW")
                        return
                except Exception as e:
                    log_lines.append(f"Patched assert error: {e}")
                # Fallback to original (will likely fail due to PNG-in-ICO)
                _orig_assert(self2)

            pw.Icon._assert_icon_handle = _patched_assert_icon_handle

            image = self._create_tray_image()
            log_lines.append(f"_create_tray_image: {image}, size={image.size if image else 'None'}")
            menu = pystray.Menu(
                pystray.MenuItem("显示", self._show_from_tray, default=True),
                pystray.MenuItem("检查更新", lambda icon, item: self._check_for_updates()),
                pystray.MenuItem("退出", self._quit_from_tray),
            )
            self._tray_icon = pystray.Icon(APP_NAME, image, APP_NAME, menu=menu)
            self._tray_icon.run_detached()
            log_lines.append("run_detached called")
        except Exception as e:
            import traceback
            log_lines.append(f"Tray setup error: {e}")
            log_lines.append(traceback.format_exc())
        finally:
            log_path = os.path.join(
                os.environ.get("TEMP", "C:\\temp"), "clipstash_tray.log"
            )
            try:
                with open(log_path, "w", encoding="utf-8") as f:
                    f.write("\n".join(log_lines))
            except Exception:
                pass

    def _start_hotkey_listener(self):
        if not _load_tray_modules():
            return
        if self._hotkey_listener:
            try:
                self._hotkey_listener.stop()
            except Exception:
                pass
            self._hotkey_listener = None

        show_hotkey = _normalize_hotkey(get_show_hotkey())
        capture_hotkey = _normalize_hotkey(get_capture_hotkey())
        hotkeys = {}
        if show_hotkey:
            hotkeys[show_hotkey] = lambda: self.after(0, self._toggle_show_hide)
        if capture_hotkey and capture_hotkey != show_hotkey:
            hotkeys[capture_hotkey] = lambda: self.after(0, self._capture_current_clipboard)
        if not hotkeys:
            self._show_status("未启用全局快捷键")
            return

        def listen():
            try:
                with keyboard.GlobalHotKeys(hotkeys) as h:
                    self._hotkey_listener = h
                    h.join()
            except Exception as e:
                message = f"快捷键注册失败: {e}"
                self.after(0, lambda: self._show_status(message))

        threading.Thread(target=listen, daemon=True).start()

    def _show_from_tray(self, icon=None, item=None):
        hwnd = _get_foreground_hwnd()
        if hwnd and not _is_own_window(hwnd):
            self._return_hwnd = hwnd
            self._last_external_hwnd = hwnd
        self.after(0, self._do_show)

    def _do_show(self):
        self.deiconify()
        self.lift()
        self.focus_force()

    def _track_foreground_window(self):
        hwnd = _get_foreground_hwnd()
        try:
            if hwnd and not _is_own_window(hwnd) and ctypes.windll.user32.IsWindow(hwnd):
                self._return_hwnd = hwnd
                self._last_external_hwnd = hwnd
        except Exception:
            pass
        self.after(500, self._track_foreground_window)

    def _hide_to_tray(self):
        self.withdraw()

    def _toggle_show_hide(self):
        if self.state() == "withdrawn":
            hwnd = _get_foreground_hwnd()
            if hwnd and not _is_own_window(hwnd):
                self._return_hwnd = hwnd
                self._last_external_hwnd = hwnd
            self._do_show()
        else:
            self._hide_to_tray()

    def _quit_from_tray(self, icon=None, item=None):
        self.after(0, self._quit_app)

    def _quit_app(self):
        # 1. 停止全局热键监听（pynput）
        if self._hotkey_listener:
            try:
                self._hotkey_listener.stop()
            except Exception:
                pass
            self._hotkey_listener = None

        # 2. 停止托盘图标（pystray）
        if self._tray_icon:
            try:
                self._tray_icon.stop()
            except Exception:
                pass
            self._tray_icon = None

        # 3. 销毁主窗口并退出事件循环
        try:
            self.destroy()
        except Exception:
            pass

        # 4. 强制退出进程（兜底）
        self.after(200, lambda: os._exit(0))

    # ---------- UI 组件 ----------
    def _create_header(self):
        self.header = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=0, height=56)
        self.header.pack(fill="x")
        self.header.pack_propagate(False)
        self.header.bind("<Configure>", self._adjust_header_layout)

        title_frame = ctk.CTkFrame(self.header, fg_color="transparent")
        title_frame.pack(side="left", padx=(14, 8), pady=10)
        self.title_label = ctk.CTkLabel(
            title_frame, text=APP_NAME,
            font=_font(17, weight="bold"),
            text_color=COLORS["primary"]
        )
        self.title_label.pack(side="left")

        btn_frame = ctk.CTkFrame(self.header, fg_color="transparent")
        btn_frame.pack(side="right", padx=(6, 10), pady=10)

        self.count_label = ctk.CTkLabel(
            btn_frame, text="总计 0 条消息",
            font=_font(11), text_color=COLORS["text_hint"],
            width=78
        )
        self.count_label.pack(side="left", padx=(0, 6))

        self.pin_btn = ctk.CTkButton(
            btn_frame, text="置顶", width=40, height=30,
            font=_font(11),
            fg_color="transparent", text_color=COLORS["text_hint"],
            hover_color=COLORS["tab_hover"], corner_radius=6,
            command=self._toggle_always_on_top
        )
        self.pin_btn.pack(side="left", padx=(0, 2))

        self.settings_btn = ctk.CTkButton(
            btn_frame, text="设置", width=40, height=30,
            font=_font(11),
            fg_color="transparent", text_color=COLORS["text_hint"],
            hover_color=COLORS["tab_hover"], corner_radius=6,
            command=self._open_settings
        )
        self.settings_btn.pack(side="left", padx=(0, 4))

        self.new_msg_btn = ctk.CTkButton(
            btn_frame, text="+ 新建", width=82, height=32,
            font=_font(12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._on_new_message,
        )
        self.new_msg_btn.pack(side="left")

    def _adjust_header_layout(self, event=None):
        if not hasattr(self, "count_label"):
            return
        width = event.width if event else self.winfo_width()
        self._refresh_count_label(width)
        if event is None:
            return
        if width < 405:
            self.title_label.configure(font=_font(15, weight="bold"))
            self.new_msg_btn.configure(width=76)
        else:
            self.title_label.configure(font=_font(17, weight="bold"))
            self.new_msg_btn.configure(width=82)

    def _refresh_count_label(self, width=None):
        if not hasattr(self, "count_label"):
            return
        width = width if width is not None else self.winfo_width()
        count = self._view_counts.get(self.view_mode, 0)
        if width < 405:
            self.count_label.configure(width=42, text=f"{count} 条")
        else:
            self.count_label.configure(width=78, text=f"总计 {count} 条消息")

    def _create_content(self):
        content_frame = ctk.CTkFrame(self, fg_color="transparent")
        content_frame.pack(fill="both", expand=True, padx=12, pady=12)

        self.tab_frame = ctk.CTkFrame(content_frame, fg_color=COLORS["card"], corner_radius=10)
        self.tab_frame.pack(fill="x", pady=(0, 8))

        self.tab_active = ctk.CTkButton(
            self.tab_frame, text="消息", width=80, height=32,
            font=_font(12, weight="bold"),
            fg_color=COLORS["primary"], text_color="white",
            hover_color=COLORS["primary_hover"], corner_radius=8,
            command=lambda: self._switch_view("active")
        )
        self.tab_active.pack(side="left", padx=8, pady=8)

        self.tab_archived = ctk.CTkButton(
            self.tab_frame, text="已归档", width=80, height=32,
            font=_font(12),
            fg_color="transparent", text_color=COLORS["text_secondary"],
            hover_color=COLORS["tab_hover"], corner_radius=8,
            command=lambda: self._switch_view("archived")
        )
        self.tab_archived.pack(side="left", padx=(0, 8), pady=8)

        self.scroll_frame = ctk.CTkScrollableFrame(
            content_frame, fg_color="transparent",
            scrollbar_fg_color=COLORS["bg"],          # 轨道与背景融合，去掉方形阴影
            scrollbar_button_color=COLORS["primary"], # thumb 用主题色
            scrollbar_button_hover_color=COLORS["primary_hover"]
        )
        self.scroll_frame.pack(fill="both", expand=True)
        self.bind_all("<MouseWheel>", self._on_mouse_wheel)
        self._bind_blank_new_message(self.scroll_frame)
        try:
            self._bind_blank_new_message(self.scroll_frame._parent_canvas)
        except Exception:
            pass

        # 应用滚动速度
        self._apply_scroll_speed(get_scroll_speed())

    def _bind_blank_new_message(self, widget):
        widget.bind("<Double-Button-1>", self._on_blank_double_click, add="+")

    def _on_blank_double_click(self, event=None):
        self._on_new_message()
        return "break"

    def _apply_scroll_speed(self, speed):
        """保存滚动速度。不要改 yscrollincrement，否则滚轮会像失效一样变慢。"""
        try:
            self._scroll_speed = max(1, min(5, int(speed)))
            self.scroll_frame._parent_canvas.configure(yscrollincrement=0)
        except Exception:
            self._scroll_speed = 2

    def _on_mouse_wheel(self, event):
        """接管滚轮事件，让滚动速度设置真正生效。"""
        try:
            if not self.scroll_frame.check_if_master_is_canvas(event.widget):
                return
            canvas = self.scroll_frame._parent_canvas
            yview = canvas.yview()
            if yview == (0.0, 1.0):
                return "break"
            speed = max(1, min(5, int(getattr(self, "_scroll_speed", 2))))
            steps = -int(event.delta / 120) * speed * 3
            if steps:
                canvas.yview("scroll", steps, "units")
            return "break"
        except Exception:
            return "break"

    def _create_footer(self):
        self.footer = ctk.CTkFrame(self, fg_color=COLORS["card"], height=36)
        self.footer.pack(fill="x", side="bottom")
        self.footer.pack_propagate(False)
        self.status_bar = ctk.CTkLabel(
            self.footer, text="",
            font=_font(11), text_color=COLORS["text_secondary"]
        )
        self.status_bar.pack(side="left", padx=16)
        self.hotkey_hint_label = ctk.CTkLabel(
            self.footer, text="Ctrl+Shift+V 呼出",
            font=_font(11), text_color=COLORS["text_hint"]
        )
        self.hotkey_hint_label.pack(side="right", padx=16)
        self._refresh_hotkey_hint()

    def _toggle_always_on_top(self):
        self._always_on_top = not self._always_on_top
        self.attributes("-topmost", self._always_on_top)
        if self._always_on_top:
            self.pin_btn.configure(text_color=COLORS["primary"])
            self._show_status("窗口已置顶")
        else:
            self.pin_btn.configure(text_color=COLORS["text_hint"])
            self._show_status("已取消置顶")

    def _open_settings(self):
        SettingsDialog(self, on_save=self._on_settings_saved)

    def _on_settings_saved(self):
        self._refresh_main_fonts()
        self._mark_views_dirty()
        self.load_items(immediate=True)
        self._start_hotkey_listener()
        self._refresh_hotkey_hint()
        self._apply_scroll_speed(get_scroll_speed())
        self._show_status("设置已保存")

    def _refresh_main_fonts(self):
        if hasattr(self, "count_label"):
            self.count_label.configure(font=_font(11))
        if hasattr(self, "pin_btn"):
            self.pin_btn.configure(font=_font(11))
        if hasattr(self, "settings_btn"):
            self.settings_btn.configure(font=_font(11))
        if hasattr(self, "new_msg_btn"):
            self.new_msg_btn.configure(font=_font(12, weight="bold"))
        if hasattr(self, "tab_active"):
            self.tab_active.configure(font=_font(12, weight="bold"))
        if hasattr(self, "tab_archived"):
            self.tab_archived.configure(font=_font(12))
        if hasattr(self, "status_bar"):
            self.status_bar.configure(font=_font(11))
        if hasattr(self, "hotkey_hint_label"):
            self.hotkey_hint_label.configure(font=_font(11))
        self._adjust_header_layout()

    def _refresh_hotkey_hint(self):
        if hasattr(self, "hotkey_hint_label"):
            self.hotkey_hint_label.configure(text=f"{get_show_hotkey()} 呼出")

    def _check_for_updates(self, icon=None, item=None, parent=None, status_callback=None):
        if self._checking_update:
            self.after(0, lambda: self._show_status("正在检查更新..."))
            if status_callback:
                self.after(0, lambda: status_callback("正在检查更新...", checking=True))
            return
        self._checking_update = True
        self.after(0, lambda: self._show_status("正在检查更新..."))
        if status_callback:
            self.after(0, lambda: status_callback("正在检查更新...", checking=True))

        def worker():
            try:
                release = _fetch_latest_release()
                latest_version = release.get("tag_name", "")
                has_update = _parse_version(latest_version) > _parse_version(APP_VERSION)
                self.after(
                    0,
                    lambda: self._finish_update_check(
                        release, has_update, parent=parent, status_callback=status_callback
                    )
                )
            except urllib.error.HTTPError as e:
                message = "未找到 GitHub Release" if e.code == 404 else f"检查更新失败: HTTP {e.code}"
                self.after(
                    0,
                    lambda: self._finish_update_check(
                        None, False, message, parent=parent, status_callback=status_callback
                    )
                )
            except Exception as e:
                message = f"检查更新失败: {e}"
                self.after(
                    0,
                    lambda: self._finish_update_check(
                        None, False, message, parent=parent, status_callback=status_callback
                    )
                )

        threading.Thread(target=worker, daemon=True).start()

    def _finish_update_check(self, release, has_update, message=None, parent=None, status_callback=None):
        self._checking_update = False
        if message:
            self._show_status(message)
            if status_callback:
                status_callback(message, checking=False)
            return
        if has_update:
            latest_version = release.get("tag_name", "")
            message = f"发现新版本 {latest_version}"
            self._show_status(message)
            if status_callback:
                status_callback(message, checking=False)
            UpdateDialog(parent or self, release)
        else:
            latest_version = release.get("tag_name", APP_VERSION) if release else APP_VERSION
            message = f"已是最新版本 {latest_version}"
            self._show_status(message)
            if status_callback:
                status_callback(message, checking=False)

    def _switch_view(self, mode):
        self.view_mode = mode
        if mode == "active":
            self.tab_active.configure(
                fg_color=COLORS["primary"], text_color="white",
                hover_color=COLORS["primary_hover"]
            )
            self.tab_archived.configure(
                fg_color="transparent", text_color=COLORS["text_secondary"],
                hover_color=COLORS["tab_hover"]
            )
        else:
            self.tab_active.configure(
                fg_color="transparent", text_color=COLORS["text_secondary"],
                hover_color=COLORS["tab_hover"]
            )
            self.tab_archived.configure(
                fg_color=COLORS["primary"], text_color="white",
                hover_color=COLORS["primary_hover"]
            )
        self._refresh_count_label()
        self.load_items(immediate=True)

    def _open_editor(self, on_save, title="新建消息", text_content="", images=None):
        """统一管理编辑器窗口：确保只有一个，且关闭后清空引用"""
        if self._editor_dialog and self._editor_dialog.winfo_exists():
            self._editor_dialog.lift()
            self._editor_dialog.focus_force()
            return self._editor_dialog

        def _on_close():
            self._editor_dialog = None

        dialog = MessageEditorDialog(
            self, on_save, on_close=_on_close,
            title=title, text_content=text_content, images=images
        )
        self._editor_dialog = dialog
        return dialog

    def _on_new_message(self):
        def on_save(text, images_data):
            db.add_message(text_content=text, images_data=images_data if images_data else None)
            self._reset_view_cache("active")
            if self.view_mode != "active":
                self._switch_view("active")
            else:
                self.load_items(immediate=True)
            self._show_status("已保存")
        self._open_editor(on_save)

    def _on_paste(self, event=None):
        img, diagnostics = get_clipboard_image()
        if img is None:
            text = get_clipboard_text()
            if text:
                db.add_message(text_content=text)
                self._reset_view_cache("active")
                if self.view_mode != "active":
                    self._switch_view("active")
                else:
                    self.load_items(immediate=True)
                self._show_status("已保存文字")
                return "break"
            DiagnoseDialog(self, diagnostics)
            return

        def on_save(text, images_data):
            db.add_message(text_content=text, images_data=images_data)
            self._reset_view_cache("active")
            if self.view_mode != "active":
                self._switch_view("active")
            else:
                self.load_items(immediate=True)
            self._show_status("已保存")

        dialog = self._open_editor(on_save)
        buf = BytesIO()
        img.save(buf, format="PNG")
        dialog.images.append((img, buf.getvalue()))
        dialog._render_thumbnails()

    def _capture_current_clipboard(self):
        img, diagnostics = get_clipboard_image()
        if img is not None:
            buf = BytesIO()
            img.save(buf, format="PNG")
            db.add_message(images_data=[buf.getvalue()])
            self._reset_view_cache("active")
            if self.view_mode != "active":
                self._switch_view("active")
            else:
                self.load_items(immediate=True)
            self._show_status("已导入剪切板图片")
            return

        text = get_clipboard_text()
        if text:
            db.add_message(text_content=text)
            self._reset_view_cache("active")
            if self.view_mode != "active":
                self._switch_view("active")
            else:
                self.load_items(immediate=True)
            self._show_status("已导入剪切板文字")
            return

        self._show_status("剪切板没有可导入内容")

    def _on_edit_message(self, msg_id: int):
        item = db.get_message(msg_id)
        if not item:
            self._show_status("消息不存在")
            return
        _, text_content, image_filenames, _ = item

        images_data = []
        for img_file in image_filenames:
            path = db.get_image_path(img_file)
            if path and os.path.exists(path):
                with open(path, "rb") as f:
                    images_data.append(f.read())

        def on_save(text, new_images_data):
            db.update_message_text(msg_id, text)
            # 编辑时总是替换图片（空列表表示清空所有图片）
            db.delete_message_images(msg_id)
            for img_data in new_images_data:
                db.add_image_to_message(msg_id, img_data)
            self._reset_view_cache(self.view_mode)
            self.load_items(immediate=True)
            self._show_status("已更新")

        self._open_editor(
            on_save,
            title="编辑消息",
            text_content=text_content or "",
            images=images_data
        )

    def _on_import_message(self, msg_id: int):
        item = db.get_message(msg_id)
        if not item:
            self._show_status("消息不存在")
            return
        _, text_content, image_filenames, _ = item

        self._import_queue = []
        if text_content:
            self._import_queue.append(("text", text_content))
        for img_file in image_filenames:
            path = db.get_image_path(img_file)
            if path and os.path.exists(path):
                self._import_queue.append(("image", path))

        if not self._import_queue:
            self._show_status("消息为空")
            return

        self._import_msg_id = msg_id
        if not self._focus_return_window():
            self._show_status("未找到外部输入窗口，已取消导入")
            return
        self._show_status("正在导入...")
        self.after(350, self._do_import_step)

    def _focus_return_window(self):
        hwnd = self._return_hwnd or self._last_external_hwnd
        if not hwnd:
            return False
        try:
            user32 = ctypes.windll.user32
            if not user32.IsWindow(hwnd) or _is_own_window(hwnd):
                return False
            current = user32.GetForegroundWindow()
            if current and _is_own_window(current) and not self._last_external_hwnd:
                return False
            self._return_hwnd = hwnd
            self._last_external_hwnd = hwnd
            self._restore_after_import = self.state() != "withdrawn"
            self.withdraw()
            self.update_idletasks()
            user32.ShowWindow(hwnd, 9)
            user32.SetForegroundWindow(hwnd)
            return user32.GetForegroundWindow() == hwnd or bool(user32.IsWindow(hwnd))
        except Exception:
            return False

    def _do_import_step(self):
        if not self._import_queue:
            self._show_status("导入完成")
            if get_auto_archive_after_import() and self._import_msg_id:
                db.toggle_archive(self._import_msg_id)
                self._import_msg_id = None
                self._reset_view_cache("active", "archived")
                self.load_items(immediate=True)
            if self._restore_after_import:
                self._restore_after_import = False
                self.after(300, self._do_show)
            return

        item_type, data = self._import_queue.pop(0)
        if item_type == "text":
            copy_text_to_clipboard(data)
        elif item_type == "image":
            try:
                copy_image_to_clipboard(data)
            except Exception as e:
                print(f"复制图片失败: {e}")
                self.after(200, self._do_import_step)
                return
        send_ctrl_v()
        self.after(250, self._do_import_step)

    def _on_archive(self, msg_id: int):
        new_val = db.toggle_archive(msg_id)
        self.after_idle(lambda: self._reload_views("active", "archived"))
        self._show_status("已归档" if new_val else "已恢复")

    def _delete_message(self, msg_id: int):
        db.delete_message(msg_id)
        self.after_idle(lambda: self._reload_views("archived"))
        self._show_status("已删除")

    def _show_status(self, message, duration=2000):
        self.status_bar.configure(text=message)
        self.after(duration, lambda: self.status_bar.configure(text=""))

    def load_items(self, immediate=False):
        """渲染消息列表。缓存两个队列的内容 frame，切换时避免整页重建。"""
        # 取消上一次的延迟渲染
        if self._load_items_after_id:
            self.after_cancel(self._load_items_after_id)
            self._load_items_after_id = None

        if immediate:
            self._show_or_build_current_view()
        else:
            # 延迟 50ms 渲染，如果期间再次调用会取消旧的
            self._load_items_after_id = self.after(50, self._show_or_build_current_view)

    def _mark_views_dirty(self, *modes):
        if not modes:
            modes = ("active", "archived")
        for mode in modes:
            self._view_dirty[mode] = True
            self._view_build_tokens[mode] += 1
        self._cancel_render_batches()

    def _reload_views(self, *modes):
        self._reset_view_cache(*modes)
        self.load_items(immediate=True)

    def _reset_view_cache(self, *modes):
        if not modes:
            modes = ("active", "archived")
        self._cancel_scroll_region_updates()
        self._cancel_render_batches()
        if self._load_items_after_id:
            self.after_cancel(self._load_items_after_id)
            self._load_items_after_id = None
        for mode in modes:
            frame = self._view_frames.get(mode)
            if frame and frame.winfo_exists():
                for widget in list(frame.winfo_children()):
                    widget.destroy()
                frame.pack_forget()
            self._view_dirty[mode] = True
            self._view_build_tokens[mode] += 1
            self._view_items[mode] = []
            self._view_callbacks[mode] = {}
            self._view_rendered_count[mode] = 0
            self._view_render_complete[mode] = False

    def _get_view_frame(self, mode):
        frame = self._view_frames.get(mode)
        if frame is None or not frame.winfo_exists():
            frame = ctk.CTkFrame(self.scroll_frame, fg_color="transparent")
            self._bind_blank_new_message(frame)
            self._view_frames[mode] = frame
            self._view_dirty[mode] = True
        return frame

    def _show_or_build_current_view(self):
        """实际执行列表渲染"""
        self._load_items_after_id = None
        self._cancel_scroll_region_updates()
        self._cancel_render_batches()

        mode = self.view_mode
        for frame_mode, frame in list(self._view_frames.items()):
            if frame_mode != mode and frame.winfo_exists():
                frame.pack_forget()

        parent_frame = self._get_view_frame(mode)
        parent_frame.update_idletasks()
        parent_frame.pack(fill="x", expand=True)

        if not self._view_dirty.get(mode, True):
            self._refresh_count_label()
            self._schedule_scroll_region_update(reset=True)
            return

        for widget in list(parent_frame.winfo_children()):
            widget.destroy()
        parent_frame.update_idletasks()

        sort_order = get_sort_order()
        items = db.get_all_messages(
            archived=(mode == "archived"),
            sort_order=sort_order
        )
        count = len(items)
        self._view_counts[mode] = count
        self._refresh_count_label()

        if not items:
            self._render_empty_state(parent_frame, mode)
            self._view_dirty[mode] = False
            return

        callbacks = {
            "copy_image": self._copy_image,
            "copy_text": self._copy_text,
            "edit": self._on_edit_message,
            "import_message": self._on_import_message,
            "archive": self._on_archive,
            "delete": self._delete_message,
        }
        self._view_items[mode] = items
        self._view_callbacks[mode] = callbacks
        self._view_rendered_count[mode] = 0
        self._view_render_complete[mode] = False

        token = self._view_build_tokens[mode] + 1
        self._view_build_tokens[mode] = token
        self._render_message_batch(parent_frame, items, mode, callbacks, token, start=0)

    def _cancel_render_batches(self):
        for after_id in self._render_batch_after_ids:
            try:
                self.after_cancel(after_id)
            except Exception:
                pass
        self._render_batch_after_ids = []

    def _render_message_batch(self, parent_frame, items, mode, callbacks, token, start=0):
        if self._view_build_tokens.get(mode) != token:
            return

        # Render the current view in one pass. Incremental rendering while the
        # scrollbar is being dragged can leave CTk's canvas with stale item
        # heights, which visually compresses cards into thin rows.
        batch_size = len(items)
        end = min(start + batch_size, len(items))
        for item in items[start:end]:
            MessageCard(parent_frame, item, mode, callbacks)
        self._view_rendered_count[mode] = end
        parent_frame.update_idletasks()

        self._schedule_scroll_region_update(reset=(start == 0))

        self._view_dirty[mode] = False
        self._view_render_complete[mode] = end >= len(items)

    def _render_more_for_current_view(self):
        mode = self.view_mode
        if self._view_dirty.get(mode, True) or self._view_render_complete.get(mode, False):
            return
        frame = self._view_frames.get(mode)
        if not frame or not frame.winfo_exists():
            return
        items = self._view_items.get(mode) or []
        callbacks = self._view_callbacks.get(mode)
        if not callbacks:
            return
        start = self._view_rendered_count.get(mode, 0)
        if start >= len(items):
            self._view_render_complete[mode] = True
            return
        token = self._view_build_tokens[mode]
        self._render_message_batch(frame, items, mode, callbacks, token, start=start)

    def _cancel_scroll_region_updates(self):
        for after_id in self._scroll_region_after_ids:
            try:
                self.after_cancel(after_id)
            except Exception:
                pass
        self._scroll_region_after_ids = []

    def _schedule_scroll_region_update(self, reset=False):
        self._cancel_scroll_region_updates()
        self._scroll_region_after_ids = [
            self.after_idle(lambda: self._update_scroll_region(reset=reset)),
            self.after(80, lambda: self._update_scroll_region(reset=reset)),
        ]

    def _update_scroll_region(self, reset=False):
        """更新 CTkScrollableFrame 的滚动区域并重置到顶部"""
        try:
            self._scroll_region_after_ids = []
            canvas = self.scroll_frame._parent_canvas
            parent_frame = self.scroll_frame
            parent_frame.update_idletasks()
            canvas.update_idletasks()

            # 让 canvas 的窗口 item 自动适应内容高度
            canvas.configure(scrollregion=canvas.bbox("all"))
            if reset:
                canvas.yview_moveto(0)
        except Exception:
            pass

    def _render_empty_state(self, parent_frame, mode):
        empty_frame = ctk.CTkFrame(parent_frame, fg_color="transparent")
        self._bind_blank_new_message(empty_frame)
        empty_frame.pack(fill="both", expand=True, pady=80)

        if mode == "archived":
            icon, title, desc = "📦", "没有归档消息", "归档的消息会显示在这里"
        else:
            icon, title, desc = "📋", "还没有消息", "Ctrl+V 粘贴截图，点击「+ 新建」，或双击空白处创建"

        ctk.CTkLabel(empty_frame, text=icon, font=ctk.CTkFont(size=48)).pack(pady=(0, 12))
        ctk.CTkLabel(empty_frame, text=title,
                     font=_font(16, weight="bold"),
                     text_color=COLORS["text"]).pack(pady=(0, 8))
        ctk.CTkLabel(empty_frame, text=desc,
                     font=_font(13),
                     text_color=COLORS["text_hint"]).pack()

        for child in empty_frame.winfo_children():
            self._bind_blank_new_message(child)

        self._schedule_scroll_region_update(reset=True)

    def _copy_image(self, image_path):
        try:
            copy_image_to_clipboard(image_path)
            self._show_status("图片已复制到剪贴板")
        except Exception as e:
            self._show_status(f"复制失败: {e}")

    def _copy_text(self, text):
        try:
            copy_text_to_clipboard(text)
            self._show_status("文本已复制到剪贴板")
        except Exception as e:
            self._show_status(f"复制失败: {e}")


def insert_test_data():
    img = Image.new("RGB", (600, 400), color=(59, 130, 246))
    draw = ImageDraw.Draw(img)
    draw.text((20, 20), "Test Screenshot", fill=(255, 255, 255))
    buf = BytesIO()
    img.save(buf, format="PNG")
    img1 = buf.getvalue()
    img2 = Image.new("RGB", (500, 300), color=(139, 92, 246))
    buf2 = BytesIO()
    img2.save(buf2, format="PNG")
    img2_data = buf2.getvalue()
    db.add_message(
        text_content="这是一条测试消息，包含多张图片和说明文字。",
        images_data=[img1, img2_data],
    )
    db.add_message(text_content="纯文字消息示例，可以点击复制。")
    db.add_message(images_data=[img1])


if __name__ == "__main__":
    _set_windows_app_id()  # 必须在创建窗口前设置 AUMID
    if not _ensure_single_instance():
        sys.exit(0)
    try:
        db.init_db()
        if os.environ.get("CLIPSTASH_DEBUG", "0") == "1" and not db.get_all_messages():
            insert_test_data()
        app = DemandStashApp()
        app.mainloop()
    except Exception as e:
        import traceback
        log_path = os.path.join(os.environ.get("TEMP", "C:\\temp"), "clipstash_error.log")
        with open(log_path, "w", encoding="utf-8") as f:
            f.write(f"Exception: {e}\n{traceback.format_exc()}")
        raise
