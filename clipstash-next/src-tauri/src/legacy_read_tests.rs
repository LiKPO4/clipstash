use crate::{
    legacy_clipboard::{
        copy_legacy_message_import_queue_item_to_clipboard_from_dir,
        preview_legacy_message_import_queue_from_dir,
    },
    legacy_data::{list_legacy_messages, read_legacy_stats},
    legacy_model::{LegacyMessage, MessageView, SortOrder},
    legacy_paths::legacy_data_dir,
    legacy_query::{
        list_legacy_messages_from_dir, list_legacy_messages_from_dir_filtered, query_count,
        read_legacy_stats_from_dir,
    },
    legacy_test_support::{
        assert_message_order_matches_db_from_dir, collect_all_messages, query_image_rows,
        tiny_png_bytes,
    },
};
use rusqlite::{Connection, OpenFlags};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process,
};

#[test]
fn batch_images_preserve_page_boundaries_order_and_nullable_archive() {
    let data_dir = env::temp_dir().join(format!(
        "clipstash-batch-images-{}-{}",
        process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(data_dir.join("images")).unwrap();
    fs::write(data_dir.join("images/present.png"), b"fixture").unwrap();
    let conn = Connection::open(data_dir.join("clipstash.db")).unwrap();
    conn.execute_batch(
        "CREATE TABLE messages (id INTEGER PRIMARY KEY, text_content TEXT,
            created_at TEXT, archived INTEGER, archived_at TEXT);
         CREATE TABLE message_images (id INTEGER PRIMARY KEY, message_id INTEGER,
            image_filename TEXT);
         CREATE INDEX image_owner ON message_images(message_id);
         WITH RECURSIVE seq(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM seq WHERE x<205)
         INSERT INTO messages SELECT x, 'match', '2024-01-01',
            CASE WHEN x=1 THEN NULL WHEN x>103 THEN 1 ELSE 0 END, NULL FROM seq;
         INSERT INTO message_images SELECT id*10+2, id, 'missing.png' FROM messages WHERE id%3<>0;
         INSERT INTO message_images SELECT id*10+1, id, 'present.png' FROM messages WHERE id%3<>0;
         INSERT INTO message_images VALUES (99999, 99999, 'orphan.png');",
    )
    .unwrap();
    for (view, first, last) in [
        (MessageView::Normal, 1, 103),
        (MessageView::Archived, 104, 205),
    ] {
        for sort in [SortOrder::Newest, SortOrder::Oldest] {
            let mut actual = Vec::new();
            let mut offset = 0;
            loop {
                let page = list_legacy_messages_from_dir_filtered(
                    data_dir.clone(),
                    view,
                    sort,
                    Some(offset),
                    Some(500),
                    Some("match".into()),
                )
                .unwrap();
                assert_eq!(page.limit, 100);
                assert_eq!(page.total_count, last - first + 1);
                for message in &page.messages {
                    assert_eq!(message.archived, message.id > 103);
                    if message.id % 3 == 0 {
                        assert!(message.images.is_empty());
                    } else {
                        assert_eq!(
                            message
                                .images
                                .iter()
                                .map(|image| image.id)
                                .collect::<Vec<_>>(),
                            vec![message.id * 10 + 1, message.id * 10 + 2]
                        );
                        assert!(message.images[0].exists);
                        assert!(!message.images[1].exists);
                        assert_eq!(message.images[0].filename, "present.png");
                    }
                    actual.push(message.id);
                }
                offset += page.messages.len() as i64;
                if !page.has_more {
                    break;
                }
            }
            let mut expected: Vec<i64> = (first..=last).collect();
            if sort == SortOrder::Newest {
                expected.reverse();
            }
            assert_eq!(actual, expected);
        }
    }
    let empty = list_legacy_messages_from_dir_filtered(
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Newest,
        None,
        None,
        Some("absent".into()),
    )
    .unwrap();
    assert!(empty.messages.is_empty());
    assert_eq!(empty.total_count, 0);
    assert!(!empty.has_more);
    let nullable =
        crate::legacy_query::read_legacy_message_by_id(&conn, &data_dir.join("images"), 1).unwrap();
    assert!(!nullable.archived);
    assert_eq!(nullable.images.len(), 2);
    drop(conn);
    fs::remove_dir_all(data_dir).unwrap();
}

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
        CREATE TABLE message_images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_filename TEXT NOT NULL
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

    let search_page = list_legacy_messages_from_dir_filtered(
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Newest,
        Some(0),
        Some(10),
        Some("older".to_string()),
    )
    .expect("search normal messages");

    assert_eq!(search_page.total_count, 1);
    assert!(!search_page.has_more);
    assert_eq!(search_page.messages[0].id, 1);

    fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
}

