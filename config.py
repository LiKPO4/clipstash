import os
import json

_DEFAULT_APPDATA = os.environ.get("APPDATA", os.path.expanduser("~"))
DEFAULT_DATA_DIR = os.path.join(_DEFAULT_APPDATA, "ClipStash")

DATA_DIR = DEFAULT_DATA_DIR
DB_PATH = os.path.join(DATA_DIR, "clipstash.db")
IMAGES_DIR = os.path.join(DATA_DIR, "images")

SETTINGS_PATH = os.path.join(DATA_DIR, "settings.json")

DEFAULT_SETTINGS = {
    "hover_delay_ms": 800,
    "auto_archive_after_import": False,
    "sort_order": "newest",  # "newest" | "oldest"
    "launch_on_startup": False,
    "show_hotkey": "<ctrl>+<shift>+v",
    "capture_hotkey": "<ctrl>+<alt>+v",
    "scroll_speed": 2,
}

_settings = None


def load_settings():
    global _settings
    if _settings is not None:
        return _settings.copy()

    if os.path.exists(SETTINGS_PATH):
        try:
            with open(SETTINGS_PATH, "r", encoding="utf-8") as f:
                data = json.load(f)
            merged = DEFAULT_SETTINGS.copy()
            merged.update(data)
            _settings = merged
            return merged.copy()
        except Exception:
            pass

    _settings = DEFAULT_SETTINGS.copy()
    return _settings.copy()


def save_settings(settings):
    global _settings
    _settings = settings.copy()
    try:
        with open(SETTINGS_PATH, "w", encoding="utf-8") as f:
            json.dump(settings, f, ensure_ascii=False, indent=2)
    except Exception:
        pass


def get_hover_delay_ms():
    return load_settings().get("hover_delay_ms", 800)


def get_auto_archive_after_import():
    return load_settings().get("auto_archive_after_import", False)


def get_sort_order():
    return load_settings().get("sort_order", "newest")


def get_launch_on_startup():
    return load_settings().get("launch_on_startup", False)


def get_show_hotkey():
    return load_settings().get("show_hotkey", "<ctrl>+<shift>+v")


def get_capture_hotkey():
    return load_settings().get("capture_hotkey", "<ctrl>+<alt>+v")


def get_scroll_speed():
    return load_settings().get("scroll_speed", 2)


os.makedirs(DATA_DIR, exist_ok=True)
os.makedirs(IMAGES_DIR, exist_ok=True)
