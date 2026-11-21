package com.clipstash.next

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import java.io.File
import org.json.JSONObject

data class ClipStashWidgetState(
  val count: Int,
  val items: List<ClipStashWidgetItem>,
  val status: ClipStashWidgetStatus = ClipStashWidgetStatus.Ready,
)

data class ClipStashWidgetItem(
  val id: Long,
  val text: String,
)

enum class ClipStashWidgetStatus {
  Ready,
  Empty,
  MissingDatabase,
  Error,
}

object ClipStashWidgetData {
  private const val APP_DATA_DIR_NAME = "ClipStash Next"
  private const val DATA_LOCATION_FILE_NAME = "data-location.json"
  private const val DB_NAME = "clipstash.db"
  private const val SETTINGS_FILE_NAME = "settings.json"
  // 小组件背景默认不透明；设置缺失、损坏或越界时都回退到这个值，保持旧版本行为。
  private const val DEFAULT_WIDGET_OPACITY = 100
  // 小组件每次刷新最多加载的未归档消息数：小组件列表实际只需展示少量行，
  // 200 条足以覆盖滚动查看需求，同时避免消息量大时全量查询拖慢每次刷新。
  private const val WIDGET_ITEM_LIMIT = 200

  fun load(context: Context): ClipStashWidgetState {
    val dbFile = resolveDatabaseFile(context)
    if (!dbFile.isFile) {
      return ClipStashWidgetState(
        count = 0,
        items = emptyList(),
        status = ClipStashWidgetStatus.MissingDatabase,
      )
    }

    return try {
      SQLiteDatabase.openDatabase(
        dbFile.absolutePath,
        null,
        SQLiteDatabase.OPEN_READONLY,
      ).use { db ->
        db.applyBusyTimeout()
        val count = queryNormalCount(db)
        if (count <= 0) {
          return ClipStashWidgetState(
            count = 0,
            items = emptyList(),
            status = ClipStashWidgetStatus.Empty,
          )
        }

        ClipStashWidgetState(
          count = count,
          items = queryLatestItems(db),
        )
      }
    } catch (_: Exception) {
      ClipStashWidgetState(
        count = 0,
        items = emptyList(),
        status = ClipStashWidgetStatus.Error,
      )
    }
  }

  fun archiveMessage(context: Context, messageId: Long): Boolean {
    if (messageId <= 0) return false
    val dbFile = resolveDatabaseFile(context)
    if (!dbFile.isFile) return false

    return try {
      SQLiteDatabase.openDatabase(
        dbFile.absolutePath,
        null,
        SQLiteDatabase.OPEN_READWRITE,
      ).use { db ->
        db.applyBusyTimeout()
        val updated = db.execArchiveMessage(messageId)
        updated > 0
      }
    } catch (_: Exception) {
      false
    }
  }

  // 小组件背景不透明度（0-100）。settings.json 与数据库在同一数据目录，
  // 直接按数据库文件解析出的目录读取，设置面板改动后由 refreshWidgets 触发重建生效。
  fun loadWidgetOpacity(context: Context): Int {
    val settingsFile = File(resolveDatabaseFile(context).parentFile, SETTINGS_FILE_NAME)
    if (!settingsFile.isFile) return DEFAULT_WIDGET_OPACITY
    return try {
      parseWidgetOpacity(settingsFile.readText())
    } catch (_: Exception) {
      DEFAULT_WIDGET_OPACITY
    }
  }

  internal fun parseWidgetOpacity(jsonText: String?): Int {
    if (jsonText.isNullOrBlank()) return DEFAULT_WIDGET_OPACITY
    val value = try {
      JSONObject(jsonText).optDouble("widget_opacity", DEFAULT_WIDGET_OPACITY.toDouble())
    } catch (_: Exception) {
      return DEFAULT_WIDGET_OPACITY
    }
    if (value.isNaN()) return DEFAULT_WIDGET_OPACITY
    return value.coerceIn(0.0, 100.0).toInt()
  }

  private fun resolveDatabaseFile(context: Context): File {
    val defaultDataDirs = defaultDataDirs(context)
    val configuredDataDirs = defaultDataDirs.mapNotNull { readConfiguredDataDir(File(it, DATA_LOCATION_FILE_NAME)) }
    val candidates = (configuredDataDirs + defaultDataDirs).distinctBy { it.absolutePath }
    return candidates
      .map { File(it, DB_NAME) }
      .firstOrNull { it.isFile }
      ?: File(defaultDataDirs.first(), DB_NAME)
  }