#[test]
fn search_like_wildcards_match_literals_only() {
    let data_dir = env::temp_dir().join(format!(
        "clipstash-next-legacy-like-escape-test-{}",
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
        CREATE TABLE message_images (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            message_id INTEGER NOT NULL,
            image_filename TEXT NOT NULL
        );
        INSERT INTO messages (id, text_content, created_at, archived) VALUES
            (1, 'discount 100% off', '2024-01-01 00:00:00', 0),
            (2, 'score is 100x better', '2024-02-01 00:00:00', 0),
            (3, 'has_underscore text', '2024-03-01 00:00:00', 0),
            (4, 'hasXunderscore text', '2024-04-01 00:00:00', 0);
        ",
    )
    .expect("seed fixture");
    drop(conn);

    let percent_page = list_legacy_messages_from_dir_filtered(
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Newest,
        Some(0),
        Some(10),
        Some("100%".to_string()),
    )
    .expect("search literal percent");

    assert_eq!(percent_page.total_count, 1);
    assert_eq!(percent_page.messages[0].id, 1);
    assert_eq!(
        percent_page.messages[0].text_content.as_deref(),
        Some("discount 100% off")
    );

    let underscore_page = list_legacy_messages_from_dir_filtered(
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Newest,
        Some(0),
        Some(10),
        Some("has_underscore".to_string()),
    )
    .expect("search literal underscore");

    assert_eq!(underscore_page.total_count, 1);
    assert_eq!(underscore_page.messages[0].id, 3);
    assert_eq!(
        underscore_page.messages[0].text_content.as_deref(),
        Some("has_underscore text")
    );

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

    let preview = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 1, false)
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

    let empty = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 2, false)
        .expect_err("empty message should fail preview");
    assert!(empty.contains("消息为空或图片文件缺失"));

    let out_of_range =
        copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir.clone(), 1, 3, false)
            .expect_err("out-of-range queue item should fail before writing clipboard");
    assert!(out_of_range.contains("索引超出范围"));

    let empty_copy =
        copy_legacy_message_import_queue_item_to_clipboard_from_dir(data_dir.clone(), 2, 0, false)
            .expect_err("empty message should fail before writing clipboard");
    assert!(empty_copy.contains("消息为空或图片文件缺失"));

    fs::remove_dir_all(data_dir).expect("remove sqlite fixture");
}

