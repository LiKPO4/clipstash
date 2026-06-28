package com.clipstash.next

import android.appwidget.AppWidgetManager
import android.content.Context
import android.content.Intent
import android.graphics.Paint
import android.widget.RemoteViews
import android.widget.RemoteViewsService

class ClipStashWidgetService : RemoteViewsService() {
  override fun onGetViewFactory(intent: Intent): RemoteViewsFactory {
    val appWidgetId = intent.getIntExtra(
      AppWidgetManager.EXTRA_APPWIDGET_ID,
      AppWidgetManager.INVALID_APPWIDGET_ID,
    )
    return ClipStashWidgetFactory(applicationContext, appWidgetId)
  }
}

private class ClipStashWidgetFactory(
  private val context: Context,
  private val appWidgetId: Int,
) : RemoteViewsService.RemoteViewsFactory {
  private var items: List<ClipStashWidgetItem> = emptyList()
  private var feedbackMessageId: Long? = null

  override fun onCreate() = Unit

  override fun onDataSetChanged() {
    val loadedItems = ClipStashWidgetData.load(context).items.toMutableList()
    val feedback = ClipStashWidgetProvider.getArchiveFeedback(appWidgetId)
    feedbackMessageId = feedback?.item?.id
    if (feedback != null && loadedItems.none { it.id == feedback.item.id }) {
      loadedItems.add(feedback.position.coerceIn(0, loadedItems.size), feedback.item)
    }
    items = loadedItems
  }

  override fun onDestroy() {
    items = emptyList()
  }

  override fun getCount(): Int = items.size

  override fun getViewAt(position: Int): RemoteViews? {
    val item = items.getOrNull(position) ?: return null
    val archivedFeedback = item.id == feedbackMessageId
    return RemoteViews(context.packageName, R.layout.widget_todo_row).apply {
      setTextViewText(R.id.widget_row_text, item.text)
      setImageViewResource(
        R.id.widget_row_done,
        if (archivedFeedback) R.drawable.widget_status_done else R.drawable.widget_status_circle,
      )
      setInt(
        R.id.widget_row_text,
        "setPaintFlags",
        if (archivedFeedback) Paint.STRIKE_THRU_TEXT_FLAG else 0,
      )
      setTextColor(
        R.id.widget_row_text,
        context.getColor(if (archivedFeedback) R.color.widget_circle else R.color.widget_text),
      )
      setOnClickFillInIntent(
        R.id.widget_row_root,
        itemIntent(ClipStashWidgetProvider.ITEM_ACTION_OPEN, item, position),
      )
      setOnClickFillInIntent(
        R.id.widget_row_done,
        itemIntent(ClipStashWidgetProvider.ITEM_ACTION_ARCHIVE, item, position),
      )
    }
  }

  override fun getLoadingView(): RemoteViews? = null

  override fun getViewTypeCount(): Int = 1

  override fun getItemId(position: Int): Long = items.getOrNull(position)?.id ?: position.toLong()

  override fun hasStableIds(): Boolean = true

  private fun itemIntent(action: String, item: ClipStashWidgetItem, position: Int): Intent {
    return Intent().apply {
      putExtra(ClipStashWidgetProvider.EXTRA_ITEM_ACTION, action)
      putExtra(ClipStashWidgetProvider.EXTRA_MESSAGE_ID, item.id)
      putExtra(ClipStashWidgetProvider.EXTRA_ROW_INDEX, position)
      putExtra(ClipStashWidgetProvider.EXTRA_MESSAGE_TEXT, item.text)
    }
  }
}
