import { useEffect, useState } from "react";
import "./App.css";
import { getLegacyStats } from "./api/legacy";
import type { LegacyStats } from "./api/types";

function App() {
  const [stats, setStats] = useState<LegacyStats | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;

    getLegacyStats()
      .then((nextStats) => {
        if (!alive) return;
        setStats(nextStats);
        setError(null);
      })
      .catch((err: unknown) => {
        if (!alive) return;
        setError(err instanceof Error ? err.message : String(err));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });

    return () => {
      alive = false;
    };
  }, []);

  return (
    <main className="shell">
      <section className="summary">
        <p className="eyebrow">ClipStash Next / MVP-0</p>
        <h1>旧数据只读检查</h1>
        <p className="lede">Tauri 2 + React + TypeScript + Rust 原型已连接旧数据定位层。</p>
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

export default App;
