package com.clipstash.next

import android.appwidget.AppWidgetManager
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/**
 * 小组件自定义动作（点击打开 / 归档）接收器。
 *
 * 与导出的 [ClipStashWidgetProvider] 隔离：本接收器声明为
 * android:exported="false"，只接收应用自身 PendingIntent 发出的显式广播，
 * 外部应用无法伪造 ITEM_CLICK/ARCHIVE 广播静默操作消息。
 */
class ClipStashWidgetActionsReceiver : BroadcastReceiver() {
  override fun onReceive(context: Context, intent: Intent) {
    if (intent.action != ClipStashWidgetProvider.ACTION_ITEM_CLICK) return
    val appWidgetId = intent.getIntExtra(
      AppWidgetManager.EXTRA_APPWIDGET_ID,
      AppWidgetManager.INVALID_APPWIDGET_ID,
    )
    ClipStashWidgetProvider.handleItemClick(context, intent, appWidgetId)
  }
}
