package com.clipstash.next

import android.content.ActivityNotFoundException
import android.app.DownloadManager
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import android.util.Base64
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.OnBackPressedCallback
import androidx.core.content.FileProvider
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject

class MainActivity : TauriActivity() {
  private var appWebView: WebView? = null
  private var pendingShareJson: String? = null
  private var pendingWidgetAction: String? = null
  private var pendingUpdateApk: File? = null

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

  override fun onResume() {
    super.onResume()
    val apk = pendingUpdateApk ?: return
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O || packageManager.canRequestPackageInstalls()) {
      pendingUpdateApk = null
      openApkInstaller(apk)
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

    @JavascriptInterface
    fun checkForUpdates(): Boolean {
      return try {
        Thread {
          try {
            val connection = URL(GITHUB_RELEASE_API_URL).openConnection() as HttpURLConnection
            connection.requestMethod = "GET"
            connection.connectTimeout = 15_000
            connection.readTimeout = 20_000
            connection.setRequestProperty("Accept", "application/vnd.github+json")
            connection.setRequestProperty("User-Agent", "ClipStash-Next-Android-Updater")
            try {
              val statusCode = connection.responseCode
              if (statusCode !in 200..299) {
                throw IllegalStateException("GitHub Release 检查失败：HTTP $statusCode")
              }
              val release = connection.inputStream.bufferedReader().use { reader ->
                JSONObject(reader.readText())
              }
              notifyAndroidUpdate("checked", "检查完成", release)
            } finally {
              connection.disconnect()
            }
          } catch (err: Exception) {
            notifyAndroidUpdate("error", err.message ?: "Android 更新检查失败")
          }
        }.start()
        true
      } catch (err: Exception) {
        notifyAndroidUpdate("error", err.message ?: "无法启动 Android 更新检查")
        false
      }
    }

    @JavascriptInterface
    fun downloadAndInstallApk(downloadUrl: String, filename: String): Boolean {
      return try {
        val safeFilename = validateUpdateDownload(downloadUrl, filename)
        val downloadDir = File(
          getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS),
          "updates",
        ).apply { mkdirs() }
        val apk = File(downloadDir, safeFilename)
        if (apk.exists() && !apk.delete()) {
          throw IllegalStateException("无法覆盖旧的更新安装包")
        }

        val request = DownloadManager.Request(Uri.parse(downloadUrl)).apply {
          setTitle("ClipStash Next 更新")
          setDescription("正在下载 $safeFilename")
          setMimeType(APK_MIME_TYPE)
          setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
          setDestinationInExternalFilesDir(
            this@MainActivity,
            Environment.DIRECTORY_DOWNLOADS,
            "updates/$safeFilename",
          )
        }
        val manager = getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
        val downloadId = manager.enqueue(request)
        notifyAndroidUpdate("downloading", "正在下载更新安装包")
        watchUpdateDownload(manager, downloadId, apk)
        true
      } catch (err: Exception) {
        notifyAndroidUpdate("error", err.message ?: "启动更新下载失败")
        false
      }
    }
  }

  private fun validateUpdateDownload(downloadUrl: String, filename: String): String {
    val uri = Uri.parse(downloadUrl)
    if (
      uri.scheme != "https" ||
      uri.host != "github.com" ||
      !uri.path.orEmpty().startsWith("/LiKPO4/clipstash/releases/download/")
    ) {
      throw IllegalArgumentException("更新下载链接不是 ClipStash 官方 Release 地址")
    }
    val safeFilename = filename.trim()
    if (
      safeFilename.isEmpty() ||
      !safeFilename.endsWith(".apk", ignoreCase = true) ||
      safeFilename.any { it in "<>:\"/\\|?*" }
    ) {
      throw IllegalArgumentException("更新资产不是有效的 Android APK")
    }
    return safeFilename
  }

  private fun watchUpdateDownload(manager: DownloadManager, downloadId: Long, apk: File) {
    Thread {
      while (true) {
        Thread.sleep(500)
        val query = DownloadManager.Query().setFilterById(downloadId)
        manager.query(query)?.use { cursor ->
          if (!cursor.moveToFirst()) return@use
          val status = cursor.getInt(cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS))
          when (status) {
            DownloadManager.STATUS_SUCCESSFUL -> {
              runOnUiThread {
                notifyAndroidUpdate("installing", "下载完成，正在打开系统安装界面")
                installDownloadedApk(apk)
              }
              return@Thread
            }
            DownloadManager.STATUS_FAILED -> {
              val reason = cursor.getInt(cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_REASON))
              notifyAndroidUpdate("error", "更新安装包下载失败（$reason）")
              return@Thread
            }
          }
        }
      }
    }.start()
  }

  private fun installDownloadedApk(apk: File) {
    if (!apk.isFile || apk.length() <= 0) {
      notifyAndroidUpdate("error", "下载的更新安装包不可用")
      return
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !packageManager.canRequestPackageInstalls()) {
      pendingUpdateApk = apk
      notifyAndroidUpdate("permission", "请允许 ClipStash 安装未知应用")
      startActivity(
        Intent(
          Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
          Uri.parse("package:$packageName"),
        ),
      )
      return
    }
    openApkInstaller(apk)
  }

  private fun openApkInstaller(apk: File) {
    val uri = FileProvider.getUriForFile(
      this,
      "$packageName.fileprovider",
      apk,
    )
    val intent = Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(uri, APK_MIME_TYPE)
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    try {
      startActivity(intent)
    } catch (_: ActivityNotFoundException) {
      notifyAndroidUpdate("error", "系统中没有可用的 APK 安装器")
    }
  }

  private fun notifyAndroidUpdate(status: String, message: String, release: JSONObject? = null) {
    val payload = JSONObject().put("status", status).put("message", message).apply {
      if (release != null) put("release", release)
    }.toString()
    appWebView?.post {
      appWebView?.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('clipstash-android-update', { detail: $payload }))",
        null,
      )
    }
    runOnUiThread {
      Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
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

  companion object {
    private const val APK_MIME_TYPE = "application/vnd.android.package-archive"
    private const val GITHUB_RELEASE_API_URL =
      "https://api.github.com/repos/LiKPO4/clipstash/releases/latest"
  }
}
