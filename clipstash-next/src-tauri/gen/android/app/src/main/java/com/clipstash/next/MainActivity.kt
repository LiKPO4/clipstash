package com.clipstash.next

import android.content.ActivityNotFoundException
import android.app.DownloadManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.activity.OnBackPressedCallback
import androidx.core.content.FileProvider
import java.io.File
import java.lang.ref.WeakReference
import java.util.UUID
import java.net.HttpURLConnection
import java.net.URL
import android.widget.Toast
import org.json.JSONArray
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

class MainActivity : TauriActivity() {
  @Volatile private var appWebView: WebView? = null
  // 分享载荷队列：连续多次分享不会互相覆盖（前端每次消费一条，直到队列为空）
  private var pendingWidgetAction: String? = null
  private var pendingUpdateJson: String? = null
  private var pendingUpdateApk: File? = null
  // 本次下载流程是否已拉起过「安装未知应用」授权页，避免用户拒绝后 onResume 死循环
  private var installPermissionPromptShown = false
  // 当前正在监控的下载任务 id，用于幂等恢复与防重复监控
  private var activeUpdateDownloadId: Long? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    shareActivity = WeakReference(this)
    val shareRoot = File(applicationContext.cacheDir, "clipstash-shares")
    shareExecutor.execute { runCatching { ShareFileIO.removeStalePackets(shareRoot) } }
    captureSharedIntent(intent)
    captureWidgetAction(intent)
    restorePendingUpdateDownload()
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

