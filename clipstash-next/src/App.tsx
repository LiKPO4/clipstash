import { useEffect, useState } from "react";
import "./App.css";
import { getLegacyStats, listLegacyMessages } from "./api/legacy";
import type {
  LegacyMessage,
  LegacyMessagePage,
  LegacyStats,
  MessageView,
  SortOrder,
} from "./api/types";

const PAGE_LIMIT = 30;

function App() {
  const [stats, setStats] = useState<LegacyStats | null>(null);
  const [page, setPage] = useState<LegacyMessagePage | null>(null);
  const [view, setView] = useState<MessageView>("normal");
  const [sort, setSort] = useState<SortOrder>("newest");
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadingMore, setLoadingMore] = useState(false);

  useEffect(() => {
    let alive = true;

    setLoading(true);
    setError(null);

    Promise.all([
      getLegacyStats(),
      listLegacyMessages({ view, sort, offset: 0, limit: PAGE_LIMIT }),
    ])
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

  const visibleCount = page?.messages.length ?? 0;

  return (
    <main className="shell">
      <section className="summary">
        <p className="eyebrow">ClipStash Next / 阶段 1</p>
        <h1>旧消息只读列表</h1>
        <p className="lede">读取旧数据库和图片目录，展示消息归属、排序和图片文件状态。</p>
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

          {page && <MessageList messages={page.messages} />}

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

function MessageList({ messages }: { messages: LegacyMessage[] }) {
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
            <div className="image-strip" aria-label="图片文件状态">
              {message.images.map((image) => (
                <span
                  className={image.exists ? "image-chip image-ok" : "image-chip image-missing"}
                  key={image.id}
                  title={image.path}
                >
                  {image.filename}
                </span>
              ))}
            </div>
          )}
        </article>
      ))}
    </section>
  );
}

export default App;
