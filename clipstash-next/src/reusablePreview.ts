export type PreviewPosition = { left: number; top: number; width: number; height: number };
export type PreparedPreview = { filename: string; src: string; width: number; height: number; release: () => Promise<void> };
export interface PreviewWindowPort {
  hide(): Promise<void>;
  clear(): Promise<void>;
  configure(position: PreviewPosition, title: string): Promise<void>;
  load(id: number, source: PreparedPreview): Promise<void>;
  cancelLoad(): void;
  show(): Promise<void>;
}

/** One window; serialized native operations; only the newest hover is allowed to show it. */
export class ReusablePreview {
  private sequence = 0;
  private tail: Promise<void> = Promise.resolve();
  private window: PreviewWindowPort | null = null;
  private source: PreparedPreview | null = null;
  private visibleRequested = false;
  constructor(private createWindow: (onDestroyed: () => void) => Promise<PreviewWindowPort>) {}

  show(prepare: () => Promise<PreparedPreview>, position: (width: number, height: number) => PreviewPosition) {
    this.visibleRequested = true;
    const id = ++this.sequence;
    this.window?.cancelLoad();
    return this.enqueue(async () => {
      if (id !== this.sequence) return;
      try {
        this.window ??= await this.createWindow(() => {
          this.window?.cancelLoad();
          this.window = null;
          const source = this.source;
          this.source = null;
          void source?.release().catch(() => undefined);
        });
        await this.window.hide();
        await this.window.clear();
        await this.release();
        if (id !== this.sequence) return;
        this.source = await prepare();
        if (id !== this.sequence) { await this.release(); return; }
        await this.window.configure(position(this.source.width, this.source.height), this.source.filename);
        if (id !== this.sequence) { await this.release(); return; }
        await this.window.load(id, this.source);
        if (id === this.sequence) await this.window.show();
      } catch (error) {
        await this.window?.hide().catch(() => undefined);
        await this.window?.clear().catch(() => undefined);
        await this.release();
        if (id === this.sequence) throw error;
      }
    });
  }

  hide() {
    if (!this.visibleRequested) return Promise.resolve();
    this.visibleRequested = false;
    ++this.sequence;
    this.window?.cancelLoad();
    return this.enqueue(async () => {
      try {
        await this.window?.hide();
        await this.window?.clear();
      } finally { await this.release(); }
    });
  }

  private async release() {
    const source = this.source;
    this.source = null;
    await source?.release();
  }
  private enqueue(job: () => Promise<void>) {
    const result = this.tail.then(job);
    this.tail = result.catch(() => undefined);
    return result;
  }
}
