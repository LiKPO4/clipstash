package com.clipstash.next

import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Base64
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.OnBackPressedCallback
import androidx.core.content.FileProvider
import java.io.File
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null
  private var pendingShareJson: String? = null
  private var pendingWidgetAction: String? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    captureSharedIntent(intent)
    captureWidgetAction(intent)
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        val webView = appWebView
        if (webView == null) {
          isEnabled = false
          onBackPressedDispatcher.onBackPressed()
          isEnabled = true
          return
        }

        webView.evaluateJavascript(
          """
            (() => {
              const event = new CustomEvent('clipstash-android-back', { cancelable: true });
              window.dispatchEvent(event);
              return event.defaultPrevented;
            })()
          """.trimIndent()
        ) { result ->
          if (result != "true") {
            isEnabled = false
            onBackPressedDispatcher.onBackPressed()
            isEnabled = true
          }
        }
      }
    })
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    appWebView = webView
    webView.settings.apply {
      setSupportZoom(false)
      builtInZoomControls = false
      displayZoomControls = false
      textZoom = 100
    }
    webView.addJavascriptInterface(ClipStashAndroidBridge(), "ClipStashAndroid")
    notifyWidgetActionAvailable()
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    if (captureWidgetAction(intent)) {
      notifyWidgetActionAvailable()
    }
    if (captureSharedIntent(intent)) {
      notifyShareAvailable()
    }
  }

  inner class ClipStashAndroidBridge {
    @JavascriptInterface
    fun consumePendingShare(): String {
      val payload = pendingShareJson ?: return ""
      pendingShareJson = null
      return payload
    }

    @JavascriptInterface
    fun consumePendingWidgetAction(): String {
      val action = pendingWidgetAction ?: return ""
      pendingWidgetAction = null
      return action
    }

    @JavascriptInterface
    fun refreshWidgets() {
      runOnUiThread {
        ClipStashWidgetProvider.refreshAll(this@MainActivity)
      }
    }

    @JavascriptInterface
    fun shareZip(path: String) {
      runOnUiThread {
        val file = File(path)
        if (!file.exists()) {
          Toast.makeText(this@MainActivity, "导出的 zip 不存在", Toast.LENGTH_SHORT).show()
          return@runOnUiThread
        }

        val uri = FileProvider.getUriForFile(
          this@MainActivity,
          "${applicationContext.packageName}.fileprovider",
          file,
        )
        val intent = Intent(Intent.ACTION_SEND).apply {
          type = "application/zip"
          putExtra(Intent.EXTRA_STREAM, uri)
          addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        try {
          startActivity(Intent.createChooser(intent, "分享 ClipStash 数据包"))
        } catch (_: ActivityNotFoundException) {
          Toast.makeText(this@MainActivity, "没有可用的分享应用", Toast.LENGTH_SHORT).show()
        }
      }
    }
  }

  private fun notifyShareAvailable() {
    appWebView?.post {
      appWebView?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('clipstash-android-share-ready'))",
        null,
      )
    }
  }

  private fun notifyWidgetActionAvailable() {
    appWebView?.post {
      appWebView?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('clipstash-android-widget-action-ready'))",
        null,
      )
    }
  }

  private fun captureWidgetAction(intent: Intent?): Boolean {
    val action = intent?.getStringExtra(ClipStashWidgetProvider.EXTRA_WIDGET_ACTION) ?: return false
    if (action != ClipStashWidgetProvider.ACTION_CREATE && action != ClipStashWidgetProvider.ACTION_EXPORT) return false
    pendingWidgetAction = action
    intent.removeExtra(ClipStashWidgetProvider.EXTRA_WIDGET_ACTION)
    Toast.makeText(
      this,
      if (action == ClipStashWidgetProvider.ACTION_CREATE) "正在新建需求" else "正在准备分享",
      Toast.LENGTH_SHORT,
    ).show()
    return true
  }

  private fun captureSharedIntent(intent: Intent?): Boolean {
    if (intent == null) return false
    val action = intent.action ?: return false
    if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) return false

    val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.trim().orEmpty()
    val images = readSharedImages(intent)
    if (text.isEmpty() && images.length() == 0) return false

    pendingShareJson = JSONObject()
      .put("text", text)
      .put("images", images)
      .toString()
    return true
  }

  private fun readSharedImages(intent: Intent): JSONArray {
    val images = JSONArray()
    for (uri in sharedImageUris(intent)) {
      val mimeType = contentResolver.getType(uri) ?: intent.type.orEmpty()
      if (!mimeType.startsWith("image/")) continue

      val bytes = contentResolver.openInputStream(uri)?.use { stream ->
        stream.readBytes()
      } ?: continue
      if (bytes.isEmpty()) continue

      images.put(
        JSONObject()
          .put("mimeType", mimeType)
          .put("data", Base64.encodeToString(bytes, Base64.NO_WRAP)),
      )
    }
    return images
  }

  @Suppress("DEPRECATION")
  private fun sharedImageUris(intent: Intent): List<Uri> {
    if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
      return intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty()
    }
    return intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.let { listOf(it) }.orEmpty()
  }
}