#[test]
fn matches_internal_blank_lines_to_existing_images_when_enabled() {
    let data_dir = env::temp_dir().join(format!(
        "clipstash-next-import-queue-blank-line-test-{}",
        process::id()
    ));
    let _ = fs::remove_dir_all(&data_dir);
    fs::create_dir_all(data_dir.join("images")).expect("create images dir");
    for filename in ["one.png", "two.png", "three.png"] {
        fs::write(data_dir.join("images").join(filename), tiny_png_bytes()).expect("seed image");
    }

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
            (1, 'first\n\nsecond\n\n\nthird', 0),
            (2, 'no blank line', 0),
            (3, 'one\n\ntwo', 0),
            (4, 'left\r\n  \r\nright', 0),
            (5, 'first\n\nsecond\n\nthird', 0),
            (6, 'first\n\n  \r\nthird', 0),
            (7, 'text only', 0),
            (8, NULL, 0);
        INSERT INTO message_images (id, message_id, image_filename) VALUES
            (10, 1, 'one.png'),
            (20, 1, 'two.png'),
            (30, 1, 'three.png'),
            (40, 2, 'one.png'),
            (50, 2, 'two.png'),
            (60, 3, 'one.png'),
            (70, 3, 'two.png'),
            (80, 3, 'three.png'),
            (90, 4, 'one.png'),
            (100, 5, 'one.png'),
            (110, 5, 'missing.png'),
            (120, 5, 'two.png'),
            (130, 6, 'one.png'),
            (140, 8, 'one.png');
        ",
    )
    .expect("seed fixture");
    drop(conn);

    let default_off = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 1, false)
        .expect("preview with matching disabled");
    assert_eq!(default_off.items.len(), 4);
    assert_eq!(
        default_off.items[0].text.as_deref(),
        Some("first\n\nsecond\n\n\nthird")
    );

    let matched = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 1, true)
        .expect("preview matching three images");
    assert_eq!(matched.items.len(), 7);
    assert_eq!(matched.items[0].text.as_deref(), Some("first\n"));
    assert_eq!(
        matched.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );
    assert_eq!(matched.items[2].text.as_deref(), Some("\nsecond\n"));
    assert_eq!(
        matched.items[3]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("two.png")
    );
    assert_eq!(matched.items[4].text.as_deref(), Some("\n"));
    assert_eq!(
        matched.items[5]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("three.png")
    );
    assert_eq!(matched.items[6].text.as_deref(), Some("\nthird"));

    let no_blank = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 2, true)
        .expect("preview without blank lines");
    assert_eq!(no_blank.items.len(), 3);
    assert_eq!(no_blank.items[0].text.as_deref(), Some("no blank line"));
    assert_eq!(
        no_blank.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );
    assert_eq!(
        no_blank.items[2]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("two.png")
    );

    let extra_images = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 3, true)
        .expect("preview with extra images");
    assert_eq!(extra_images.items.len(), 5);
    assert_eq!(extra_images.items[0].text.as_deref(), Some("one\n"));
    assert_eq!(
        extra_images.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );
    assert_eq!(extra_images.items[2].text.as_deref(), Some("\ntwo"));
    assert_eq!(
        extra_images.items[3]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("two.png")
    );
    assert_eq!(
        extra_images.items[4]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("three.png")
    );

    let whitespace_crlf = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 4, true)
        .expect("preview CRLF whitespace-only placeholder");
    assert_eq!(whitespace_crlf.items.len(), 3);
    assert_eq!(whitespace_crlf.items[0].text.as_deref(), Some("left\r\n"));
    assert_eq!(
        whitespace_crlf.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );
    assert_eq!(whitespace_crlf.items[2].text.as_deref(), Some("\r\nright"));

    let missing_does_not_consume_slot =
        preview_legacy_message_import_queue_from_dir(data_dir.clone(), 5, true)
            .expect("preview with missing image");
    assert_eq!(missing_does_not_consume_slot.skipped_missing_image_count, 1);
    assert_eq!(missing_does_not_consume_slot.items.len(), 5);
    assert_eq!(
        missing_does_not_consume_slot.items[0].text.as_deref(),
        Some("first\n")
    );
    assert_eq!(
        missing_does_not_consume_slot.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );
    assert_eq!(
        missing_does_not_consume_slot.items[2].text.as_deref(),
        Some("\nsecond\n")
    );
    assert_eq!(
        missing_does_not_consume_slot.items[3]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("two.png")
    );
    assert_eq!(
        missing_does_not_consume_slot.items[4].text.as_deref(),
        Some("\nthird")
    );

    let image_shortage = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 6, true)
        .expect("preview with fewer existing images than blank lines");
    assert_eq!(image_shortage.items.len(), 3);
    assert_eq!(image_shortage.items[0].text.as_deref(), Some("first\n"));
    assert_eq!(
        image_shortage.items[1]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );
    assert_eq!(
        image_shortage.items[2].text.as_deref(),
        Some("\n  \r\nthird")
    );

    let text_only = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 7, true)
        .expect("preview text-only message");
    assert_eq!(text_only.items.len(), 1);
    assert_eq!(text_only.items[0].text.as_deref(), Some("text only"));

    let image_only = preview_legacy_message_import_queue_from_dir(data_dir.clone(), 8, true)
        .expect("preview image-only message");
    assert_eq!(image_only.items.len(), 1);
    assert_eq!(
        image_only.items[0]
            .image
            .as_ref()
            .map(|image| image.filename.as_str()),
        Some("one.png")
    );

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
    verify_legacy_data_dir_readonly_consistency(data_dir);
}

