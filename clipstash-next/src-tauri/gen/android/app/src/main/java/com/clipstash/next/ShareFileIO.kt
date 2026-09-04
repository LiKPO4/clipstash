package com.clipstash.next

import java.io.File
import java.io.InputStream

internal object ShareFileIO {
  const val MAX_IMAGE_BYTES = 256L * 1024 * 1024
  const val MAX_PACKET_BYTES = 4L * 1024 * 1024 * 1024

  fun removeStalePackets(root: File, now: Long = System.currentTimeMillis()) {
    val canonicalRoot = root.canonicalFile
    root.listFiles()?.forEach { packet ->
      if (packet.name.matches(Regex("[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}")) &&
        packet.isDirectory && packet.canonicalFile.parentFile == canonicalRoot &&
        now - packet.lastModified() > 24L * 60 * 60 * 1000) packet.deleteRecursively()
    }
  }

  // Called only on the share executor. Memory is bounded by this buffer, not the entire image.
  fun copy(input: InputStream, target: File, limit: Long = MAX_IMAGE_BYTES): Long {
    try {
      var count = 0L
      val buffer = ByteArray(64 * 1024)
      target.outputStream().use { output ->
        while (true) {
          val length = input.read(buffer)
          if (length < 0) break
          count += length
          require(count <= limit) { "分享图片超过大小上限" }
          output.write(buffer, 0, length)
        }
      }
      require(count > 0) { "分享图片为空" }
      return count
    } catch (error: Exception) {
      target.delete()
      throw error
    }
  }
}
