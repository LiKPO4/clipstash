import { invoke } from "@tauri-apps/api/core";
import type { DataImportResult } from "./api/types";
import { runDataTransfer } from "./transferProgress";

const CHUNK_BYTES = 1024 * 1024;
export function importZipFile(file: File): Promise<DataImportResult> {
  return runDataTransfer("导入", async (progress, update) => {
    const uploadId = await invoke<string>("begin_zip_upload", { filename: file.name, size: file.size });
    try {
      let offset = 0;
      while (offset < file.size) {
        const bytes = await file.slice(offset, Math.min(offset + CHUNK_BYTES, file.size)).arrayBuffer();
        if (!bytes.byteLength) throw new Error("读取数据包分块为空");
        const next = await invoke<number>("append_zip_upload", bytes, {
          headers: { "x-upload-id": uploadId, "x-upload-offset": String(offset) },
        });
        if (next !== offset + bytes.byteLength) throw new Error("导入分块确认位置不正确");
        offset = next;
        update({ phase: "uploading", completed_bytes: offset, total_bytes: file.size });
      }
      return await invoke<DataImportResult>("finish_zip_upload", { uploadId, ...(progress ? { progress } : {}) });
    } catch (error) {
      try { await invoke("abort_zip_upload", { uploadId }); }
      catch (cleanupError) { throw new Error(`${String(error)}；清理暂存失败：${String(cleanupError)}`); }
      throw error;
    }
  });
}
