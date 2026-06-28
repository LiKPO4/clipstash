package com.clipstash.next

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.view.View
import android.widget.RemoteViews
import android.widget.Toast

class ClipStashWidgetProvider : AppWidgetProvider() {
  override fun onReceive(context: Context, intent: Intent) {
    super.onReceive(context, intent)
    if (intent.action != ACTION_ITEM_CLICK) return

    val appWidgetId = intent.getIntExtra(
      AppWidgetManager.EXTRA_APPWIDGET_ID,
      AppWidgetManager.INVALID_APPWIDGET_ID,
    )
    when (intent.getStringExtra(EXTRA_ITEM_ACTION)) {
      ITEM_ACTION_OPEN -> openApp(context, appWidgetId)
      ITEM_ACTION_ARCHIVE -> archiveMessage(context, intent, appWidgetId)
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
    const val ACTION_ITEM_CLICK = "com.clipstash.next.widget.ITEM_CLICK"
    const val EXTRA_ITEM_ACTION = "clipstash_widget_item_action"
    const val EXTRA_MESSAGE_ID = "clipstash_widget_message_id"
    const val EXTRA_ROW_INDEX = "clipstash_widget_row_index"
    const val EXTRA_MESSAGE_TEXT = "clipstash_widget_message_text"
    const val ITEM_ACTION_OPEN = "open"
    const val ITEM_ACTION_ARCHIVE = "archive"
    private const val ACTION_OPEN = "open"
    private const val FEEDBACK_DURATION_MS = 650L
    private val archiveFeedbacks = mutableMapOf<Int, ArchiveFeedback>()

    fun refreshAll(context: Context) {
      val appWidgetManager = AppWidgetManager.getInstance(context)
      val component = ComponentName(context, ClipStashWidgetProvider::class.java)
      appWidgetManager.getAppWidgetIds(component).forEach { appWidgetId ->
        updateWidget(context, appWidgetManager, appWidgetId)
      }
    }

    fun updateWidget(
      context: Context,
      appWidgetManager: AppWidgetManager,
      appWidgetId: Int,
    ) {
      val state = ClipStashWidgetData.load(context)
      val serviceIntent = Intent(context, ClipStashWidgetService::class.java).apply {
        putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
        data = Uri.parse("clipstash://widget/$appWidgetId")
      }
      val views = RemoteViews(context.packageName, R.layout.widget_todo).apply {
        setTextViewText(R.id.widget_title, context.getString(R.string.widget_todo_title))
        setTextViewText(R.id.widget_count, state.count.toString())
        val showRows = state.status == ClipStashWidgetStatus.Ready && state.items.isNotEmpty()
        setViewVisibility(R.id.widget_list, if (showRows) View.VISIBLE else View.GONE)
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
        setRemoteAdapter(R.id.widget_list, serviceIntent)
        setEmptyView(R.id.widget_list, R.id.widget_empty)
        setPendingIntentTemplate(R.id.widget_list, itemClickTemplate(context, appWidgetId))
        setOnClickPendingIntent(R.id.widget_share, openAppIntent(context, appWidgetId, ACTION_EXPORT))
        setOnClickPendingIntent(R.id.widget_compose, openAppIntent(context, appWidgetId, ACTION_CREATE))
      }

      appWidgetManager.updateAppWidget(appWidgetId, views)
      appWidgetManager.notifyAppWidgetViewDataChanged(appWidgetId, R.id.widget_list)
    }

    fun getArchiveFeedback(appWidgetId: Int): ArchiveFeedback? = synchronized(archiveFeedbacks) {
      val feedback = archiveFeedbacks[appWidgetId] ?: return@synchronized null
      if (feedback.expiresAt > SystemClock.elapsedRealtime()) {
        feedback
      } else {
        archiveFeedbacks.remove(appWidgetId)
        null
      }
    }

    private fun archiveMessage(context: Context, intent: Intent, appWidgetId: Int) {
      val messageId = intent.getLongExtra(EXTRA_MESSAGE_ID, 0)
      val rowIndex = intent.getIntExtra(EXTRA_ROW_INDEX, 0)
      val messageText = intent.getStringExtra(EXTRA_MESSAGE_TEXT).orEmpty()
      if (appWidgetId == AppWidgetManager.INVALID_APPWIDGET_ID || messageId <= 0) return

      synchronized(archiveFeedbacks) {
        archiveFeedbacks[appWidgetId] = ArchiveFeedback(
          item = ClipStashWidgetItem(messageId, messageText),
          position = rowIndex,
          expiresAt = SystemClock.elapsedRealtime() + FEEDBACK_DURATION_MS,
        )
      }
      val manager = AppWidgetManager.getInstance(context)
      manager.notifyAppWidgetViewDataChanged(appWidgetId, R.id.widget_list)

      val archived = ClipStashWidgetData.archiveMessage(context, messageId)
      Toast.makeText(context, if (archived) "已归档" else "归档失败", Toast.LENGTH_SHORT).show()
      if (!archived) {
        synchronized(archiveFeedbacks) { archiveFeedbacks.remove(appWidgetId) }
        refreshAll(context)
        return
      }

      Handler(Looper.getMainLooper()).postDelayed({
        synchronized(archiveFeedbacks) { archiveFeedbacks.remove(appWidgetId) }
        refreshAll(context)
      }, FEEDBACK_DURATION_MS)
    }

    private fun openApp(context: Context, appWidgetId: Int) {
      val intent = context.packageManager.getLaunchIntentForPackage(context.packageName)
        ?: Intent(context, MainActivity::class.java)
      intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP)
      intent.putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
      context.startActivity(intent)
    }

    private fun itemClickTemplate(context: Context, appWidgetId: Int): PendingIntent {
      val intent = Intent(context, ClipStashWidgetProvider::class.java).apply {
        action = ACTION_ITEM_CLICK
        putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, appWidgetId)
      }
      return PendingIntent.getBroadcast(
        context,
        appWidgetId,
        intent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_MUTABLE,
      )
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
      return PendingIntent.getActivity(
        context,
        appWidgetId * 10 + actionCode,
        intent,
        PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
      )
    }
  }
}

data class ArchiveFeedback(
  val item: ClipStashWidgetItem,
  val position: Int,
  val expiresAt: Long,
)
