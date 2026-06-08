use crate::{
    legacy_clipboard::{
        copy_legacy_message_import_queue_item_to_clipboard_from_dir,
        preview_legacy_message_import_queue_from_dir,
    },
    legacy_data::{list_legacy_messages, read_legacy_stats},
    legacy_model::{LegacyMessage, MessageView, SortOrder},
    legacy_paths::legacy_data_dir,
    legacy_query::{list_legacy_messages_from_dir, query_count, read_legacy_stats_from_dir},
    legacy_test_support::{
        assert_message_order_matches_db, collect_all_messages, query_image_rows, tiny_png_bytes,
    },
};
use rusqlite::{Connection, OpenFlags};
use std::{env, fs, process};

#[test]
fn reads_counts_from_legacy_messages_table() {
    let data_dir = env::temp_dir().join(format!(
        "clipstash-next-legacy-stats-test-{}",
        process::id()
    ));
    let _ = fs::remove_dir_all(&data_dir);
    fs::create_dir_all(data_dir.join("images")).expect("create images dir");

    let db_path = data_dir.join("clipstash.db");
    let conn = Connection::open(&db_path).expect("create sqlite fixture");
    conn.execute_batch(
        "
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text_content TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            archived INTEGER DEFAULT 0,
            archived_at TIMESTAMP
        );
        INSERT INTO messages (text_content, archived) VALUES ('normal', 0);
        INSERT INTO messages (text_content, archived) VALUES ('archived', 1);
        INSERT INTO messages (text_content, archived) VALUES ('legacy-null', NULL);
        ",
    )
    .expect("seed fixture");
    drop(conn);

    let stats = read_legacy_stats_from_dir(data_dir.clone()).expect("read legacy stats");

    assert!(stats.db_exists);
    assert!(stats.images_dir_exists);
    assert_eq!(stats.normal_count, 2);
    assert_eq!(stats.archived_count, 1);
    assert_eq!(stats.total_count, 3);

    fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
}

#[test]
fn lists_messages_with_ordered_image_status() {
    let data_dir =
        env::temp_dir().join(format!("clipstash-next-legacy-list-test-{}", process::id()));
    let _ = fs::remove_dir_all(&data_dir);
    fs::create_dir_all(data_dir.join("images")).expect("create images dir");
    fs::write(data_dir.join("images").join("existing.png"), b"png").expect("seed image");

    let db_path = data_dir.join("clipstash.db");
    let conn = Connection::open(&db_path).expect("create sqlite fixture");
    conn.execute_batch(
        "
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text_content TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            archived INTEGER DEFAULT 0,
            archived_at TIMESTAMP
        );
        CREATE TABLE message_images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_filename TEXT NOT NULL
        );
        INSERT INTO messages (id, text_content, created_at, archived) VALUES
            (1, 'older', '2024-01-01 00:00:00', 0),
            (2, 'newer', '2024-02-01 00:00:00', 0),
            (3, 'archived', '2024-03-01 00:00:00', 1);
        INSERT INTO message_images (id, message_id, image_filename) VALUES
            (10, 2, 'existing.png'),
            (11, 2, 'missing.png');
        ",
    )
    .expect("seed fixture");
    drop(conn);

    let page = list_legacy_messages_from_dir(
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Newest,
        Some(0),
        Some(10),
    )
    .expect("list normal messages");

    assert_eq!(page.total_count, 2);
    assert!(!page.has_more);
    assert_eq!(page.messages[0].id, 2);
    assert_eq!(page.messages[1].id, 1);
    assert_eq!(page.messages[0].images[0].id, 10);
    assert!(page.messages[0].images[0].exists);
    assert_eq!(page.messages[0].images[1].id, 11);
    assert!(!page.messages[0].images[1].exists);

    let archived_page = list_legacy_messages_from_dir(
        data_dir.clone(),
        MessageView::Archived,
        SortOrder::Newest,
        Some(0),
        Some(10),
    )
    .expect("list archived messages");

    assert_eq!(archived_page.total_count, 1);
    assert_eq!(archived_page.messages[0].id, 3);
    assert!(archived_page.messages[0].archived);

    fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
}

#[test]
fn previews_import_queue_in_legacy_order_without_writing() {
    let data_dir = env::temp_dir().join(format!(
        "clipstash-next-import-queue-test-{}",
        process::id()
    ));
    let _ = fs::remove_dir_all(&data_dir);
    fs::create_dir_all(data_dir.join("images")).expect("create images dir");
    fs::write(data_dir.join("images").join("second.png"), tiny_png_bytes())
        .expect("seed second image");
    fs::write(data_dir.join("images").join("first.png"), tiny_png_bytes())
        .expect("seed first image");

    let db_path = data_dir.join("clipstash.db");
    let conn = Connection::open(&db_path).expect("create sqlite fixture");
    conn.execute_batch(
        "
        CREATE TABLE messages (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            text_content TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
            archived INTEGER DEFAULT 0,
            archived_at TIMESTAMP
        );
        CREATE TABLE message_images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_filename TEXT NOT NULL
        );
        INSERT INTO messages (id, text_content, archived) VALUES
            (1, '  hello queue  ', 0),
            (2, NULL, 0);
        INSERT INTO message_images (id, message_id, image_filename) VALUES
            (21, 1, 'second.png'),
            (20, 1, 'first.png'),
            (22, 1, 'missing.png');
        ",
    )
    .expect("seed fixture");
    drop(conn);

    let preview = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 1)
        .expect("preview import queue");

    assert_eq!(preview.message_id, 1);
    assert_eq!(preview.item_count, 3);
    assert_eq!(preview.text_length, 11);
    assert_eq!(preview.image_count, 2);
    assert_eq!(preview.skipped_missing_image_count, 1);
    assert_eq!(preview.items[0].kind, "text");
    assert_eq!(preview.items[0].text.as_deref(), Some("hello queue"));
    assert_eq!(
        preview.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("first.png")
    );
    assert_eq!(
        preview.items[2]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("second.png")
    );

    let empty = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 2)
        .expect_err("empty message should fail preview");
    assert!(empty.contains("消息为空或图片文件缺失"));

    let out_of_range =
        copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir.clone(), 1, 3)
            .expect_err("out-of-range queue item should fail before writing clipboard");
    assert!(out_of_range.contains("索引超出范围"));

    let empty_copy =
        copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir.clone(), 2, 0)
            .expect_err("empty message should fail before writing clipboard");
    assert!(empty_copy.contains("消息为空或图片文件缺失"));

    fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
}

