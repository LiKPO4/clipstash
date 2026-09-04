type Entry = {
  references: number;
  bytes: number;
  url?: string;
  promise: Promise<string>;
};

export type ThumbnailLease = { result: Promise<string>; release: () => void };

/** Compressed thumbnails only. Visible images are pinned; unused images share a 16 MiB LRU. */
export class ThumbnailCache {
  private entries = new Map<string, Entry>();
  private running = 0;
  private queue: Array<() => void> = [];

  constructor(
    private budget = 16 * 1024 * 1024,
    private concurrency = 4,
    private createUrl = (bytes: Uint8Array) => URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: "image/png" })),
    private revokeUrl = (url: string) => URL.revokeObjectURL(url),
  ) {}

  acquire(key: string, load: () => Promise<Uint8Array>): ThumbnailLease {
    let entry = this.entries.get(key);
    if (entry) {
      this.entries.delete(key);
      this.entries.set(key, entry);
      entry.references += 1;
    } else {
      entry = { references: 1, bytes: 0, promise: Promise.resolve("") };
      const created = entry;
      this.entries.set(key, created);
      created.promise = this.schedule(async () => {
        if (this.entries.get(key) !== created || !created.references) throw new Error("Thumbnail request released");
        const bytes = await load();
        if (this.entries.get(key) !== created || !created.references) throw new Error("Thumbnail request released");
        created.url = this.createUrl(bytes);
        created.bytes = bytes.byteLength;
        this.trim();
        return created.url;
      }).catch((error: unknown) => {
        if (this.entries.get(key) === created) this.entries.delete(key);
        throw error;
      });
    }

    const acquired = entry;
    let released = false;
    return {
      result: acquired.promise,
      release: () => {
        if (released) return;
        released = true;
        acquired.references -= 1;
        if (!acquired.references && !acquired.url && this.entries.get(key) === acquired) this.entries.delete(key);
        this.trim();
      },
    };
  }

  clear() {
    for (const entry of this.entries.values()) if (entry.url) this.revokeUrl(entry.url);
    this.entries.clear();
  }

  get retainedBytes() {
    return Array.from(this.entries.values()).reduce((sum, entry) => sum + entry.bytes, 0);
  }

  private trim() {
    let size = this.retainedBytes;
    for (const [key, entry] of this.entries) {
      if (size <= this.budget) break;
      if (entry.references || !entry.url) continue;
      this.revokeUrl(entry.url);
      size -= entry.bytes;
      this.entries.delete(key);
    }
  }

  private schedule<T>(job: () => Promise<T>): Promise<T> {
    return new Promise<T>((resolve, reject) => {
      const start = () => {
        this.running += 1;
        void job().then(resolve, reject).finally(() => {
          this.running -= 1;
          this.queue.shift()?.();
        });
      };
      if (this.running < this.concurrency) start();
      else this.queue.push(start);
    });
  }
}