  private fun readConfiguredDataDir(locationFile: File): File? {
    if (!locationFile.isFile) return null
    return try {
      val dataDir = JSONObject(locationFile.readText()).optString("data_dir").trim()
      if (dataDir.isEmpty()) null else File(dataDir)
    } catch (_: Exception) {
      null
    }
  }

  private fun defaultDataDirs(context: Context): List<File> {
    return listOf(
      File(context.dataDir, APP_DATA_DIR_NAME),
      File(context.filesDir, APP_DATA_DIR_NAME),
      File(context.filesDir, "app_data/$APP_DATA_DIR_NAME"),
      File(context.noBackupFilesDir, "app_data/$APP_DATA_DIR_NAME"),
      File(context.dataDir, "app_data/$APP_DATA_DIR_NAME"),
    )
  }

  private fun queryNormalCount(db: SQLiteDatabase): Int {
    db.rawQuery(
      "SELECT COUNT(*) FROM messages WHERE archived = 0 OR archived IS NULL",
      emptyArray(),
    ).use { cursor ->
      return if (cursor.moveToFirst()) cursor.getInt(0) else 0
    }
  }

  private fun queryLatestItems(db: SQLiteDatabase): List<ClipStashWidgetItem> {
    val items = mutableListOf<ClipStashWidgetItem>()
    val sql = latestItemsSql(
      hasUrgentColumn = db.hasColumn("messages", "urgent"),
      hasPinnedColumn = db.hasColumn("messages", "pinned"),
    )
    db.rawQuery(sql, emptyArray()).use { cursor ->
      while (cursor.moveToNext()) {
        val text = cursor.getString(1)?.trim().orEmpty()
        val imageCount = cursor.getInt(2)
        items.add(ClipStashWidgetItem(cursor.getLong(0), formatItem(text, imageCount)))
      }
    }
    return items
  }

  internal fun latestItemsSql(hasUrgentColumn: Boolean, hasPinnedColumn: Boolean): String {
    val priorityPrefix = buildString {
      if (hasUrgentColumn) append("COALESCE(m.urgent, 0) DESC, ")
      if (hasPinnedColumn) append("COALESCE(m.pinned, 0) DESC, ")
    }
    return """
        SELECT m.id, m.text_content, COUNT(mi.id) AS image_count
        FROM messages m
        LEFT JOIN message_images mi ON mi.message_id = m.id
        WHERE m.archived = 0 OR m.archived IS NULL
        GROUP BY m.id
        ORDER BY ${priorityPrefix}m.created_at DESC, m.id DESC
        LIMIT $WIDGET_ITEM_LIMIT
      """.trimIndent()
  }

  internal fun archiveMessageSql(hasUrgentColumn: Boolean): String {
    return if (hasUrgentColumn) {
      """
        UPDATE messages
        SET archived = 1, archived_at = datetime('now'), urgent = 0
        WHERE id = ? AND (archived = 0 OR archived IS NULL)
      """.trimIndent()
    } else {
      """
        UPDATE messages
        SET archived = 1, archived_at = datetime('now')
        WHERE id = ? AND (archived = 0 OR archived IS NULL)
      """.trimIndent()
    }
  }

  private fun SQLiteDatabase.hasColumn(tableName: String, columnName: String): Boolean {
    rawQuery("PRAGMA table_info($tableName)", emptyArray()).use { cursor ->
      val nameIndex = cursor.getColumnIndex("name")
      while (cursor.moveToNext()) {
        if (nameIndex >= 0 && cursor.getString(nameIndex) == columnName) {
          return true
        }
      }
    }
    return false
  }

  // execSQL 不能执行带结果行的语句，busy_timeout 赋值会返回一行导致抛异常，
  // 这里改用 rawQuery 设置超时。
  private fun SQLiteDatabase.applyBusyTimeout() {
    rawQuery("PRAGMA busy_timeout=3000", emptyArray()).close()
  }

  private fun SQLiteDatabase.execArchiveMessage(messageId: Long): Int {
    val sql = archiveMessageSql(hasColumn("messages", "urgent"))
    return compileStatement(sql).use { statement ->
      statement.bindLong(1, messageId)
      statement.executeUpdateDelete()
    }
  }

  internal fun formatItem(text: String, imageCount: Int): String {
    val imageLabel = when {
      imageCount <= 0 -> ""
      imageCount == 1 -> "[图片]"
      else -> "[图片] ×$imageCount"
    }
    return when {
      text.isNotEmpty() && imageLabel.isNotEmpty() -> "$text $imageLabel"
      text.isNotEmpty() -> text
      imageLabel.isNotEmpty() -> imageLabel
      else -> "无文字内容"
    }
  }
}
