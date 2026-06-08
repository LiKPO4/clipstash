import { type ChangeEvent, type FormEvent, useEffect, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import "./App.css";
import {
  copyLegacyImageToClipboard,
  createLegacyImageMessage,
  createLegacyMixedMessage,
  createLegacyTextMessage,
  deleteLegacyMessage,
  getLegacyStats,
  listLegacyMessages,
  replaceLegacyMessageImages,
  setLegacyMessageArchived,
  stageLegacyMessageImportToClipboard,
  updateLegacyMessageText,
} from "./api/legacy";
import type {
  LegacyMessageImage,
  LegacyMessage,
  LegacyMessagePage,
  LegacyStats,
  LegacyArchiveMessageResult,
  LegacyCopyImageResult,
  LegacyCreateTextMessageResult,
  LegacyImportStageResult,
  LegacyReplaceImagesResult,
  MessageView,
  SortOrder,
} from "./api/types";

const PAGE_LIMIT = 30;

type PreviewImage = {
  filename: string;
  path: string;
  src: string;
};

type EditResult = LegacyCreateTextMessageResult | LegacyReplaceImagesResult;

type CopyResult = {
  messageId: number;
  textLength: number;
};

type ImageCopyResult = LegacyCopyImageResult;

type ImportStageResult = LegacyImportStageResult;

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
  const [mediaTextDraft, setMediaTextDraft] = useState("");
  const [mediaFiles, setMediaFiles] = useState<File[]>([]);
  const [mediaInputKey, setMediaInputKey] = useState(0);
  const [mediaWriteConfirmed, setMediaWriteConfirmed] = useState(false);
  const [creatingMediaMessage, setCreatingMediaMessage] = useState(false);
  const [createMediaError, setCreateMediaError] = useState<string | null>(null);
  const [createMediaResult, setCreateMediaResult] =
    useState<LegacyCreateTextMessageResult | null>(null);
  const [editingMessage, setEditingMessage] = useState<LegacyMessage | null>(null);
  const [editTextDraft, setEditTextDraft] = useState("");
  const [editFiles, setEditFiles] = useState<File[]>([]);
  const [editInputKey, setEditInputKey] = useState(0);
  const [editConfirmed, setEditConfirmed] = useState(false);
  const [savingEdit, setSavingEdit] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);
  const [editResult, setEditResult] = useState<EditResult | null>(null);
  const [deletingMessage, setDeletingMessage] = useState<LegacyMessage | null>(null);
  const [deleteConfirmed, setDeleteConfirmed] = useState(false);
  const [deletingLegacyMessage, setDeletingLegacyMessage] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  const [deleteResult, setDeleteResult] = useState<EditResult | null>(null);
  const [archivingMessageId, setArchivingMessageId] = useState<number | null>(null);
  const [archiveError, setArchiveError] = useState<string | null>(null);
  const [archiveResult, setArchiveResult] = useState<LegacyArchiveMessageResult | null>(null);
  const [copyError, setCopyError] = useState<string | null>(null);
  const [copyResult, setCopyResult] = useState<CopyResult | null>(null);
  const [copyImageError, setCopyImageError] = useState<string | null>(null);
  const [copyImageResult, setCopyImageResult] = useState<ImageCopyResult | null>(null);
  const [importingMessageId, setImportingMessageId] = useState<number | null>(null);
  const [importStageError, setImportStageError] = useState<string | null>(null);
  const [importStageResult, setImportStageResult] = useState<ImportStageResult | null>(null);

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

  async function refreshLegacyData() {
    const [nextStats, nextPage] = await loadLegacyData(view, sort);
    setStats(nextStats);
    setPage(nextPage);
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
      await refreshLegacyData();
      setTextDraft("");
      setWriteConfirmed(false);
      setCreateTextResult(result);
    } catch (err) {
      setCreateTextError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreatingTextMessage(false);
    }
  }

  function selectMediaFiles(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    setMediaFiles(files);
    setCreateMediaError(null);
    setCreateMediaResult(null);
  }

  async function createMediaMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    if (mediaFiles.length === 0 || !mediaWriteConfirmed || creatingMediaMessage) return;

    setCreatingMediaMessage(true);
    setCreateMediaError(null);
    setCreateMediaResult(null);

    try {
      const imagesData = await filesToNumberArrays(mediaFiles);
      const text = mediaTextDraft.trim();
      const result = text
        ? await createLegacyMixedMessage(text, imagesData)
        : await createLegacyImageMessage(imagesData);
      await refreshLegacyData();
      setMediaTextDraft("");
      setMediaFiles([]);
      setMediaInputKey((key) => key + 1);
      setMediaWriteConfirmed(false);
      setCreateMediaResult(result);
    } catch (err) {
      setCreateMediaError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreatingMediaMessage(false);
    }
  }

  function openEditMessage(message: LegacyMessage) {
    setEditingMessage(message);
    setEditTextDraft(message.text_content ?? "");
    setEditFiles([]);
    setEditInputKey((key) => key + 1);
    setEditConfirmed(false);
    setEditError(null);
    setEditResult(null);
  }

  function closeEditMessage() {
    if (savingEdit) return;
    setEditingMessage(null);
    setEditError(null);
    setEditResult(null);
  }

  function selectEditFiles(event: ChangeEvent<HTMLInputElement>) {
    setEditFiles(Array.from(event.target.files ?? []));
    setEditError(null);
    setEditResult(null);
  }

  async function saveEditedMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingMessage || !canSaveEdit) return;

    setSavingEdit(true);
    setEditError(null);
    setEditResult(null);

    try {
      let result: EditResult | null = null;
      const text = editTextDraft.trim();
      const normalizedText = text.length > 0 ? text : null;
      if ((editingMessage.text_content ?? null) !== normalizedText) {
        result = await updateLegacyMessageText(editingMessage.id, normalizedText);
      }
      if (editFiles.length > 0) {
        const imagesData = await filesToNumberArrays(editFiles);
        result = await replaceLegacyMessageImages(editingMessage.id, imagesData);
      }
      if (!result) {
        throw new Error("没有需要保存的变更");
      }
      await refreshLegacyData();
      setEditFiles([]);
      setEditInputKey((key) => key + 1);
      setEditConfirmed(false);
      setEditResult(result);
    } catch (err) {
      setEditError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingEdit(false);
    }
  }

  function openDeleteMessage(message: LegacyMessage) {
    setDeletingMessage(message);
    setDeleteConfirmed(false);
    setDeleteError(null);
    setDeleteResult(null);
  }

  function closeDeleteMessage() {
    if (deletingLegacyMessage) return;
    setDeletingMessage(null);
    setDeleteError(null);
  }

  async function confirmDeleteMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!deletingMessage || !deleteConfirmed || deletingLegacyMessage) return;

    setDeletingLegacyMessage(true);
    setDeleteError(null);
    setDeleteResult(null);

    try {
      const result = await deleteLegacyMessage(deletingMessage.id);
      await refreshLegacyData();
      setDeleteResult(result);
      setDeletingMessage(null);
      setDeleteConfirmed(false);
    } catch (err) {
      setDeleteError(err instanceof Error ? err.message : String(err));
    } finally {
      setDeletingLegacyMessage(false);
    }
  }

  async function toggleArchiveMessage(message: LegacyMessage) {
    if (archivingMessageId !== null) return;

    setArchivingMessageId(message.id);
    setArchiveError(null);
    setArchiveResult(null);

    try {
      const result = await setLegacyMessageArchived(message.id, !message.archived);
      await refreshLegacyData();
      setArchiveResult(result);
    } catch (err) {
      setArchiveError(err instanceof Error ? err.message : String(err));
    } finally {
      setArchivingMessageId(null);
    }
  }

  async function copyMessageText(message: LegacyMessage) {
    const text = message.text_content?.trim();
    if (!text) return;

    setCopyError(null);
    setCopyResult(null);

    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("当前环境不支持剪贴板写入");
      }
      await navigator.clipboard.writeText(text);
      setCopyResult({ messageId: message.id, textLength: text.length });
    } catch (err) {
      setCopyError(err instanceof Error ? err.message : String(err));
    }
  }

  async function copyMessageImage(image: LegacyMessageImage) {
    if (!image.exists) return;

    setCopyImageError(null);
    setCopyImageResult(null);

    try {
      const result = await copyLegacyImageToClipboard(image.filename);
      setCopyImageResult(result);
    } catch (err) {
      setCopyImageError(err instanceof Error ? err.message : String(err));
    }
  }

  async function stageMessageImport(message: LegacyMessage) {
    if (importingMessageId !== null) return;

    setImportingMessageId(message.id);
    setImportStageError(null);
    setImportStageResult(null);

    try {
      const result = await stageLegacyMessageImportToClipboard(message.id);
      setImportStageResult(result);
    } catch (err) {
      setImportStageError(err instanceof Error ? err.message : String(err));
    } finally {
      setImportingMessageId(null);
    }
  }

  const visibleCount = page?.messages.length ?? 0;
  const canCreateText = textDraft.trim().length > 0 && writeConfirmed && !creatingTextMessage;
  const canCreateMedia =
    mediaFiles.length > 0 && mediaWriteConfirmed && !creatingMediaMessage;
  const editText = editTextDraft.trim();
  const editWillHaveImages = (editingMessage?.images.length ?? 0) > 0 || editFiles.length > 0;
  const editHasContent = editText.length > 0 || editWillHaveImages;
  const canSaveEdit =
    !!editingMessage &&
    editConfirmed &&
    !savingEdit &&
    editHasContent &&
    (((editingMessage.text_content ?? null) !==
      (editText.length > 0 ? editText : null)) ||
      editFiles.length > 0);

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

          <section className="write-panel" aria-label="新增图片或图文消息">
            <div className="write-panel-head">
              <div>
                <p className="eyebrow">Phase 2 / Media Guard</p>
                <h2>新增图片 / 图文消息</h2>
              </div>
              <span className="write-badge media-badge">备份后写入</span>
            </div>

            <form className="text-create-form" onSubmit={createMediaMessage}>
              <label className="field-label" htmlFor="new-media-message-text">
                配套文字
              </label>
              <textarea
                id="new-media-message-text"
                value={mediaTextDraft}
                onChange={(event) => setMediaTextDraft(event.target.value)}
                placeholder="留空则创建纯图片消息"
                rows={3}
              />

              <label className="field-label" htmlFor="new-media-message-files">
                图片
              </label>
              <input
                key={mediaInputKey}
                id="new-media-message-files"
                className="file-input"
                type="file"
                accept="image/*"
                multiple
                onChange={selectMediaFiles}
              />

              {mediaFiles.length > 0 && (
                <div className="selected-files" aria-label="已选图片">
                  {mediaFiles.map((file, index) => (
                    <span key={`${file.name}-${file.size}-${index}`}>
                      {file.name} · {formatBytes(file.size)}
                    </span>
                  ))}
                </div>
              )}

              <label className="write-confirm">
                <input
                  type="checkbox"
                  checked={mediaWriteConfirmed}
                  onChange={(event) => setMediaWriteConfirmed(event.target.checked)}
                />
                <span>确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。</span>
              </label>

              <button type="submit" className="write-submit" disabled={!canCreateMedia}>
                {creatingMediaMessage ? "正在写入..." : "新增并备份"}
              </button>
            </form>

            {createMediaError && (
              <div className="write-result write-result-error" role="alert">
                <strong>写入失败</strong>
                <p>{createMediaError}</p>
              </div>
            )}

            {createMediaResult && (
              <div className="write-result write-result-ok" role="status">
                <strong>已写入 #{createMediaResult.message.id}</strong>
                <p>
                  {createMediaResult.message.created_at} · 图片{" "}
                  {createMediaResult.message.images.length}
                </p>
                <PathRow
                  label="备份"
                  value={createMediaResult.backup.backup_path}
                  ok={createMediaResult.backup.bytes_copied > 0}
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

          {page && (
            <MessageList
              messages={page.messages}
              archivingMessageId={archivingMessageId}
              importingMessageId={importingMessageId}
              onDelete={openDeleteMessage}
              onEdit={openEditMessage}
              onArchive={toggleArchiveMessage}
              onCopyImage={copyMessageImage}
              onCopyText={copyMessageText}
              onStageImport={stageMessageImport}
              onPreview={setPreviewImage}
            />
          )}

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

      {editingMessage && (
        <EditMessageDialog
          confirmed={editConfirmed}
          error={editError}
          files={editFiles}
          inputKey={editInputKey}
          message={editingMessage}
          result={editResult}
          saving={savingEdit}
          textDraft={editTextDraft}
          canSave={canSaveEdit}
          onClose={closeEditMessage}
          onConfirmChange={setEditConfirmed}
          onFileChange={selectEditFiles}
          onSubmit={saveEditedMessage}
          onTextChange={setEditTextDraft}
        />
      )}

      {deletingMessage && (
        <DeleteMessageDialog
          confirmed={deleteConfirmed}
          error={deleteError}
          message={deletingMessage}
          deleting={deletingLegacyMessage}
          onClose={closeDeleteMessage}
          onConfirmChange={setDeleteConfirmed}
          onSubmit={confirmDeleteMessage}
        />
      )}

      {deleteResult && (
        <section className="floating-result" role="status">
          <strong>已删除 #{deleteResult.message.id}</strong>
          <PathRow
            label="备份"
            value={deleteResult.backup.backup_path}
            ok={deleteResult.backup.bytes_copied > 0}
          />
        </section>
      )}

      {(archiveError || archiveResult) && (
        <section
          className={`floating-result ${archiveError ? "floating-result-error" : ""}`}
          role={archiveError ? "alert" : "status"}
        >
          {archiveError ? (
            <>
              <strong>归档操作失败</strong>
              <p>{archiveError}</p>
            </>
          ) : (
            archiveResult && (
              <>
                <strong>
                  {archiveResult.message.archived ? "已归档" : "已恢复"} #
                  {archiveResult.message.id}
                </strong>
                <PathRow
                  label="备份"
                  value={archiveResult.backup.backup_path}
                  ok={archiveResult.backup.bytes_copied > 0}
                />
              </>
            )
          )}
        </section>
      )}

      {(copyError || copyResult) && (
        <section
          className={`floating-result ${copyError ? "floating-result-error" : ""}`}
          role={copyError ? "alert" : "status"}
        >
          {copyError ? (
            <>
              <strong>复制失败</strong>
              <p>{copyError}</p>
            </>
          ) : (
            copyResult && (
              <>
                <strong>已复制 #{copyResult.messageId}</strong>
                <p>{copyResult.textLength} 个字符</p>
              </>
            )
          )}
        </section>
      )}

      {(copyImageError || copyImageResult) && (
        <section
          className={`floating-result ${copyImageError ? "floating-result-error" : ""}`}
          role={copyImageError ? "alert" : "status"}
        >
          {copyImageError ? (
            <>
              <strong>复制图片失败</strong>
              <p>{copyImageError}</p>
            </>
          ) : (
            copyImageResult && (
              <>
                <strong>已复制图片</strong>
                <p>
                  {copyImageResult.filename} · {copyImageResult.width} ×{" "}
                  {copyImageResult.height}
                </p>
              </>
            )
          )}
        </section>
      )}

      {(importStageError || importStageResult) && (
        <section
          className={`floating-result ${importStageError ? "floating-result-error" : ""}`}
          role={importStageError ? "alert" : "status"}
        >
          {importStageError ? (
            <>
              <strong>准备导入失败</strong>
              <p>{importStageError}</p>
            </>
          ) : (
            importStageResult && (
              <>
                <strong>已准备导入 #{importStageResult.message_id}</strong>
                <p>
                  {importStageResult.staged_kind === "text"
                    ? `${importStageResult.text_length} 个字符已进入剪贴板`
                    : `${importStageResult.first_image_filename ?? "图片"} 已进入剪贴板`}
                </p>
              </>
            )
          )}
        </section>
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
  archivingMessageId,
  importingMessageId,
  messages,
  onArchive,
  onCopyImage,
  onCopyText,
  onDelete,
  onEdit,
  onStageImport,
  onPreview,
}: {
  archivingMessageId: number | null;
  importingMessageId: number | null;
  messages: LegacyMessage[];
  onArchive: (message: LegacyMessage) => void;
  onCopyImage: (image: LegacyMessageImage) => void;
  onCopyText: (message: LegacyMessage) => void;
  onDelete: (message: LegacyMessage) => void;
  onEdit: (message: LegacyMessage) => void;
  onStageImport: (message: LegacyMessage) => void;
  onPreview: (image: PreviewImage) => void;
}) {
  return (
    <section className="message-list" aria-label="旧消息列表">
      {messages.map((message) => (
        <article className="message-card" key={message.id}>
          <header className="message-meta">
            <div className="message-meta-text">
              <strong>#{message.id}</strong>
              <span>{message.created_at}</span>
              {message.archived && <span>归档于 {message.archived_at ?? "未知时间"}</span>}
            </div>
            <div className="message-actions" aria-label={`消息 ${message.id} 操作`}>
              <button
                type="button"
                className="archive-action"
                disabled={archivingMessageId !== null}
                onClick={() => onArchive(message)}
              >
                {archivingMessageId === message.id
                  ? "处理中..."
                  : message.archived
                    ? "恢复"
                    : "归档"}
              </button>
              {message.text_content && (
                <button type="button" onClick={() => onCopyText(message)}>
                  复制文字
                </button>
              )}
              <button
                type="button"
                disabled={importingMessageId !== null}
                onClick={() => onStageImport(message)}
              >
                {importingMessageId === message.id ? "准备中..." : "准备导入"}
              </button>
              <button type="button" onClick={() => onEdit(message)}>
                编辑
              </button>
              <button type="button" className="danger-action" onClick={() => onDelete(message)}>
                删除
              </button>
            </div>
          </header>

          <p className={message.text_content ? "message-text" : "message-text empty-text"}>
            {message.text_content || "无文字内容"}
          </p>

          {message.images.length > 0 && (
            <div className="image-grid" aria-label="图片缩略图">
              {message.images.map((image) => (
                <MessageImageTile
                  image={image}
                  key={image.id}
                  onCopy={onCopyImage}
                  onPreview={onPreview}
                />
              ))}
            </div>
          )}
        </article>
      ))}
    </section>
  );
}

function EditMessageDialog({
  canSave,
  confirmed,
  error,
  files,
  inputKey,
  message,
  result,
  saving,
  textDraft,
  onClose,
  onConfirmChange,
  onFileChange,
  onSubmit,
  onTextChange,
}: {
  canSave: boolean;
  confirmed: boolean;
  error: string | null;
  files: File[];
  inputKey: number;
  message: LegacyMessage;
  result: EditResult | null;
  saving: boolean;
  textDraft: string;
  onClose: () => void;
  onConfirmChange: (confirmed: boolean) => void;
  onFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onTextChange: (text: string) => void;
}) {
  return (
    <div className="preview-backdrop edit-backdrop" role="presentation" onClick={onClose}>
      <section
        aria-label={`编辑消息 ${message.id}`}
        aria-modal="true"
        className="edit-dialog"
        role="dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="edit-header">
          <div>
            <p className="eyebrow">Phase 2 / Edit Guard</p>
            <h2>编辑 #{message.id}</h2>
          </div>
          <button type="button" className="preview-close" onClick={onClose} aria-label="关闭编辑">
            ×
          </button>
        </header>

        <form className="text-create-form" onSubmit={onSubmit}>
          <label className="field-label" htmlFor="edit-message-text">
            文字内容
          </label>
          <textarea
            id="edit-message-text"
            value={textDraft}
            onChange={(event) => onTextChange(event.target.value)}
            rows={5}
          />

          <label className="field-label" htmlFor="edit-message-files">
            替换图片
          </label>
          <input
            key={inputKey}
            id="edit-message-files"
            className="file-input"
            type="file"
            accept="image/*"
            multiple
            onChange={onFileChange}
          />

          {files.length > 0 && (
            <div className="selected-files" aria-label="待替换图片">
              {files.map((file, index) => (
                <span key={`${file.name}-${file.size}-${index}`}>
                  {file.name} · {formatBytes(file.size)}
                </span>
              ))}
            </div>
          )}

          <label className="write-confirm">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(event) => onConfirmChange(event.target.checked)}
            />
            <span>确认本次会写入旧数据库和旧图片目录，并在写入前自动创建备份。</span>
          </label>

          <div className="dialog-actions">
            <button type="submit" className="write-submit" disabled={!canSave}>
              {saving ? "正在保存..." : "保存并备份"}
            </button>
            <button type="button" className="secondary-action" onClick={onClose}>
              关闭
            </button>
          </div>
        </form>

        {error && (
          <div className="write-result write-result-error" role="alert">
            <strong>保存失败</strong>
            <p>{error}</p>
          </div>
        )}

        {result && (
          <div className="write-result write-result-ok" role="status">
            <strong>已保存 #{result.message.id}</strong>
            <PathRow
              label="备份"
              value={result.backup.backup_path}
              ok={result.backup.bytes_copied > 0}
            />
          </div>
        )}
      </section>
    </div>
  );
}

function DeleteMessageDialog({
  confirmed,
  deleting,
  error,
  message,
  onClose,
  onConfirmChange,
  onSubmit,
}: {
  confirmed: boolean;
  deleting: boolean;
  error: string | null;
  message: LegacyMessage;
  onClose: () => void;
  onConfirmChange: (confirmed: boolean) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
}) {
  return (
    <div className="preview-backdrop edit-backdrop" role="presentation" onClick={onClose}>
      <section
        aria-label={`删除消息 ${message.id}`}
        aria-modal="true"
        className="delete-dialog"
        role="dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="edit-header">
          <div>
            <p className="eyebrow">Phase 2 / Delete Guard</p>
            <h2>删除 #{message.id}</h2>
          </div>
          <button type="button" className="preview-close" onClick={onClose} aria-label="关闭删除">
            ×
          </button>
        </header>

        <p className="delete-copy">
          这会删除旧数据库中的消息记录和关联图片文件，执行前会自动备份数据库和现有图片。
        </p>

        <form className="text-create-form" onSubmit={onSubmit}>
          <label className="write-confirm">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(event) => onConfirmChange(event.target.checked)}
            />
            <span>确认删除这条旧库消息，并保留自动备份用于回滚。</span>
          </label>

          <div className="dialog-actions">
            <button
              type="submit"
              className="write-submit delete-submit"
              disabled={!confirmed || deleting}
            >
              {deleting ? "正在删除..." : "删除并备份"}
            </button>
            <button type="button" className="secondary-action" onClick={onClose}>
              取消
            </button>
          </div>
        </form>

        {error && (
          <div className="write-result write-result-error" role="alert">
            <strong>删除失败</strong>
            <p>{error}</p>
          </div>
        )}
      </section>
    </div>
  );
}

function MessageImageTile({
  image,
  onCopy,
  onPreview,
}: {
  image: LegacyMessageImage;
  onCopy: (image: LegacyMessageImage) => void;
  onPreview: (image: PreviewImage) => void;
}) {
  const [broken, setBroken] = useState(false);
  const canRenderImage = image.exists && !broken;
  const src = canRenderImage ? getAssetSrc(image.path) : "";

  if (canRenderImage && src) {
    return (
      <div className="image-tile" title={image.path}>
        <button
          type="button"
          className="image-preview-action"
          onClick={() => onPreview({ filename: image.filename, path: image.path, src })}
        >
          <img
            alt={image.filename}
            loading="lazy"
            src={src}
            onError={() => setBroken(true)}
          />
        </button>
        <div className="image-tile-actions">
          <button type="button" className="image-copy-action" onClick={() => onCopy(image)}>
            复制图片
          </button>
        </div>
        <span className="image-caption">{image.filename}</span>
      </div>
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

async function filesToNumberArrays(files: File[]) {
  return Promise.all(
    files.map(async (file) => {
      const buffer = await file.arrayBuffer();
      return Array.from(new Uint8Array(buffer));
    }),
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

export default App;
