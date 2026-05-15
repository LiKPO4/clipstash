import customtkinter as ctk
from PIL import Image, ImageDraw
import db
import os
import sys
import threading
import ctypes
import json
import urllib.error
import urllib.request
import webbrowser
from io import BytesIO
from clipboard_utils import (
    get_clipboard_image, get_clipboard_text, copy_text_to_clipboard,
    copy_image_to_clipboard, send_ctrl_v, diagnose_clipboard
)
from config import (
    load_settings, save_settings,
    get_hover_delay_ms, get_auto_archive_after_import, get_sort_order
)

# 托盘 & 全局快捷键
try:
    import pystray
    from pynput import keyboard
    HAS_TRAY = True
except ImportError:
    HAS_TRAY = False

APP_NAME = "需求暂存站"
APP_VERSION = "v1.0.9"
APP_REPOSITORY = "LiKPO4/clipstash"
LATEST_RELEASE_API = f"https://api.github.com/repos/{APP_REPOSITORY}/releases/latest"
WINDOWS_APP_ID = f"LiKPO4.ClipStash.{APP_VERSION.lstrip('v')}"


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


# ========== 单例锁（Windows 命名互斥量）==========
_mutex_handle = None

def _ensure_single_instance():
    """确保只有一个实例运行。如果已有实例，激活它并退出。"""
    global _mutex_handle
    kernel32 = ctypes.windll.kernel32
    kernel32.CreateMutexW.argtypes = [ctypes.c_void_p, ctypes.c_bool, ctypes.c_wchar_p]
    kernel32.CreateMutexW.restype = ctypes.c_void_p
    kernel32.GetLastError.restype = ctypes.c_uint32

    _mutex_handle = kernel32.CreateMutexW(None, False, "ClipStash_SingleInstance_Mutex")
    if kernel32.GetLastError() == 183:  # ERROR_ALREADY_EXISTS
        # 激活已有实例
        user32 = ctypes.windll.user32
        # 尝试匹配当前版本标题
        hwnd = user32.FindWindowW(None, f"{APP_NAME} {APP_VERSION}  @linjianglu")
        if not hwnd:
            # 兼容旧版本标题
            hwnd = user32.FindWindowW(None, f"ClipStash {APP_VERSION}  @linjianglu")
        if hwnd:
            user32.ShowWindow(hwnd, 9)  # SW_RESTORE
            user32.SetForegroundWindow(hwnd)
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
    "text": "#1E293B",
    "text_secondary": "#64748B",
    "text_hint": "#94A3B8",
    "border": "#E2E8F0",
    "tag_bg": "#F1F5F9",
}


# ========== 工具函数 ==========
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


