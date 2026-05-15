import sqlite3
import os
import uuid
from config import DB_PATH, IMAGES_DIR


def _save_image_file(image_data: bytes) -> str:
    """保存图片二进制到文件系统，返回文件名"""
    filename = f"{uuid.uuid4().hex}.png"
    path = os.path.join(IMAGES_DIR, filename)
    with open(path, "wb") as f:
        f.write(image_data)
    return filename


def _delete_image_file(filename: str):
    """删除文件系统中的图片"""
    if not filename:
        return
    path = os.path.join(IMAGES_DIR, filename)
    if os.path.exists(path):
        os.remove(path)


def init_db():
    """初始化数据库，支持自动迁移旧数据和新增字段"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    # 新表：元消息
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text_content TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)

    # 检查并添加 archived 字段
    cursor.execute("PRAGMA table_info(messages)")
    columns = [row[1] for row in cursor.fetchall()]
    if "archived" not in columns:
        cursor.execute("ALTER TABLE messages ADD COLUMN archived INTEGER DEFAULT 0")
        print("[Migrate] Added 'archived' column to messages")

    # 新表：消息关联的图片
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS message_images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_filename TEXT NOT NULL,
            FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
        )
    """)

    # 检查是否存在旧表并迁移数据
    cursor.execute("""
        SELECT name FROM sqlite_master 
        WHERE type='table' AND name='clip_items'
    """)
    if cursor.fetchone():
        cursor.execute("""
            SELECT id, text_content, image_filename, created_at 
            FROM clip_items ORDER BY id
        """)
        for row in cursor.fetchall():
            old_id, text, img_file, created = row
            cursor.execute(
                "INSERT INTO messages (id, text_content, created_at, archived) VALUES (?, ?, ?, 0)",
                (old_id, text, created)
            )
            if img_file:
                cursor.execute(
                    "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
                    (old_id, img_file)
                )
        cursor.execute("DROP TABLE clip_items")
        conn.commit()
        print("[Migrate] Old clip_items data migrated to new schema")

    conn.commit()
    conn.close()


def add_message(text_content: str = None, images_data: list = None) -> int:
    """
    创建一条元消息
    :param text_content: 配套文字（可选）
    :param images_data: 图片二进制列表（可选）
    :return: 新消息 id
    """
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        "INSERT INTO messages (text_content, archived) VALUES (?, 0)",
        (text_content,)
    )
    msg_id = cursor.lastrowid

    if images_data:
        for img_data in images_data:
            filename = _save_image_file(img_data)
            cursor.execute(
                "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
                (msg_id, filename)
            )

    conn.commit()
    conn.close()
    return msg_id


def get_all_messages(archived: bool = False, sort_order: str = "newest") -> list:
    """
    获取元消息
    :param archived: False 返回未归档，True 返回已归档
    :param sort_order: "newest" 最新优先, "oldest" 最早优先
    :return: [(id, text_content, [image_filenames...], created_at), ...]
    """
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    order = "DESC" if sort_order == "newest" else "ASC"
    cursor.execute(
        f"SELECT id, text_content, created_at FROM messages WHERE archived = ? ORDER BY created_at {order}",
        (1 if archived else 0,)
    )
    results = []
    for row in cursor.fetchall():
        msg_id, text, created = row
        cursor.execute(
            "SELECT image_filename FROM message_images WHERE message_id = ? ORDER BY id",
            (msg_id,)
        )
        images = [r[0] for r in cursor.fetchall()]
        results.append((msg_id, text, images, created))
    conn.close()
    return results


def get_message(msg_id: int) -> tuple:
    """
    根据 id 获取单条元消息
    :return: (id, text_content, [image_filenames...], created_at) 或 None
    """
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        "SELECT id, text_content, created_at FROM messages WHERE id = ?",
        (msg_id,)
    )
    row = cursor.fetchone()
    if row is None:
        conn.close()
        return None

    msg_id, text, created = row
    cursor.execute(
        "SELECT image_filename FROM message_images WHERE message_id = ? ORDER BY id",
        (msg_id,)
    )
    images = [r[0] for r in cursor.fetchall()]
    conn.close()
    return (msg_id, text, images, created)


def add_image_to_message(msg_id: int, image_data: bytes) -> str:
    """向已有消息追加一张图片"""
    filename = _save_image_file(image_data)
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        "INSERT INTO message_images (message_id, image_filename) VALUES (?, ?)",
        (msg_id, filename)
    )
    conn.commit()
    conn.close()
    return filename


def delete_message(msg_id: int) -> bool:
    """删除元消息，清理所有关联图片文件"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()

    cursor.execute(
        "SELECT image_filename FROM message_images WHERE message_id = ?",
        (msg_id,)
    )
    for row in cursor.fetchall():
        _delete_image_file(row[0])

    cursor.execute("DELETE FROM messages WHERE id = ?", (msg_id,))
    deleted = cursor.rowcount > 0
    conn.commit()
    conn.close()
    return deleted


def toggle_archive(msg_id: int) -> bool:
    """切换消息的归档状态，返回新的 archived 值"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT archived FROM messages WHERE id = ?", (msg_id,))
    row = cursor.fetchone()
    if row is None:
        conn.close()
        return False
    new_val = 0 if row[0] else 1
    cursor.execute(
        "UPDATE messages SET archived = ? WHERE id = ?",
        (new_val, msg_id)
    )
    conn.commit()
    conn.close()
    return new_val


def update_message_text(msg_id: int, text_content: str):
    """更新消息的文字内容"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        "UPDATE messages SET text_content = ? WHERE id = ?",
        (text_content, msg_id)
    )
    conn.commit()
    conn.close()


def delete_message_images(msg_id: int):
    """删除消息的所有关联图片（文件和数据库记录）"""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        "SELECT image_filename FROM message_images WHERE message_id = ?",
        (msg_id,)
    )
    for row in cursor.fetchall():
        _delete_image_file(row[0])
    cursor.execute("DELETE FROM message_images WHERE message_id = ?", (msg_id,))
    conn.commit()
    conn.close()


def get_image_path(image_filename: str) -> str:
    """获取图片完整路径"""
    if not image_filename:
        return None
    return os.path.join(IMAGES_DIR, image_filename)
