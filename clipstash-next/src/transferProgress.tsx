import { Channel, invoke } from "@tauri-apps/api/core";
import { useSyncExternalStore } from "react";

type Progress = { phase: string; completed_bytes: number; total_bytes: number | null };
type ActiveProgress = Progress & { id: number; title: string };
const listeners = new Set<() => void>();
const active = new Map<number, ActiveProgress>();
let snapshot: ActiveProgress[] = [];
let sequence = 0;
function publish() {
  snapshot = [...active.values()];
  listeners.forEach((notify) => notify());
}
function subscribe(notify: () => void) {
  listeners.add(notify);
  return () => { listeners.delete(notify); };
}
const getSnapshot = () => snapshot;

export async function invokeDataTransfer<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  const title = command.startsWith("export") ? "导出" : command.startsWith("preview") ? "导入预览" : "导入";
  return runDataTransfer(title, (channel) => channel ? invoke<T>(command, { ...args, progress: channel })
    : args === undefined ? invoke<T>(command) : invoke<T>(command, args));
}

export async function runDataTransfer<T>(title: string,
  work: (channel: Channel<Progress> | undefined, update: (progress: Progress) => void) => Promise<T>): Promise<T> {
  // Callers without a mounted progress view keep the original command protocol.
  if (listeners.size === 0) return work(undefined, () => {});
  const id = ++sequence;
  const channel = new Channel<Progress>();
  active.set(id, { id, title, phase: "preparing", completed_bytes: 0, total_bytes: null });
  publish();
  channel.onmessage = (value) => {
    if (!active.has(id)) return;
    active.set(id, { ...value, id, title });
    publish();
  };
  try {
    return await work(channel, channel.onmessage);
  } finally {
    active.delete(id);
    channel.onmessage = () => {};
    publish();
  }
}

const phases: Record<string, string> = {
  preparing: "准备数据", uploading: "传输数据包", export_hash: "读取并校验原图", dedupe: "检查重复消息",
  preview: "校验数据包", import: "导入图片", export_write: "写入压缩包", commit: "保存结果",
};
function size(bytes: number) { return `${(bytes / 1048576).toFixed(1)} MiB`; }

export function DataTransferProgress() {
  const tasks = useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
  if (!tasks.length) return null;
  return <aside className="data-transfer-progress" aria-label="数据处理进度">
    {tasks.map((task) => {
      const known = task.total_bytes !== null && task.total_bytes > 0;
      return <div key={task.id}>
        <div role="status">{task.title}：{phases[task.phase] ?? "处理中"}</div>
        <progress aria-label={`${task.title}进度`} max={known ? task.total_bytes! : undefined}
          value={known ? Math.min(task.completed_bytes, task.total_bytes!) : undefined} />
        {(known || task.completed_bytes > 0) && <small>
          已处理 {size(task.completed_bytes)}{known ? ` / ${size(task.total_bytes!)}` : ""}
        </small>}
      </div>;
    })}
  </aside>;
}
