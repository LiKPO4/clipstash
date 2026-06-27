package com.clipstash.next

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Paint
import android.os.Handler
import android.os.Looper
import android.view.View
import android.widget.Toast
import android.widget.RemoteViews

class ClipStashWidgetProvider : AppWidgetProvider() {
  override fun onReceive(context: Context, intent: Intent) {
    super.onReceive(context, intent)
    if (intent.action == ACTION_ARCHIVE_MESSAGE) {
      val appWidgetId = intent.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, AppWidgetManager.INVALID_APPWIDGET_ID)
      val rowIndex = intent.getIntExtra(EXTRA_ROW_INDEX, -1)
      val messageId = intent.getLongExtra(EXTRA_MESSAGE_ID, 0)
      if (appWidgetId != AppWidgetManager.INVALID_APPWIDGET_ID && rowIndex >= 0) {
        showArchiveFeedback(context, appWidgetId, rowIndex)
      }
      val archived = ClipStashWidgetData.archiveMessage(context, messageId)
      Toast.makeText(
        context,
        if (archived) "已归档" else "归档失败",
        Toast.LENGTH_SHORT,
      ).show()
      Handler(Looper.getMainLooper()).postDelayed({ refreshAll(context) }, 650)
    }
  }

  override fun onUpdate(
    context: Context,
    appWidgetManager: AppWidgetManager,
    appWidgetIds: IntArray,
  ) {
    appWidgetIds.forEach { appWidgetId ->
      updateWidget(context, appWidgetManager, appWidgetId)
    }
  }

  companion object {
    const val EXTRA_WIDGET_ACTION = "clipstash_widget_action"
    const val ACTION_CREATE = "create"
    const val ACTION_EXPORT = "export"
    private const val ACTION_OPEN = "open"
    private const val ACTION_ARCHIVE_MESSAGE = "com.clipstash.next.widget.ARCHIVE_MESSAGE"
    private const val EXTRA_MESSAGE_ID = "clipstash_widget_message_id"
    private const val EXTRA_ROW_INDEX = "clipstash_widget_row_index"
    private val rowDoneIds = intArrayOf(R.id.widget_done_1, R.id.widget_done_2, R.id.widget_done_3)
    private val rowTextIds = intArrayOf(R.id.widget_item_1, R.id.widget_item_2, R.id.widget_item_3)

    fun refreshAll(context: Context) {
      val appWidgetManager = AppWidgetManager.getInstance(context)
      val component = ComponentName(context, ClipStashWidgetProvider::class.java)
      val appWidgetIds = appWidgetManager.getAppWidgetIds(component)
      appWidgetIds.forEach { appWidgetId ->
        updateWidget(context, appWidgetManager, appWidgetId)
      }
    }

    fun updateWidget(
      context: Context,
      appWidgetManager: AppWidgetManager,
      appWidgetId: Int,
    ) {
      val state = ClipStashWidgetData.load(context)
      val views = RemoteViews(context.packageName, R.layout.widget_todo).apply {
        setTextViewText(R.id.widget_title, context.getString(R.string.widget_todo_title))
        setTextViewText(R.id.widget_count, state.count.toString())
        val showRows = state.status == ClipStashWidgetStatus.Ready && state.items.isNotEmpty()
        setViewVisibility(R.id.widget_rows, if (showRows) View.VISIBLE else View.GONE)
        setViewVisibility(R.id.widget_empty, if (showRows) View.GONE else View.VISIBLE)
        setTextViewText(
          R.id.widget_empty,
          context.getString(
            when (state.status) {
              ClipStashWidgetStatus.MissingDatabase -> R.string.widget_todo_initialize
              ClipStashWidgetStatus.Error -> R.string.widget_todo_unavailable
              else -> R.string.widget_todo_empty
            },
          ),
        )
        bindRow(context, appWidgetId, this, R.id.widget_row_1, R.id.widget_done_1, R.id.widget_item_1, state.items.getOrNull(0), 0)
        bindRow(context, appWidgetId, this, R.id.widget_row_2, R.id.widget_done_2, R.id.widget_item_2, state.items.getOrNull(1), 1)
        bindRow(context, appWidgetId, this, R.id.widget_row_3, R.id.widget_done_3, R.id.widget_item_3, state.items.getOrNull(2), 2)

        setOnClickPendingIntent(R.id.widget_root, openAppIntent(context, appWidgetId, ACTION_OPEN))
        setOnClickPendingIntent(R.id.widget_share, openAppIntent(context, appWidgetId, ACTION_EXPORT))
        setOnClickPendingIntent(R.id.widget_compose, openAppIntent(context, appWidgetId, ACTION_CREATE))
        setOnClickPendingIntent(R.id.widget_row_1, openAppIntent(context, appWidgetId, ACTION_OPEN))
        setOnClickPendingIntent(R.id.widget_row_2, openAppIntent(context, appWidgetId, ACTION_OPEN))
        setOnClickPendingIntent(R.id.widget_row_3, openAppIntent(context, appWidgetId, ACTION_OPEN))
      }

      appWidgetManager.updateAppWidget(appWidgetId, views)
    }

    private fun bindRow(
      context: Context,
      appWidgetId: Int,
      views: RemoteViews,
      rowId: Int,
      doneId: Int,
      textId: Int,
      item: ClipStashWidgetItem?,
      index: Int,
    ) {
      if (item == null) {
        views.setViewVisibility(rowId, View.GONE)
        return
      }
      views.setViewVisibility(rowId, View.VISIBLE)
      views.setTextViewText(textId, item.text)
      views.setImageViewResource(doneId, R.drawable.widget_status_circle)
      views.setInt(textId, "setPaintFlags", 0)
      views.setTextColor(textId, context.getColor(R.color.widget_text))
      if (item.id > 0) {
        views.setOnClickPendingIntent(doneId, archiveIntent(context, appWidgetId, item.id, index))
      }
    }

    private fun archiveIntent(
      context: Context,
      appWidgetId: Int,
      messageId: Long,
      index: Int,
    ): PendingIntent {
      val intent = Intent(context, ClipStashWidgetProvider::class.java).apply {
        action = ACTION_ARCHIVE_MESSAGE
        putExtra(EXTRA_MESSAGE_ID, messageId)
        putExtra(EXTRA_ROW_INDEX, index)
        putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
      }
      return PendingIntent.getBroadcast(
        context,
        appWidgetId * 100 + index,
        intent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
    }

    private fun showArchiveFeedback(context: Context, appWidgetId: Int, rowIndex: Int) {
      val doneId = rowDoneIds.getOrNull(rowIndex) ?: return
      val textId = rowTextIds.getOrNull(rowIndex) ?: return
      val views = RemoteViews(context.packageName, R.layout.widget_todo).apply {
        setImageViewResource(doneId, R.drawable.widget_status_done)
        setInt(textId, "setPaintFlags", Paint.STRIKE_THRU_TEXT_FLAG)
        setTextColor(textId, context.getColor(R.color.widget_circle))
      }
      AppWidgetManager.getInstance(context).partiallyUpdateAppWidget(appWidgetId, views)
    }

    private fun openAppIntent(context: Context, appWidgetId: Int, action: String): PendingIntent {
      val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)
        ?: Intent(context, MainActivity::class.java)
      intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
      intent.putExtra(EXTRA_WIDGET_ACTION, action)
      intent.putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)

      val actionCode = when (action) {
        ACTION_CREATE -> 1
        ACTION_EXPORT -> 2
        else -> 0
      }
      val requestCode = appWidgetId * 10 + actionCode
      return PendingIntent.getActivity(
        context,
        requestCode,
        intent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
    }
  }
}