#[test]
#[ignore = "requires local ClipStash app data"]
fn reads_local_legacy_stats_when_available() {
    let stats = read_legacy_stats().expect("read local legacy stats");

    eprintln!(
        "normal={} archived={} total={} db={}",
        stats.normal_count, stats.archived_count, stats.total_count, stats.db_path
    );

    assert!(stats.db_exists);
    assert_eq!(stats.total_count, stats.normal_count + stats.archived_count);
}

#[test]
#[ignore = "requires local ClipStash app data"]
fn lists_local_legacy_messages_when_available() {
    let page = list_legacy_messages(MessageView::Normal, SortOrder::Newest, Some(0), Some(5))
        .expect("list local legacy messages");

    eprintln!(
        "view={} total={} returned={} has_more={}",
        page.view,
        page.total_count,
        page.messages.len(),
        page.has_more
    );

    assert!(page.total_count >= page.messages.len() as i64);
    for message in page.messages {
        assert!(!message.archived);
    }
}

#[test]
#[ignore = "requires local ClipStash app data"]
fn verifies_local_legacy_readonly_consistency() {
    let data_dir = legacy_data_dir().expect("resolve local legacy data dir");
    let db_path = data_dir.join("clipstash.db");
    let images_dir = data_dir.join("images");
    let conn = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .expect("open local legacy database read-only");

    let stats = read_legacy_stats_from_dir(data_dir.clone()).expect("read local stats");
    let normal_messages = collect_all_messages(data_dir.clone(), MessageView::Normal);
    let archived_messages = collect_all_messages(data_dir.clone(), MessageView::Archived);
    let all_messages: Vec<&LegacyMessage> = normal_messages
        .iter()
        .chain(archived_messages.iter())
        .collect();

    assert_eq!(stats.normal_count, normal_messages.len() as i64);
    assert_eq!(stats.archived_count, archived_messages.len() as i64);
    assert_eq!(
        stats.total_count,
        (normal_messages.len() + archived_messages.len()) as i64
    );

    for message in &normal_messages {
        assert!(
            !message.archived,
            "normal view included archived message {}",
            message.id
        );
    }
    for message in &archived_messages {
        assert!(
            message.archived,
            "archived view included normal message {}",
            message.id
        );
    }

    assert_message_order_matches_db(&conn, MessageView::Normal, SortOrder::Newest);
    assert_message_order_matches_db(&conn, MessageView::Normal, SortOrder::Oldest);
    assert_message_order_matches_db(&conn, MessageView::Archived, SortOrder::Newest);
    assert_message_order_matches_db(&conn, MessageView::Archived, SortOrder::Oldest);

    let api_image_count: i64 = all_messages
        .iter()
        .map(|message| message.images.len() as i64)
        .sum();
    let db_joined_image_count = query_count(
        &conn,
        "SELECT COUNT(*) \
         FROM message_images mi \
         JOIN messages m ON m.id = mi.message_id",
    )
    .expect("count joined images");
    let db_orphan_image_count = query_count(
        &conn,
        "SELECT COUNT(*) \
         FROM message_images mi \
         LEFT JOIN messages m ON m.id = mi.message_id \
         WHERE m.id IS NULL",
    )
    .expect("count orphan images");

    assert_eq!(api_image_count, db_joined_image_count);

    for message in all_messages {
        let db_images = query_image_rows(&conn, message.id);
        assert_eq!(
            db_images.len(),
            message.images.len(),
            "image count mismatch for message {}",
            message.id
        );

        let mut previous_image_id = None;
        for (index, image) in message.images.iter().enumerate() {
            let (db_image_id, db_filename) = &db_images[index];
            assert_eq!(
                &image.id, db_image_id,
                "image id mismatch for message {}",
                message.id
            );
            assert_eq!(
                &image.filename, db_filename,
                "image filename mismatch for message {}",
                message.id
            );
            assert_eq!(
                image.exists,
                images_dir.join(&image.filename).is_file(),
                "image file status mismatch for {}",
                image.filename
            );

            if let Some(previous) = previous_image_id {
                assert!(
                    image.id > previous,
                    "image order is not ascending for message {}",
                    message.id
                );
            }
            previous_image_id = Some(image.id);
        }
    }

    eprintln!(
        "legacy-readonly-ok normal={} archived={} total={} joined_images={} orphan_images={} db={}",
        stats.normal_count,
        stats.archived_count,
        stats.total_count,
        db_joined_image_count,
        db_orphan_image_count,
        db_path.display()
    );
}
