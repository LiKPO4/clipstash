import { type FormEvent, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import "./App.css";
import { createLegacyTextMessage, getLegacyStats, listLegacyMessages } from "./api/legacy";
import type {
  LegacyMessageImage,
  LegacyMessage,
  LegacyMessagePage,
  LegacyStats,
  LegacyCreateTextMessageResult,
  MessageView,
  SortOrder,
} from "./api/types";

const PAGE_LIMIT = 30;

type PreviewImage = {
  filename: string;
  path: string;
  src: string;
};

function App() {
  const [stats, setStats] = useState<LegacyStats | null>(null);
  const [page, setPage] = useState<LegacyMessagePage | null>(null);
  const [view, setView] = useState<MessageView>("normal");
  const [sort, setSort] = useState<SortOrder>("newest");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);
  const [previewImage, setPreviewImage] = useState<PreviewImage | null>(null);
  const [textDraft, setTextDraft] = useState("");
  const [writeConfirmed, setWriteConfirmed] = useState(false);
  const [creatingTextMessage, setCreatingTextMessage] = useState(false);
  const [createTextError, setCreateTextError] = useState<string | null>(null);
  const [createTextResult, setCreateTextResult] =
    useState<LegacyCreateTextMessageResult | null>(null);

  useEffect(() => {
    let alive = true;

    setLoading(true);
    setError(null);

    loadLegacyData(view, sort)
      .then(([nextStats, nextPage]) => {
        if (!alive) return;
        setStats(nextStats);
        setPage(nextPage);
        setError(null);
      })
      .catch((err: unknown) => {
        if (!alive) return;
        setError(err instanceof Error ? err.message : String(err));
        setPage(null);
      })
      .finally(() => {
        if (alive) setLoading(false);
      });

    return () => {
      alive = false;
    };
  }, [view, sort]);

  useEffect(() => {
    if (!previewImage) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setPreviewImage(null);
      }
    };

    document.body.classList.add("preview-open");
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      document.body.classList.remove("preview-open");
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [previewImage]);

  async function loadMore() {
    if (!page || loadingMore) return;

    setLoadingMore(true);
    setError(null);

    try {
      const nextPage = await listLegacyMessages({
        view,
        sort,
        offset: page.offset + page.messages.length,
        limit: PAGE_LIMIT,
      });
      setPage({
        ...nextPage,
        offset: 0,
        messages: [...page.messages, ...nextPage.messages],
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingMore(false);
    }
  }

  async function createTextMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const text = textDraft.trim();
    if (!text || !writeConfirmed || creatingTextMessage) return;

    setCreatingTextMessage(true);
    setCreateTextError(null);
    setCreateTextResult(null);

    try {
      const result = await createLegacyTextMessage(text);
      const [nextStats, nextPage] = await loadLegacyData(view, sort);
      setStats(nextStats);
      setPage(nextPage);
      setTextDraft("");
      setWriteConfirmed(false);
      setCreateTextResult(result);
    } catch (err) {
      setCreateTextError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreatingTextMessage(false);
    }
  }

  const visibleCount = page?.messages.length ?? 0;
  const canCreateText = textDraft.trim().length > 0 && writeConfirmed && !creatingTextMessage;

  return (
    <main className="shell">
      <section className="summary">
        <p className="eyebrow">ClipStash Next / 阶段 2</p>
        <h1>旧库兼容工作台</h1>
        <p className="lede">读取旧数据库和图片目录，分步验证新增文字消息的备份写入路径。</p>
      </section>

      {loading && <p className="status">正在读取旧数据库...</p>}

      {error && (
        <section className="notice" role="alert">
          <span className="dot error-dot" />
          <div>
            <strong>读取失败</strong>
            <p>{error}</p>
          </div>
        </section>
      )}

      {stats && (
        <>
          <section className="metrics" aria-label="消息计数">
            <article className="metric total">
              <span>总消息</span>
              <strong>{stats.total_count}</strong>
            </article>
            <article className="metric normal">
              <span>普通消息</span>
              <strong>{stats.normal_count}</strong>
            </article>
            <article className="metric archived">
              <span>已归档</span>
              <strong>{stats.archived_count}</strong>
            </article>
          </section>

          <section className="paths" aria-label="旧数据路径">
            <PathRow label="数据目录" value={stats.data_dir} ok={stats.db_exists} />
            <PathRow label="数据库" value={stats.db_path} ok={stats.db_exists} />
            <PathRow label="图片目录" value={stats.images_dir} ok={stats.images_dir_exists} />
          </section>

          <section className="write-panel" aria-label="新增纯文字消息">
            <div className="write-panel-head">
              <div>
                <p className="eyebrow">Phase 2 / Write Guard</p>
                <h2>新增纯文字消息</h2>
              </div>
              <span className="write-badge">备份后写入</span>
            </div>

            <form className="text-create-form" onSubmit={createTextMessage}>
              <label className="field-label" htmlFor="new-text-message">
                文字内容
              </label>
              <textarea
                id="new-text-message"
                value={textDraft}
                onChange={(event) => setTextDraft(event.target.value)}
                placeholder="输入要写入旧 clipstash.db 的纯文字消息"
                rows={4}
              />

              <label className="write-confirm">
                <input
                  type="checkbox"
                  checked={writeConfirmed}
                  onChange={(event) => setWriteConfirmed(event.target.checked)}
                />
                <span>确认本次会写入旧数据库，并在写入前自动创建备份。</span>
              </label>

              <button type="submit" className="write-submit" disabled={!canCreateText}>
                {creatingTextMessage ? "正在写入..." : "新增并备份"}
              </button>
            </form>

            {createTextError && (
              <div className="write-result write-result-error" role="alert">
                <strong>写入失败</strong>
                <p>{createTextError}</p>
              </div>
            )}

            {createTextResult && (
              <div className="write-result write-result-ok" role="status">
                <strong>已写入 #{createTextResult.message.id}</strong>
                <p>{createTextResult.message.created_at}</p>
                <PathRow
                  label="备份"
                  value={createTextResult.backup.backup_path}
                  ok={createTextResult.backup.bytes_copied > 0}
                />
              </div>
            )}
          </section>

          <section className="toolbar" aria-label="消息列表控制">
            <div className="segmented" role="tablist" aria-label="消息视图">
              <button
                type="button"
                className={view === "normal" ? "active" : ""}
                onClick={() => setView("normal")}
              >
                普通
              </button>
              <button
                type="button"
                className={view === "archived" ? "active" : ""}
                onClick={() => setView("archived")}
              >
                已归档
              </button>
            </div>

            <div className="segmented" aria-label="排序">
              <button
                type="button"
                className={sort === "newest" ? "active" : ""}
                onClick={() => setSort("newest")}
              >
                最新
              </button>
              <button
                type="button"
                className={sort === "oldest" ? "active" : ""}
                onClick={() => setSort("oldest")}
              >
                最早
              </button>
            </div>
          </section>

          <section className="list-head" aria-label="当前列表状态">
            <strong>{view === "normal" ? "普通消息" : "已归档消息"}</strong>
            <span>
              已显示 {visibleCount} / {page?.total_count ?? 0}
            </span>
          </section>

          {page && <MessageList messages={page.messages} onPreview={setPreviewImage} />}

          {page && page.messages.length === 0 && (
            <p className="empty">当前视图没有消息。</p>
          )}

          {page?.has_more && (
            <button
              type="button"
              className="load-more"
              onClick={loadMore}
              disabled={loadingMore}
            >
              {loadingMore ? "正在加载..." : "加载更多"}
            </button>
          )}
        </>
      )}

      {previewImage && (
        <ImagePreviewDialog image={previewImage} onClose={() => setPreviewImage(null)} />
      )}
    </main>
  );
}

