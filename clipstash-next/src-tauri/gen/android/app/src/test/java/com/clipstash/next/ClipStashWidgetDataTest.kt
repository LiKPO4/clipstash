package com.clipstash.next

import org.junit.Assert.assertEquals
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
}
