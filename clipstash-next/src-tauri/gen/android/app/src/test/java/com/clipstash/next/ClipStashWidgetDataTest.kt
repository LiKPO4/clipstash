package com.clipstash.next

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ClipStashWidgetDataTest {
  @Test
  fun formatsImageOnlyMessagesWithTheCorrectCount() {
    assertEquals("[图片]", ClipStashWidgetData.formatItem("", 1))
    assertEquals("[图片] ×4", ClipStashWidgetData.formatItem("", 4))
  }

  @Test
  fun appendsTheImageLabelToTextMessages() {
    assertEquals("需求内容 [图片]", ClipStashWidgetData.formatItem("需求内容", 1))
    assertEquals("需求内容 [图片] ×3", ClipStashWidgetData.formatItem("需求内容", 3))
  }

  @Test
  fun keepsMessagesWithoutImagesReadable() {
    assertEquals("纯文字", ClipStashWidgetData.formatItem("纯文字", 0))
    assertEquals("无文字内容", ClipStashWidgetData.formatItem("", 0))
  }

  @Test
  fun parsesWidgetOpacityFromSettings() {
    assertEquals(45, ClipStashWidgetData.parseWidgetOpacity("""{"widget_opacity": 45.5}"""))
    assertEquals(0, ClipStashWidgetData.parseWidgetOpacity("""{"widget_opacity": 0}"""))
  }

  @Test
  fun fallsBackToOpaqueWhenWidgetOpacityIsMissingOrBroken() {
    assertEquals(100, ClipStashWidgetData.parseWidgetOpacity(null))
    assertEquals(100, ClipStashWidgetData.parseWidgetOpacity(""))
    assertEquals(100, ClipStashWidgetData.parseWidgetOpacity("{ not valid json !"))
    assertEquals(100, ClipStashWidgetData.parseWidgetOpacity("""{"other": 1}"""))
  }

  @Test
  fun clampsWidgetOpacityIntoRange() {
    assertEquals(100, ClipStashWidgetData.parseWidgetOpacity("""{"widget_opacity": 150}"""))
    assertEquals(0, ClipStashWidgetData.parseWidgetOpacity("""{"widget_opacity": -20}"""))
    assertEquals(100, ClipStashWidgetData.parseWidgetOpacity("""{"widget_opacity": "NaN"}"""))
  }

  @Test
  fun clearsUrgentOnlyWhenSchemaHasUrgentColumn() {
    val withUrgent = ClipStashWidgetData.archiveMessageSql(hasUrgentColumn = true)
    assertTrue(withUrgent.contains("urgent = 0"))

    val withoutUrgent = ClipStashWidgetData.archiveMessageSql(hasUrgentColumn = false)
    assertFalse(withoutUrgent.contains("urgent"))
  }

  @Test
  fun ordersLatestItemsByUrgentAndPinnedWhenColumnsExist() {
    val full = ClipStashWidgetData.latestItemsSql(hasUrgentColumn = true, hasPinnedColumn = true)
    assertTrue(full.contains("ORDER BY COALESCE(m.urgent, 0) DESC, COALESCE(m.pinned, 0) DESC, m.created_at DESC, m.id DESC"))

    val legacy = ClipStashWidgetData.latestItemsSql(hasUrgentColumn = false, hasPinnedColumn = false)
    assertTrue(legacy.contains("ORDER BY m.created_at DESC, m.id DESC"))
  }
}