function PathRow({
  label,
  value,
  ok,
}: {
  label: string;
  value: string;
  ok: boolean;
}) {
  return (
    <div className="path-row">
      <span className={ok ? "dot ok-dot" : "dot error-dot"} />
      <span className="path-label">{label}</span>
      <code title={value}>{value}</code>
    </div>
  );
}

function MessageList({
  messages,
  onPreview,
}: {
  messages: LegacyMessage[];
  onPreview: (image: PreviewImage) => void;
}) {
  return (
    <section className="message-list" aria-label="旧消息列表">
      {messages.map((message) => (
        <article className="message-card" key={message.id}>
          <header className="message-meta">
            <strong>#{message.id}</strong>
            <span>{message.created_at}</span>
            {message.archived && <span>归档于 {message.archived_at ?? "未知时间"}</span>}
          </header>

          <p className={message.text_content ? "message-text" : "message-text empty-text"}>
            {message.text_content || "无文字内容"}
          </p>

          {message.images.length > 0 && (
            <div className="image-grid" aria-label="图片缩略图">
              {message.images.map((image) => (
                <MessageImageTile image={image} key={image.id} onPreview={onPreview} />
              ))}
            </div>
          )}
        </article>
      ))}
    </section>
  );
}

function MessageImageTile({
  image,
  onPreview,
}: {
  image: LegacyMessageImage;
  onPreview: (image: PreviewImage) => void;
}) {
  const [broken, setBroken] = useState(false);
  const canRenderImage = image.exists && !broken;
  const src = canRenderImage ? getAssetSrc(image.path) : "";

  if (canRenderImage && src) {
    return (
      <button
        type="button"
        className="image-tile"
        title={image.path}
        onClick={() => onPreview({ filename: image.filename, path: image.path, src })}
      >
        <img
          alt={image.filename}
          loading="lazy"
          src={src}
          onError={() => setBroken(true)}
        />
        <span className="image-caption">{image.filename}</span>
      </button>
    );
  }

  return (
    <div className="image-tile image-tile-missing" title={image.path}>
      <span className="image-placeholder">{image.exists ? "无法读取" : "文件缺失"}</span>
      <span className="image-caption">{image.filename}</span>
    </div>
  );
}

function ImagePreviewDialog({
  image,
  onClose,
}: {
  image: PreviewImage;
  onClose: () => void;
}) {
  return (
    <div className="preview-backdrop" role="presentation" onClick={onClose}>
      <section
        aria-label={image.filename}
        aria-modal="true"
        className="preview-dialog"
        role="dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="preview-header">
          <strong title={image.path}>{image.filename}</strong>
          <button type="button" className="preview-close" onClick={onClose} aria-label="关闭预览">
            ×
          </button>
        </header>
        <div className="preview-stage">
          <img alt={image.filename} src={image.src} />
        </div>
      </section>
    </div>
  );
}

function getAssetSrc(path: string) {
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

function loadLegacyData(view: MessageView, sort: SortOrder) {
  return Promise.all([
    getLegacyStats(),
    listLegacyMessages({ view, sort, offset: 0, limit: PAGE_LIMIT }),
  ]);
}

export default App;
