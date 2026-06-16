package com.clipstash.next

import android.content.ActivityNotFoundException
import android.content.Intent
import android.os.Bundle
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.OnBackPressedCallback
import androidx.core.content.FileProvider
import java.io.File
import android.widget.Toast

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
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
    webView.addJavascriptInterface(ClipStashAndroidBridge(), "ClipStashAndroid")
  }

  inner class ClipStashAndroidBridge {
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
}