  override fun onDestroy() {
    appWebView = null
    super.onDestroy()
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
    restorePendingUpdateDownload()
    val apk = pendingUpdateApk ?: return
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O || packageManager.canRequestPackageInstalls()) {
      pendingUpdateApk = null
      installPermissionPromptShown = false
      openApkInstaller(apk)
      return
    }
    if (installPermissionPromptShown) {
      // 本次下载流程已提示过且用户未授权：只提示结果，不再自动拉起设置页，避免死循环
      notifyAndroidUpdate("permission", "未获得安装权限，请在系统设置中允许「安装未知应用」后再试")
      return
    }
    installPermissionPromptShown = true
    notifyAndroidUpdate("permission", "请允许 ClipStash 安装未知应用")
    startActivity(
      Intent(
        Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
        Uri.parse("package:$packageName"),
      ),
    )
  }

  inner class ClipStashAndroidBridge {
    @JavascriptInterface
    fun consumePendingShare(): String = synchronized(pendingShareQueue) {
      if (pendingShareQueue.isEmpty()) return@synchronized ""
      pendingShareQueue.removeFirst()
    }

    @JavascriptInterface
    fun consumePendingWidgetAction(): String {
      val action = pendingWidgetAction ?: return ""
      pendingWidgetAction = null
      return action
    }

    @JavascriptInterface
    fun consumePendingUpdate(): String = synchronized(this@MainActivity) {
      val payload = pendingUpdateJson ?: return@synchronized ""
      pendingUpdateJson = null
      payload
    }

    @JavascriptInterface
    fun refreshWidgets() {
      runOnUiThread {
        ClipStashWidgetProvider.refreshAll(this@MainActivity)
      }
    }

    @JavascriptInterface
    fun copyText(text: String): String {
      val value = text.trim()
      if (value.isEmpty()) return "empty"

      val latch = CountDownLatch(1)
      var result = "ok"
      runOnUiThread {
        try {
          val clipboard = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
          clipboard.setPrimaryClip(ClipData.newPlainText("ClipStash", value))
          result = "ok"
        } catch (err: Exception) {
          result = "error:${err.message ?: "写入系统剪贴板失败"}"
          Toast.makeText(
            this@MainActivity,
            err.message ?: "写入系统剪贴板失败",
            Toast.LENGTH_SHORT,
          ).show()
        } finally {
          latch.countDown()
        }
      }
      return if (latch.await(3, TimeUnit.SECONDS)) result else "error:timeout"
    }

    @JavascriptInterface
    fun shareZip(path: String) {
      shareExecutor.execute {
        try {
          val file = File(path)
          require(file.isFile) { "导出的 zip 不存在" }
          val uri = FileProvider.getUriForFile(this@MainActivity, "${applicationContext.packageName}.fileprovider", file)
          val intent = Intent(Intent.ACTION_SEND).apply {
            type = "application/zip"
            putExtra(Intent.EXTRA_STREAM, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
          }
          runOnUiThread {
            try { startActivity(Intent.createChooser(intent, "分享 ClipStash 数据包")) }
            catch (_: ActivityNotFoundException) {
              Toast.makeText(this@MainActivity, "没有可用的分享应用", Toast.LENGTH_SHORT).show()
            }
          }
        } catch (error: Exception) {
          runOnUiThread { Toast.makeText(this@MainActivity, error.message ?: "分享数据包失败", Toast.LENGTH_SHORT).show() }
        }
      }
    }

    @JavascriptInterface
    fun checkForUpdates(): Boolean {
      return try {
        val executor = Executors.newSingleThreadExecutor()
        executor.execute {
          var connection: HttpURLConnection? = null
          try {
            connection = URL(GITHUB_RELEASE_API_URL).openConnection() as HttpURLConnection
            connection.requestMethod = "GET"
            connection.connectTimeout = 15_000
            connection.readTimeout = 20_000
            connection.setRequestProperty("Accept", "application/vnd.github+json")
            connection.setRequestProperty("User-Agent", "ClipStash-Next-Android-Updater")
            val statusCode = connection.responseCode
            if (statusCode !in 200..299) {
              throw IllegalStateException("GitHub Release 检查失败：HTTP $statusCode")
            }
            val release = connection.inputStream.bufferedReader().use { reader ->
              JSONObject(reader.readText())
            }
            notifyAndroidUpdate("checked", "检查完成", release)
          } catch (err: Exception) {
            notifyAndroidUpdate("error", err.message ?: "Android 更新检查失败")
          } finally {
            connection?.disconnect()
          }
        }
        executor.shutdown()
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
        installPermissionPromptShown = false
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
    if (activeUpdateDownloadId == downloadId) return
    activeUpdateDownloadId = downloadId
    // 持久化下载任务，进程被杀后 onCreate/onResume 可恢复监控
    savePendingUpdateDownload(downloadId, apk)
    Thread {
      try {
        while (true) {
          if (activeUpdateDownloadId != downloadId) return@Thread
          Thread.sleep(500)
          val query = DownloadManager.Query().setFilterById(downloadId)
          manager.query(query)?.use { cursor ->
            if (!cursor.moveToFirst()) {
              // 下载任务已不存在（用户取消或系统清理），按终止状态处理，避免空转
              if (activeUpdateDownloadId != downloadId) return@Thread
              clearPendingUpdateDownload()
              notifyAndroidUpdate("error", "更新下载任务已失效，请重新下载")
              return@Thread
            }
            val status = cursor.getInt(cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS))
            when (status) {
              DownloadManager.STATUS_SUCCESSFUL -> {
                // 先清持久化状态再安装：进程在此后被杀死时，由下载完成通知兜底安装
                clearPendingUpdateDownload()
                runOnUiThread {
                  notifyAndroidUpdate("installing", "下载完成，正在打开系统安装界面")
                  installDownloadedApk(apk)
                }
                return@Thread
              }
              DownloadManager.STATUS_FAILED -> {
                val reason = cursor.getInt(cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_REASON))
                if (activeUpdateDownloadId != downloadId) return@Thread
                clearPendingUpdateDownload()
                notifyAndroidUpdate("error", "更新安装包下载失败（$reason）")
                return@Thread
              }
              // 其余状态（RUNNING/PAUSED/PENDING）继续轮询
              else -> Unit
            }
          }
        }
      } finally {
        if (activeUpdateDownloadId == downloadId) {
          activeUpdateDownloadId = null
        }
      }
    }.start()
  }

  private fun updatePrefs(): SharedPreferences =
    getSharedPreferences(PREFS_UPDATE, Context.MODE_PRIVATE)

  private fun savePendingUpdateDownload(downloadId: Long, apk: File) {
    updatePrefs().edit()
      .putLong(KEY_DOWNLOAD_ID, downloadId)
      .putString(KEY_DOWNLOAD_APK_PATH, apk.absolutePath)
      .apply()
  }

  private fun clearPendingUpdateDownload() {
    updatePrefs().edit()
      .remove(KEY_DOWNLOAD_ID)
      .remove(KEY_DOWNLOAD_APK_PATH)
      .apply()
  }

  /** 进程被杀后恢复未完成的下载监控（onCreate/onResume 调用，幂等）。 */
  private fun restorePendingUpdateDownload() {
    val prefs = updatePrefs()
    val downloadId = prefs.getLong(KEY_DOWNLOAD_ID, 0)
    val apkPath = prefs.getString(KEY_DOWNLOAD_APK_PATH, null) ?: return
    if (downloadId <= 0 || apkPath.isEmpty()) return
    val manager = getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
    watchUpdateDownload(manager, downloadId, File(apkPath))
  }

  private fun installDownloadedApk(apk: File) {
    if (!apk.isFile || apk.length() <= 0) {
      notifyAndroidUpdate("error", "下载的更新安装包不可用")
      return
    }
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O && !packageManager.canRequestPackageInstalls()) {
      pendingUpdateApk = apk
      installPermissionPromptShown = true
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
    synchronized(this) {
      pendingUpdateJson = payload
    }
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
    val isEditAction = action.startsWith(ClipStashWidgetProvider.ACTION_EDIT_PREFIX) &&
      action.removePrefix(ClipStashWidgetProvider.ACTION_EDIT_PREFIX).toLongOrNull()?.let { it > 0 } == true
    if (
      action != ClipStashWidgetProvider.ACTION_CREATE &&
      action != ClipStashWidgetProvider.ACTION_EXPORT &&
      !isEditAction
    ) return false
    pendingWidgetAction = action
    intent.removeExtra(ClipStashWidgetProvider.EXTRA_WIDGET_ACTION)
    Toast.makeText(
      this,
      when (action) {
        ClipStashWidgetProvider.ACTION_CREATE -> "正在新建需求"
        ClipStashWidgetProvider.ACTION_EXPORT -> "正在准备分享"
        else -> "正在编辑需求"
      },
      Toast.LENGTH_SHORT,
    ).show()
    return true
  }

  private fun captureSharedIntent(intent: Intent?): Boolean {
    if (intent == null) return false
    val action = intent.action ?: return false
    if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) return false
    val text = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()?.trim().orEmpty()
    val uris = sharedImageUris(intent)
    if (text.isEmpty() && uris.isEmpty()) return false
    val fallbackMime = intent.type.orEmpty()
    val resolver = applicationContext.contentResolver
    val root = File(applicationContext.cacheDir, "clipstash-shares")
    // Consume the intent now so Activity recreation cannot enqueue this share again.
    intent.action = null
    intent.removeExtra(Intent.EXTRA_STREAM)
    intent.removeExtra(Intent.EXTRA_TEXT)
    shareExecutor.execute {
      val shareId = UUID.randomUUID().toString()
      val packet = File(root, shareId)
      val payload = try {
        check(packet.mkdirs()) { "创建分享暂存目录失败" }
        val images = JSONArray()
        var total = 0L
        for (uri in uris) {
          val mime = resolver.getType(uri) ?: fallbackMime
          if (!mime.startsWith("image/")) continue
          val filename = "${images.length()}.bin"
          val input = resolver.openInputStream(uri) ?: error("无法读取分享图片")
          total += input.use { ShareFileIO.copy(it, File(packet, filename), minOf(ShareFileIO.MAX_IMAGE_BYTES, ShareFileIO.MAX_PACKET_BYTES - total)) }
          images.put(filename)
        }
        require(text.isNotEmpty() || images.length() > 0) { "分享内容为空" }
        File(packet, "manifest.json").writeText(JSONObject().put("text", text).put("images", images).toString())
        JSONObject().put("shareId", shareId).toString()
      } catch (error: Exception) {
        packet.deleteRecursively()
        JSONObject().put("error", error.message ?: "接收分享失败").toString()
      }
      synchronized(pendingShareQueue) { pendingShareQueue.addLast(payload) }
      shareActivity?.get()?.notifyShareAvailable()
    }
    return true
  }

  @Suppress("DEPRECATION")
  private fun sharedImageUris(intent: Intent): List<Uri> {
    if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
      return intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM).orEmpty()
    }
    return intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)?.let { listOf(it) }.orEmpty()
  }

  companion object {
    private val shareExecutor = Executors.newSingleThreadExecutor()
    private val pendingShareQueue = ArrayDeque<String>()
    @Volatile private var shareActivity: WeakReference<MainActivity>? = null

    private const val APK_MIME_TYPE = "application/vnd.android.package-archive"
    private const val GITHUB_RELEASE_API_URL =
      "https://api.github.com/repos/LiKPO4/clipstash/releases/latest"
    private const val PREFS_UPDATE = "clipstash_android_update"
    private const val KEY_DOWNLOAD_ID = "download_id"
    private const val KEY_DOWNLOAD_APK_PATH = "download_apk_path"
  }
}
