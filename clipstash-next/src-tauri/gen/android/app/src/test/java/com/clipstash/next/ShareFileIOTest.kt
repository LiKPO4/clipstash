package com.clipstash.next

import java.io.ByteArrayInputStream
import java.nio.file.Files
import org.junit.Assert.*
import org.junit.Test

class ShareFileIOTest {
  @Test fun staleCleanupPreservesRecentPacketsAndUnrelatedDirectories() {
    val root = Files.createTempDirectory("clipstash-share-cleanup").toFile()
    try {
      val old = java.io.File(root, "01234567-89ab-cdef-0123-456789abcdef").apply { mkdirs(); setLastModified(1) }
      val recent = java.io.File(root, "11234567-89ab-cdef-0123-456789abcdef").apply { mkdirs() }
      val unrelated = java.io.File(root, "other").apply { mkdirs(); setLastModified(1) }
      ShareFileIO.removeStalePackets(root)
      assertFalse(old.exists()); assertTrue(recent.exists()); assertTrue(unrelated.exists())
    } finally { root.deleteRecursively() }
  }
  @Test fun copiesInBoundedChunksAndDeletesPartialFilesOnFailure() {
    val dir = Files.createTempDirectory("clipstash-share-test").toFile()
    try {
      val bytes = ByteArray(150_000) { (it % 251).toByte() }
      var largestRead = 0
      val input = object : ByteArrayInputStream(bytes) {
        override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
          largestRead = maxOf(largestRead, length)
          return super.read(buffer, offset, length)
        }
      }
      val target = java.io.File(dir, "0.bin")
      assertEquals(bytes.size.toLong(), ShareFileIO.copy(input, target))
      assertTrue(largestRead <= 64 * 1024)
      assertArrayEquals(bytes, target.readBytes())
      try {
        ShareFileIO.copy(ByteArrayInputStream(bytes), target, 70_000)
        fail("oversized image must fail")
      } catch (_: IllegalArgumentException) { assertFalse(target.exists()) }
      try {
        ShareFileIO.copy(ByteArrayInputStream(byteArrayOf()), target)
        fail("empty image must fail")
      } catch (_: IllegalArgumentException) { assertFalse(target.exists()) }
    } finally { dir.deleteRecursively() }
  }
}
