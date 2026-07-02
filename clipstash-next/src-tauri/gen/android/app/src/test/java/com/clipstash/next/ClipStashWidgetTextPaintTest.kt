package com.clipstash.next

import android.graphics.Paint
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class ClipStashWidgetTextPaintTest {
  @Test
  fun normalRowsKeepTextAntialiasing() {
    val flags = widgetTextPaintFlags(archived = false)

    assertTrue(flags and Paint.ANTI_ALIAS_FLAG != 0)
    assertTrue(flags and Paint.SUBPIXEL_TEXT_FLAG != 0)
    assertEquals(0, flags and Paint.STRIKE_THRU_TEXT_FLAG)
  }

  @Test
  fun archivedRowsAddStrikeThroughWithoutDroppingAntialiasing() {
    val flags = widgetTextPaintFlags(archived = true)

    assertTrue(flags and Paint.ANTI_ALIAS_FLAG != 0)
    assertTrue(flags and Paint.SUBPIXEL_TEXT_FLAG != 0)
    assertTrue(flags and Paint.STRIKE_THRU_TEXT_FLAG != 0)
  }
}