def center_window(window, parent):
    """将窗口居中到父窗口，无闪烁版本"""
    window.update_idletasks()
    pw, ph = parent.winfo_width(), parent.winfo_height()
    px, py = parent.winfo_x(), parent.winfo_y()
    ww, wh = window.winfo_width(), window.winfo_height()
    x = px + (pw - ww) // 2
    y = py + (ph - wh) // 2
    window.geometry(f"+{x}+{y}")


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
            font=ctk.CTkFont(size=15, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=12, pady=(12, 8), anchor="w")
        ctk.CTkLabel(
            frame, text="未能识别剪贴板中的图片，以下是详细诊断：",
            font=ctk.CTkFont(size=12), text_color=COLORS["text_secondary"]
        ).pack(padx=12, anchor="w")

        textbox = ctk.CTkTextbox(
            frame, wrap="word", font=ctk.CTkFont(size=11),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"]
        )
        textbox.pack(fill="both", expand=True, padx=12, pady=8)
        textbox.insert("1.0", "\n".join(diagnostics))
        textbox.configure(state="disabled")

        ctk.CTkButton(
            frame, text="关闭", width=80, height=32,
            font=ctk.CTkFont(size=12),
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
        self.geometry("360x340")
        self.resizable(False, False)
        self.transient(parent)
        self.configure(fg_color=COLORS["bg"])
        self.on_save = on_save
        self._parent = parent

        self.settings = load_settings()

        frame = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=12)
        frame.pack(fill="both", expand=True, padx=16, pady=16)

        ctk.CTkLabel(
            frame, text="设置",
            font=ctk.CTkFont(size=16, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=16, pady=(16, 0), anchor="w")

        # 悬浮延迟
        delay_frame = ctk.CTkFrame(frame, fg_color="transparent")
        delay_frame.pack(fill="x", padx=16, pady=(12, 4))
        ctk.CTkLabel(
            delay_frame, text="悬浮预览延迟",
            font=ctk.CTkFont(size=13), text_color=COLORS["text"]
        ).pack(side="left")
        self.delay_var = ctk.DoubleVar(value=self.settings.get("hover_delay_ms", 800) / 1000.0)
        self.delay_label = ctk.CTkLabel(
            delay_frame, text=f"{self.delay_var.get():.1f} 秒",
            font=ctk.CTkFont(size=13, weight="bold"),
            text_color=COLORS["primary"], width=60
        )
        self.delay_label.pack(side="right")

        self.slider = ctk.CTkSlider(
            frame, from_=0.2, to=3.0, number_of_steps=28,
            variable=self.delay_var, command=self._on_slider_change
        )
        self.slider.pack(fill="x", padx=16, pady=(0, 2))
        ctk.CTkLabel(
            frame, text="鼠标放在图片上多久后显示预览",
            font=ctk.CTkFont(size=11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        # 排序
        sort_frame = ctk.CTkFrame(frame, fg_color="transparent")
        sort_frame.pack(fill="x", padx=16, pady=(12, 4))
        ctk.CTkLabel(
            sort_frame, text="消息排序",
            font=ctk.CTkFont(size=13), text_color=COLORS["text"]
        ).pack(side="left")
        self.sort_var = ctk.StringVar(value=self.settings.get("sort_order", "newest"))
        sort_menu = ctk.CTkOptionMenu(
            sort_frame, variable=self.sort_var,
            values=["newest", "oldest"], width=120, height=28,
            font=ctk.CTkFont(size=12),
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
            font=ctk.CTkFont(size=13), text_color=COLORS["text"],
            progress_color=COLORS["primary"],
            button_color=COLORS["primary"],
            button_hover_color=COLORS["primary_hover"],
        )
        archive_switch.pack(fill="x", padx=16, pady=(12, 0))
        ctk.CTkLabel(
            frame, text="导入完成后自动将消息移入已归档",
            font=ctk.CTkFont(size=11), text_color=COLORS["text_hint"]
        ).pack(padx=16, anchor="w")

        # 按钮
        btn_frame = ctk.CTkFrame(frame, fg_color="transparent")
        btn_frame.pack(fill="x", padx=16, pady=(16, 12))
        ctk.CTkButton(
            btn_frame, text="取消", width=68, height=32,
            font=ctk.CTkFont(size=12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._close
        ).pack(side="right", padx=(6, 0))
        ctk.CTkButton(
            btn_frame, text="保存", width=68, height=32,
            font=ctk.CTkFont(size=12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._save
        ).pack(side="right")

        self.protocol("WM_DELETE_WINDOW", self._close)
        self._place_centered()
        self.grab_set()  # 在窗口显示并居中后再 grab，避免 withdraw/deiconify 破坏 grab 状态

    def _on_slider_change(self, value):
        self.delay_label.configure(text=f"{value:.1f} 秒")

    def _save(self):
        self.settings["hover_delay_ms"] = int(self.delay_var.get() * 1000)
        self.settings["auto_archive_after_import"] = self.archive_var.get()
        self.settings["sort_order"] = self.sort_var.get()
        save_settings(self.settings)
        if self.on_save:
            self.on_save()
        self._close()

    def _close(self):
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
        self.geometry("380x260")
        self.resizable(False, False)
        self.transient(parent)
        self.configure(fg_color=COLORS["bg"])
        self._parent = parent
        self.release_url = release.get("html_url") or f"https://github.com/{APP_REPOSITORY}/releases/latest"

        frame = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=12)
        frame.pack(fill="both", expand=True, padx=16, pady=16)

        ctk.CTkLabel(
            frame, text=f"发现新版本 {latest_version}",
            font=ctk.CTkFont(size=16, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=16, pady=(16, 6), anchor="w")

        ctk.CTkLabel(
            frame, text=f"当前版本 {APP_VERSION}，可以前往 GitHub Release 下载新版。",
            font=ctk.CTkFont(size=12), text_color=COLORS["text_secondary"],
            wraplength=320, justify="left"
        ).pack(padx=16, anchor="w")

        notes = (release.get("body") or "该版本没有填写更新说明。").strip()
        textbox = ctk.CTkTextbox(
            frame, height=90, wrap="word", font=ctk.CTkFont(size=11),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text"],
            border_width=0, corner_radius=8
        )
        textbox.pack(fill="x", padx=16, pady=(12, 0))
        textbox.insert("1.0", notes[:800])
        textbox.configure(state="disabled")

        btn_frame = ctk.CTkFrame(frame, fg_color="transparent")
        btn_frame.pack(fill="x", padx=16, pady=(14, 12))
        ctk.CTkButton(
            btn_frame, text="稍后", width=72, height=32,
            font=ctk.CTkFont(size=12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._close
        ).pack(side="right", padx=(6, 0))
        ctk.CTkButton(
            btn_frame, text="打开下载页", width=108, height=32,
            font=ctk.CTkFont(size=12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._open_release
        ).pack(side="right")

        self.protocol("WM_DELETE_WINDOW", self._close)
        self._place_centered()
        self.grab_set()

    def _open_release(self):
        webbrowser.open(self.release_url)
        self._close()

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
            font=ctk.CTkFont(size=16, weight="bold"), text_color=COLORS["text"]
        ).pack(padx=16, pady=(16, 8), anchor="w")

        # 图片区域（上方）
        self.img_frame = ctk.CTkFrame(main, fg_color="transparent")
        self.img_frame.pack(fill="x", padx=12, pady=(0, 8))
        if not self.images:
            self.img_frame.pack_forget()

        # 文字输入（下方）
        self.textbox = ctk.CTkTextbox(
            main, height=150, wrap="word",
            font=ctk.CTkFont(size=13),
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
            font=ctk.CTkFont(size=11), text_color=COLORS["text_hint"]
        )
        self.hint_label.pack(side="left")

        ctk.CTkButton(
            toolbar, text="取消", width=68, height=34,
            font=ctk.CTkFont(size=12),
            fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
            hover_color=COLORS["border"], corner_radius=8,
            command=self._on_close
        ).pack(side="right", padx=(6, 0))
        ctk.CTkButton(
            toolbar, text="保存", width=68, height=34,
            font=ctk.CTkFont(size=12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._save
        ).pack(side="right")

        self.bind("<Control-v>", self._on_paste)
        self.bind("<Control-V>", self._on_paste)
        self.bind("<Shift-Insert>", self._on_paste)
        self.textbox.bind("<Control-v>", self._on_paste)
        self.textbox.bind("<Control-V>", self._on_paste)
        self.textbox.bind("<Shift-Insert>", self._on_paste)

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
            font=ctk.CTkFont(size=12, weight="bold"), text_color=COLORS["text"]
        ).pack(side="left")
        ctk.CTkLabel(
            header, text="Ctrl+V 继续添加",
            font=ctk.CTkFont(size=11), text_color=COLORS["text_hint"]
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
                font=ctk.CTkFont(size=11, weight="bold"),
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
        text = self.textbox.get("1.0", "end-1c").strip()
        images_data = [data for _, data in self.images]
        self._saved_text = text if text else None
        self._saved_images = images_data
        # 先禁用按钮防止重复点击
        self.after(50, self._do_save_and_close)

    def _do_save_and_close(self):
        """实际执行保存和关闭"""
        self.on_save(self._saved_text, self._saved_images)
        self._on_close()


# ========== 消息卡片 ==========
class MessageCard(ctk.CTkFrame, HoverPreviewMixin):
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

        if image_filenames:
            self._render_images()
        if text_content:
            self._render_text()
        elif not image_filenames:
            ctk.CTkLabel(
                self, text="（空消息）",
                font=ctk.CTkFont(size=12), text_color=COLORS["text_hint"]
            ).pack(padx=14, pady=(10, 4), anchor="w")

        self._render_footer(view_mode)

    def _render_images(self):
        """网格布局：一行三个，最多三行"""
        max_per_row = 3
        max_rows = 3
        max_display = max_per_row * max_rows

        img_container = ctk.CTkFrame(self, fg_color="transparent")
        img_container.pack(fill="x", padx=10, pady=(8, 0))

        for idx, img_file in enumerate(self.image_filenames[:max_display]):
            image_path = db.get_image_path(img_file)
            if image_path and os.path.exists(image_path):
                try:
                    pil_image = Image.open(image_path)
                    ctk_img = _pil_to_ctk(pil_image, 120, 100)

                    # 圆角边框容器
                    row = idx // max_per_row
                    col = idx % max_per_row
                    frame = ctk.CTkFrame(
                        img_container,
                        fg_color=COLORS["tag_bg"],
                        corner_radius=8,
                        border_width=1,
                        border_color=COLORS["border"]
                    )
                    frame.grid(
                        row=row, column=col,
                        padx=(0, 6) if col < 2 else 0,
                        pady=(0, 6) if row < 2 else 0,
                        sticky="nsew"
                    )

                    lbl = ctk.CTkLabel(frame, image=ctk_img, text="", cursor="hand2")
                    lbl.pack(padx=4, pady=4)
                    lbl.image = ctk_img
                    self.bind_hover_preview(lbl, image_path)
                    lbl.bind(
                        "<Button-1>",
                        lambda e, p=image_path: self.callbacks["copy_image"](p)
                    )
                except Exception:
                    pass

        for c in range(max_per_row):
            img_container.grid_columnconfigure(c, weight=1)

        if len(self.image_filenames) > max_display:
            ctk.CTkLabel(
                img_container,
                text=f"还有 {len(self.image_filenames) - max_display} 张图片...",
                font=ctk.CTkFont(size=11), text_color=COLORS["text_hint"]
            ).grid(row=max_rows, column=0, columnspan=max_per_row, sticky="w", pady=(2, 0))

    def _render_text(self):
        text_bg = ctk.CTkFrame(self, fg_color=COLORS["tag_bg"], corner_radius=6)
        text_bg.pack(fill="x", padx=10, pady=(0, 10))
        text_label = ctk.CTkLabel(
            text_bg, text=self.text_content,
            wraplength=430, justify="left",
            font=ctk.CTkFont(size=13), text_color=COLORS["text"],
            cursor="hand2"
        )
        text_label.pack(padx=10, pady=8, anchor="w")

        def on_click(e, txt=self.text_content):
            self.callbacks["copy_text"](txt)
        text_label.bind("<Button-1>", on_click)
        text_bg.bind("<Button-1>", on_click)

    def _render_footer(self, view_mode):
        footer = ctk.CTkFrame(self, fg_color="transparent", height=28)
        footer.pack(fill="x", padx=10, pady=(0, 8))
        footer.pack_propagate(False)

        time_str = str(self.created_at).split(".")[0] if self.created_at else ""
        ctk.CTkLabel(
            footer, text=time_str,
            font=ctk.CTkFont(size=10), text_color=COLORS["text_hint"]
        ).pack(side="left")

        archive_text = "恢复" if view_mode == "archived" else "归档"

        # 删除按钮仅在归档页面显示
        if view_mode == "archived":
            ctk.CTkButton(
                footer, text="×", width=24, height=24,
                font=ctk.CTkFont(size=12, weight="bold"),
                fg_color="transparent", text_color=COLORS["text_hint"],
                hover_color=COLORS["danger"], corner_radius=6,
                command=lambda: self.callbacks["delete"](self.msg_id)
            ).pack(side="right", padx=(4, 0))

        ctk.CTkButton(
            footer, text=archive_text, width=52, height=24,
            font=ctk.CTkFont(size=11),
            fg_color="transparent", text_color=COLORS["text_secondary"],
            hover_color=COLORS["tag_bg"], corner_radius=6,
            command=lambda: self.callbacks["archive"](self.msg_id)
        ).pack(side="right")

        if view_mode == "active":
            ctk.CTkButton(
                footer, text="编辑", width=42, height=24,
                font=ctk.CTkFont(size=11),
                fg_color=COLORS["tag_bg"], text_color=COLORS["text_secondary"],
                hover_color=COLORS["border"], corner_radius=6,
                command=lambda: self.callbacks["edit"](self.msg_id)
            ).pack(side="right", padx=(0, 4))

            ctk.CTkButton(
                footer, text="导入", width=42, height=24,
                font=ctk.CTkFont(size=11),
                fg_color=COLORS["primary"], text_color="white",
                hover_color=COLORS["primary_hover"], corner_radius=6,
                command=lambda: self.callbacks["import_message"](self.msg_id)
            ).pack(side="right", padx=(0, 4))


# ========== 主窗口 ==========
class DemandStashApp(ctk.CTk):
    def __init__(self):
        super().__init__()
        self.title(f"{APP_NAME} {APP_VERSION}  @linjianglu")
        self.geometry("420x720")
        self.minsize(380, 520)

        # 设置任务栏和窗口图标
        try:
            icon_path = _ensure_app_icon()
            self.after(100, lambda: self.iconbitmap(icon_path))
        except Exception:
            pass

        ctk.set_appearance_mode("System")
        ctk.set_default_color_theme("blue")
        self.configure(fg_color=COLORS["bg"])

        self.view_mode = "active"
        self._always_on_top = False
        self._tray_icon = None
        self._import_queue = []
        self._import_msg_id = None
        self._editor_dialog = None
        self._checking_update = False

        self.bind("<Control-v>", self._on_paste)
        self.bind("<Control-V>", self._on_paste)
        self.bind("<Shift-Insert>", self._on_paste)

        self._create_header()
        self._create_content()
        self._create_footer()
        self.load_items()

        self.protocol("WM_DELETE_WINDOW", self._hide_to_tray)

        if HAS_TRAY:
            self._setup_tray()
            self._start_hotkey_listener()

    # ---------- 托盘 & 快捷键 ----------
    def _create_tray_image(self):
        return _load_app_icon_image(64)

    def _setup_tray(self):
        image = self._create_tray_image()
        menu = pystray.Menu(
            pystray.MenuItem("显示", self._show_from_tray, default=True),
            pystray.MenuItem("检查更新", self._check_for_updates),
            pystray.MenuItem("退出", self._quit_from_tray),
        )
        self._tray_icon = pystray.Icon(APP_NAME, image, APP_NAME, menu=menu)
        threading.Thread(target=self._tray_icon.run, daemon=True).start()

    def _start_hotkey_listener(self):
        def on_hotkey():
            self.after(0, self._toggle_show_hide)

        def listen():
            with keyboard.GlobalHotKeys({"<ctrl>+<shift>+v": on_hotkey}) as h:
                h.join()

        threading.Thread(target=listen, daemon=True).start()

    def _show_from_tray(self, icon=None, item=None):
        self.after(0, self._do_show)

    def _do_show(self):
        self.deiconify()
        self.lift()
        self.focus_force()

    def _hide_to_tray(self):
        self.withdraw()

    def _toggle_show_hide(self):
        if self.state() == "withdrawn":
            self._do_show()
        else:
            self._hide_to_tray()

    def _quit_from_tray(self, icon=None, item=None):
        self.after(0, self._quit_app)

    def _quit_app(self):
        if self._tray_icon:
            try:
                self._tray_icon.stop()
            except Exception:
                pass
        self.destroy()

    # ---------- UI 组件 ----------
    def _create_header(self):
        self.header = ctk.CTkFrame(self, fg_color=COLORS["card"], corner_radius=0, height=56)
        self.header.pack(fill="x")
        self.header.pack_propagate(False)

        title_frame = ctk.CTkFrame(self.header, fg_color="transparent")
        title_frame.pack(side="left", padx=16, pady=10)
        ctk.CTkLabel(title_frame, text=APP_NAME,
                     font=ctk.CTkFont(size=18, weight="bold"),
                     text_color=COLORS["primary"]).pack(side="left")

        btn_frame = ctk.CTkFrame(self.header, fg_color="transparent")
        btn_frame.pack(side="right", padx=16, pady=10)

        self.count_label = ctk.CTkLabel(
            btn_frame, text="0 条",
            font=ctk.CTkFont(size=12), text_color=COLORS["text_hint"]
        )
        self.count_label.pack(side="left", padx=(0, 10))

        self.pin_btn = ctk.CTkButton(
            btn_frame, text="置顶", width=48, height=30,
            font=ctk.CTkFont(size=11),
            fg_color="transparent", text_color=COLORS["text_hint"],
            hover_color=COLORS["tag_bg"], corner_radius=6,
            command=self._toggle_always_on_top
        )
        self.pin_btn.pack(side="left", padx=(0, 4))

        self.settings_btn = ctk.CTkButton(
            btn_frame, text="设置", width=48, height=30,
            font=ctk.CTkFont(size=11),
            fg_color="transparent", text_color=COLORS["text_hint"],
            hover_color=COLORS["tag_bg"], corner_radius=6,
            command=self._open_settings
        )
        self.settings_btn.pack(side="left", padx=(0, 4))

        self.update_btn = ctk.CTkButton(
            btn_frame, text="更新", width=48, height=30,
            font=ctk.CTkFont(size=11),
            fg_color="transparent", text_color=COLORS["text_hint"],
            hover_color=COLORS["tag_bg"], corner_radius=6,
            command=self._check_for_updates
        )
        self.update_btn.pack(side="left", padx=(0, 4))

        self.new_msg_btn = ctk.CTkButton(
            btn_frame, text="+ 新建", width=80, height=32,
            font=ctk.CTkFont(size=12, weight="bold"),
            fg_color=COLORS["primary"], hover_color=COLORS["primary_hover"],
            corner_radius=8, command=self._on_new_message,
        )
        self.new_msg_btn.pack(side="left")

    def _create_content(self):
        content_frame = ctk.CTkFrame(self, fg_color="transparent")
        content_frame.pack(fill="both", expand=True, padx=12, pady=12)

        self.tab_frame = ctk.CTkFrame(content_frame, fg_color=COLORS["card"], corner_radius=10)
        self.tab_frame.pack(fill="x", pady=(0, 8))

        self.tab_active = ctk.CTkButton(
            self.tab_frame, text="消息", width=80, height=32,
            font=ctk.CTkFont(size=12, weight="bold"),
            fg_color=COLORS["primary"], text_color="white",
            hover_color=COLORS["primary_hover"], corner_radius=8,
            command=lambda: self._switch_view("active")
        )
        self.tab_active.pack(side="left", padx=8, pady=8)

        self.tab_archived = ctk.CTkButton(
            self.tab_frame, text="已归档", width=80, height=32,
            font=ctk.CTkFont(size=12),
            fg_color="transparent", text_color=COLORS["text_secondary"],
            hover_color=COLORS["tag_bg"], corner_radius=8,
            command=lambda: self._switch_view("archived")
        )
        self.tab_archived.pack(side="left", padx=(0, 8), pady=8)

        self.scroll_frame = ctk.CTkScrollableFrame(
            content_frame, fg_color="transparent",
            scrollbar_button_color=COLORS["border"],
            scrollbar_button_hover_color=COLORS["text_hint"]
        )
        self.scroll_frame.pack(fill="both", expand=True)

    def _create_footer(self):
        self.footer = ctk.CTkFrame(self, fg_color=COLORS["card"], height=36)
        self.footer.pack(fill="x", side="bottom")
        self.footer.pack_propagate(False)
        self.status_bar = ctk.CTkLabel(
            self.footer, text="",
            font=ctk.CTkFont(size=11), text_color=COLORS["text_secondary"]
        )
        self.status_bar.pack(side="left", padx=16)
        ctk.CTkLabel(
            self.footer, text="Ctrl+Shift+V 呼出",
            font=ctk.CTkFont(size=11), text_color=COLORS["text_hint"]
        ).pack(side="right", padx=16)

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
        SettingsDialog(self, on_save=self.load_items)

    def _check_for_updates(self, icon=None, item=None):
        if self._checking_update:
            self.after(0, lambda: self._show_status("正在检查更新..."))
            return
        self._checking_update = True
        self.after(0, lambda: self._show_status("正在检查更新..."))

        def worker():
            try:
                release = _fetch_latest_release()
                latest_version = release.get("tag_name", "")
                has_update = _parse_version(latest_version) > _parse_version(APP_VERSION)
                self.after(0, lambda: self._finish_update_check(release, has_update))
            except urllib.error.HTTPError as e:
                message = "未找到 GitHub Release" if e.code == 404 else f"检查更新失败: HTTP {e.code}"
                self.after(0, lambda: self._finish_update_check(None, False, message))
            except Exception as e:
                message = f"检查更新失败: {e}"
                self.after(0, lambda: self._finish_update_check(None, False, message))

        threading.Thread(target=worker, daemon=True).start()

    def _finish_update_check(self, release, has_update, message=None):
        self._checking_update = False
        if message:
            self._show_status(message)
            return
        if has_update:
            self._show_status(f"发现新版本 {release.get('tag_name', '')}")
            UpdateDialog(self, release)
        else:
            latest_version = release.get("tag_name", APP_VERSION) if release else APP_VERSION
            self._show_status(f"已是最新版本 {latest_version}")

    def _switch_view(self, mode):
        self.view_mode = mode
        if mode == "active":
            self.tab_active.configure(fg_color=COLORS["primary"], text_color="white",
                                      font=ctk.CTkFont(size=12, weight="bold"))
            self.tab_archived.configure(fg_color="transparent", text_color=COLORS["text_secondary"],
                                        font=ctk.CTkFont(size=12))
        else:
            self.tab_active.configure(fg_color="transparent", text_color=COLORS["text_secondary"],
                                      font=ctk.CTkFont(size=12))
            self.tab_archived.configure(fg_color=COLORS["primary"], text_color="white",
                                        font=ctk.CTkFont(size=12, weight="bold"))
        self.load_items()

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
            if self.view_mode != "active":
                self._switch_view("active")
            else:
                self.load_items()
            self._show_status("已保存")
        self._open_editor(on_save)

    def _on_paste(self, event=None):
        img, diagnostics = get_clipboard_image()
        if img is None:
            text = get_clipboard_text()
            if text:
                db.add_message(text_content=text)
                if self.view_mode != "active":
                    self._switch_view("active")
                else:
                    self.load_items()
                self._show_status("已保存文字")
                return "break"
            DiagnoseDialog(self, diagnostics)
            return

        def on_save(text, images_data):
            db.add_message(text_content=text, images_data=images_data)
            if self.view_mode != "active":
                self._switch_view("active")
            else:
                self.load_items()
            self._show_status("已保存")

        dialog = self._open_editor(on_save)
        buf = BytesIO()
        img.save(buf, format="PNG")
        dialog.images.append((img, buf.getvalue()))
        dialog._render_thumbnails()

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
            self.load_items()
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
        self._hide_to_tray()
        self._show_status("正在导入...")
        self.after(300, self._do_import_step)

    def _do_import_step(self):
        if not self._import_queue:
            self._show_status("导入完成")
            if get_auto_archive_after_import() and self._import_msg_id:
                db.toggle_archive(self._import_msg_id)
                self._import_msg_id = None
                self.load_items()
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
        self.load_items()
        self._show_status("已归档" if new_val else "已恢复")

    def _delete_message(self, msg_id: int):
        db.delete_message(msg_id)
        self.load_items()
        self._show_status("已删除")

    def _show_status(self, message, duration=2000):
        self.status_bar.configure(text=message)
        self.after(duration, lambda: self.status_bar.configure(text=""))

    def load_items(self):
        # 安全清理：只销毁 _parent_frame 内的内容，不动 canvas/scrollbar
        parent_frame = self.scroll_frame._parent_frame
        for widget in list(parent_frame.winfo_children()):
            widget.destroy()
        parent_frame.update_idletasks()

        sort_order = get_sort_order()
        items = db.get_all_messages(
            archived=(self.view_mode == "archived"),
            sort_order=sort_order
        )
        count = len(items)
        label = "已归档" if self.view_mode == "archived" else "消息"
        self.count_label.configure(text=f"{count} 条{label}")

        if not items:
            self._render_empty_state()
            return

        callbacks = {
            "copy_image": self._copy_image,
            "copy_text": self._copy_text,
            "edit": self._on_edit_message,
            "import_message": self._on_import_message,
            "archive": self._on_archive,
            "delete": self._delete_message,
        }

        for item in items:
            MessageCard(parent_frame, item, self.view_mode, callbacks)

    def _render_empty_state(self):
        parent_frame = self.scroll_frame._parent_frame
        empty_frame = ctk.CTkFrame(parent_frame, fg_color="transparent")
        empty_frame.pack(fill="both", expand=True, pady=80)

        if self.view_mode == "archived":
            icon, title, desc = "📦", "没有归档消息", "归档的消息会显示在这里"
        else:
            icon, title, desc = "📋", "还没有消息", "Ctrl+V 粘贴截图，或点击「+ 新建」"

        ctk.CTkLabel(empty_frame, text=icon, font=ctk.CTkFont(size=48)).pack(pady=(0, 12))
        ctk.CTkLabel(empty_frame, text=title,
                     font=ctk.CTkFont(size=16, weight="bold"),
                     text_color=COLORS["text"]).pack(pady=(0, 8))
        ctk.CTkLabel(empty_frame, text=desc,
                     font=ctk.CTkFont(size=13),
                     text_color=COLORS["text_hint"]).pack()

        if self.view_mode != "archived":
            empty_frame.bind("<Button-1>", lambda e: self._on_new_message())
            for child in empty_frame.winfo_children():
                child.bind("<Button-1>", lambda e: self._on_new_message())

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
    _set_windows_app_id()
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