#[test]
#[ignore = "writes regression fixture under clipstash-next/test-data"]
fn generates_regression_fixture() {
    let data_dir = regression_fixture_dir();
    let _ = fs::remove_dir_all(&data_dir);
    fs::create_dir_all(data_dir.join("images")).expect("create regression images dir");

    let db_path = data_dir.join("clipstash.db");
    let conn = Connection::open(&db_path).expect("create regression sqlite fixture");
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
        INSERT INTO messages (id, text_content, created_at, archived, archived_at) VALUES
            (1, 'pure text regression message', '2024-01-01 08:00:00', 0, NULL),
            (2, 'single image with text', '2024-01-02 08:00:00', 0, NULL),
            (3, 'four images ordered by image id', '2024-01-03 08:00:00', 0, NULL),
            (4, 'eighteen image stress message', '2024-01-04 08:00:00', 0, NULL),
            (5, 'archived regression message', '2024-01-05 08:00:00', 1, '2024-01-06 09:30:00'),
            (6, NULL, '2024-01-07 08:00:00', 0, NULL),
            (7, 'long text regression message: Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vestibulum clipboard workflow keeps this message intentionally verbose for layout checks.', '2024-01-08 08:00:00', 0, NULL),
            (8, 'missing image reference', '2024-01-09 08:00:00', 0, NULL);
        ",
    )
    .expect("seed regression messages");

    let mut image_id = 100;
    insert_image_row(&conn, &data_dir, &mut image_id, 2, "single.png", true);
    for index in 0..4 {
        insert_image_row(
            &conn,
            &data_dir,
            &mut image_id,
            3,
            &format!("multi-{}.png", index + 1),
            true,
        );
    }
    for index in 0..18 {
        insert_image_row(
            &conn,
            &data_dir,
            &mut image_id,
            4,
            &format!("stress-{:02}.png", index + 1),
            true,
        );
    }
    insert_image_row(&conn, &data_dir, &mut image_id, 5, "archived.png", true);
    insert_image_row(&conn, &data_dir, &mut image_id, 6, "pure-image.png", true);
    insert_image_row(
        &conn,
        &data_dir,
        &mut image_id,
        8,
        "missing-reference.png",
        false,
    );
    drop(conn);

    verify_legacy_data_dir_readonly_consistency(data_dir.clone());
    eprintln!("regression-fixture-ok data_dir={}", data_dir.display());
}

#[test]
#[ignore = "reads regression fixture under clipstash-next/test-data"]
fn verifies_regression_fixture_readonly_consistency() {
    verify_legacy_data_dir_readonly_consistency(regression_fixture_dir());
}

fn regression_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("src-tauri has parent")
        .join("test-data")
        .join("regression")
        .join("legacy")
}

fn insert_image_row(
    conn: &Connection,
    data_dir: &Path,
    image_id: &mut i64,
    message_id: i64,
    filename: &str,
    write_file: bool,
) {
    conn.execute(
        "INSERT INTO message_images (id, message_id, image_filename) VALUES (?1, ?2, ?3)",
        (&*image_id, &message_id, filename),
    )
    .expect("insert regression image row");
    if write_file {
        fs::write(data_dir.join("images").join(filename), tiny_png_bytes())
            .expect("write regression image");
    }
    *image_id += 1;
}

fn verify_legacy_data_dir_readonly_consistency(data_dir: PathBuf) {
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

    assert_message_order_matches_db_from_dir(
        &conn,
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Newest,
    );
    assert_message_order_matches_db_from_dir(
        &conn,
        data_dir.clone(),
        MessageView::Normal,
        SortOrder::Oldest,
    );
    assert_message_order_matches_db_from_dir(
        &conn,
        data_dir.clone(),
        MessageView::Archived,
        SortOrder::Newest,
    );
    assert_message_order_matches_db_from_dir(
        &conn,
        data_dir.clone(),
        MessageView::Archived,
        SortOrder::Oldest,
    );

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
