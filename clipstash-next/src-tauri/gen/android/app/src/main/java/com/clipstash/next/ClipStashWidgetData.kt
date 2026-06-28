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
        val updated = db.execArchiveMessage(messageId)
        updated > 0
      }
    } catch (_: Exception) {
      false
    }
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
    db.rawQuery(
      """
        SELECT m.id, m.text_content, COUNT(mi.id) AS image_count
        FROM messages m
        LEFT JOIN message_images mi ON mi.message_id = m.id
        WHERE m.archived = 0 OR m.archived IS NULL
        GROUP BY m.id
        ORDER BY m.created_at DESC, m.id DESC
      """.trimIndent(),
      emptyArray(),
    ).use { cursor ->
      while (cursor.moveToNext()) {
        val text = cursor.getString(1)?.trim().orEmpty()
        val imageCount = cursor.getInt(2)
        items.add(ClipStashWidgetItem(cursor.getLong(0), formatItem(text, imageCount)))
      }
    }
    return items
  }

  private fun SQLiteDatabase.execArchiveMessage(messageId: Long): Int {
    return compileStatement(
      """
        UPDATE messages
        SET archived = 1, archived_at = datetime('now')
        WHERE id = ? AND (archived = 0 OR archived IS NULL)
      """.trimIndent(),
    ).use { statement ->
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
