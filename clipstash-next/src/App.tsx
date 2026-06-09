import {
  type ChangeEvent,
  type ClipboardEvent,
  type FormEvent,
  type ReactNode,
  type RefObject,
  type WheelEvent,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";
import {
  copyLegacyMessageTextToClipboard,
  copyLegacyImageToClipboard,
  createLegacyImageMessage,
  createLegacyMixedMessage,
  createLegacyTextMessage,
  deleteLegacyMessage,
  getAppSettings,
  getGlobalShortcutErrors,
  getLegacyStats,
  getLaunchOnStartup,
  listLegacyMessages,
  migrateLegacyData,
  pasteLegacyImportQueueToRecentWindow,
  readCurrentClipboard,
  replaceLegacyMessageImages,
  setLegacyMessageArchived,
  setLaunchOnStartup,
  previewLegacyMessageImportQueue,
  updateLegacyMessageText,
  updateAppSettings,
} from "./api/legacy";
import type {
  AppSettings,
  AppSettingsPatch,
  LegacyMessageImage,
  LegacyMessage,
  LegacyMessagePage,
  LegacyStats,
  LegacyArchiveMessageResult,
  AppMigrationResult,
  LegacyCopyImageResult,
  LegacyCreateTextMessageResult,
  LegacyImportQueuePasteArchiveResult,
  LegacyImportQueuePasteResult,
  LegacyImportQueuePreview,
  LegacyReplaceImagesResult,
  MessageView,
  SortOrder,
} from "./api/types";

const PAGE_LIMIT = 30;
const CURRENT_VERSION = "2.0.0";
const APP_TITLE = `需求暂存站 v${CURRENT_VERSION}  @linjianglu`;
const GITHUB_LATEST_RELEASE_API =
  "https://api.github.com/repos/LiKPO4/clipstash/releases/latest";
const GITHUB_RELEASES_URL = "https://github.com/LiKPO4/clipstash/releases/latest";
type PreviewImage = {
  filename: string;
  images: PreviewImageItem[];
  index: number;
  path: string;
  position?: PreviewPosition;
  src: string;
  total: number;
};

type PreviewPosition = {
  height: number;
  left: number;
  top: number;
  width: number;
};

type ScreenRect = {
  bottom: number;
  height: number;
  left: number;
  right: number;
  top: number;
  width: number;
};

type PreviewImageItem = {
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

type ImportQueuePasteAllResult = LegacyImportQueuePasteResult;

type ImportQueuePasteArchiveResult = LegacyImportQueuePasteArchiveResult;

type ReleaseCheckResult = {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  body: string;
  hasUpdate: boolean;
};

let hoverPreviewWindow: WebviewWindow | null = null;
let hoverPreviewStorageKey: string | null = null;

function App() {
  const [stats, setStats] = useState<LegacyStats | null>(null);
  const [page, setPage] = useState<LegacyMessagePage | null>(null);
  const [view, setView] = useState<MessageView>("normal");
  const [sort, setSort] = useState<SortOrder>(() => getStoredSort());
  const [error, setError] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [previewImage, setPreviewImage] = useState<PreviewImage | null>(null);
  const [hoverDelay, setHoverDelay] = useState(0.8);
  const [scrollLines, setScrollLines] = useState(1);
  const [fontScale, setFontScale] = useState(0);
  const [pasteIntervalMs, setPasteIntervalMs] = useState(250);
  const [expandedImageMessageIds, setExpandedImageMessageIds] = useState<number[]>([]);
  const [mediaTextDraft, setMediaTextDraft] = useState("");
  const [mediaFiles, setMediaFiles] = useState<File[]>([]);
  const [mediaPreviewImages, setMediaPreviewImages] = useState<PreviewImageItem[]>([]);
  const [mediaInputKey, setMediaInputKey] = useState(0);
  const [creatingMediaMessage, setCreatingMediaMessage] = useState(false);
  const [createMediaError, setCreateMediaError] = useState<string | null>(null);
  const [createMediaResult, setCreateMediaResult] =
    useState<LegacyCreateTextMessageResult | null>(null);
  const [editingMessage, setEditingMessage] = useState<LegacyMessage | null>(null);
  const [editTextDraft, setEditTextDraft] = useState("");
  const [editFiles, setEditFiles] = useState<File[]>([]);
  const [editPreviewImages, setEditPreviewImages] = useState<PreviewImageItem[]>([]);
  const [editInputKey, setEditInputKey] = useState(0);
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
  const [loadingImportQueueMessageId, setLoadingImportQueueMessageId] =
    useState<number | null>(null);
  const [importQueueError, setImportQueueError] = useState<string | null>(null);
  const [importQueuePreview, setImportQueuePreview] =
    useState<LegacyImportQueuePreview | null>(null);
  const [pastingImportQueue, setPastingImportQueue] = useState(false);
  const [importQueuePasteAllError, setImportQueuePasteAllError] = useState<string | null>(null);
  const [importQueuePasteAllResult, setImportQueuePasteAllResult] =
    useState<ImportQueuePasteAllResult | null>(null);
  const [archiveAfterImport, setArchiveAfterImport] = useState(false);
  const [importQueuePasteArchiveResult, setImportQueuePasteArchiveResult] =
    useState<ImportQueuePasteArchiveResult | null>(null);
  const [showComposer, setShowComposer] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [alwaysOnTop, setAlwaysOnTop] = useState(false);
  const [closeToTray, setCloseToTray] = useState(true);
  const [topmostError, setTopmostError] = useState<string | null>(null);
  const [openPathError, setOpenPathError] = useState<string | null>(null);
  const [startup, setStartup] = useState(false);
  const [startupError, setStartupError] = useState<string | null>(null);
  const [settingsNotice, setSettingsNotice] = useState<string | null>(null);
  const [settingsError, setSettingsError] = useState<string | null>(null);
  const [checkingUpdate, setCheckingUpdate] = useState(false);
  const [releaseCheckResult, setReleaseCheckResult] =
    useState<ReleaseCheckResult | null>(null);
  const [releaseCheckError, setReleaseCheckError] = useState<string | null>(null);
  const [globalShortcutErrors, setGlobalShortcutErrors] = useState<string[]>([]);
  const [migratingLegacyData, setMigratingLegacyData] = useState(false);
  const [migrationResult, setMigrationResult] = useState<AppMigrationResult | null>(null);
  const [migrationError, setMigrationError] = useState<string | null>(null);
  const messageListRef = useRef<HTMLElement | null>(null);
  const pendingMessageListScrollTopRef = useRef<number | null>(null);

  useEffect(() => {
    document.title = APP_TITLE;
  }, []);

  useEffect(() => {
    let alive = true;

    setError(null);

    loadAppData(view, sort)
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
    return () => {
      alive = false;
    };
  }, [view, sort]);

  useLayoutEffect(() => {
    const pendingScrollTop = pendingMessageListScrollTopRef.current;
    if (pendingScrollTop === null || !page) return;

    pendingMessageListScrollTopRef.current = null;
    if (messageListRef.current) {
      messageListRef.current.scrollTop = pendingScrollTop;
    }
  }, [page]);

  useEffect(() => {
    if (!previewImage) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setPreviewImage(null);
      } else if (event.key === "ArrowLeft") {
        setPreviewImage((current) => shiftPreviewImage(current, -1));
      } else if (event.key === "ArrowRight") {
        setPreviewImage((current) => shiftPreviewImage(current, 1));
      }
    };

    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [previewImage]);

  useEffect(() => {
    let alive = true;

    Promise.all(
      mediaFiles.map(async (file, index) => ({
        filename: file.name,
        path: `composer:${index}:${file.name}:${file.size}`,
        src: await fileToDataUrl(file),
      })),
    ).then((previewImages) => {
      if (alive) setMediaPreviewImages(previewImages);
    });

    return () => {
      alive = false;
    };
  }, [mediaFiles]);

  useEffect(() => {
    let alive = true;

    Promise.all(
      editFiles.map(async (file, index) => ({
        filename: file.name,
        path: `edit:${index}:${file.name}:${file.size}`,
        src: await fileToDataUrl(file),
      })),
    ).then((previewImages) => {
      if (alive) setEditPreviewImages(previewImages);
    });

    return () => {
      alive = false;
    };
  }, [editFiles]);

  useEffect(() => {
    if (!copyError && !copyResult && !copyImageError && !copyImageResult) return;

    const timer = window.setTimeout(() => {
      clearCopyFeedback();
    }, 2400);

    return () => window.clearTimeout(timer);
  }, [copyError, copyResult, copyImageError, copyImageResult]);

  useEffect(() => {
    if (
      !importQueueError &&
      !importQueuePreview &&
      !importQueuePasteAllError &&
      !importQueuePasteAllResult
    ) {
      return;
    }

    const timer = window.setTimeout(() => {
      clearImportFeedback();
    }, 2400);

    return () => window.clearTimeout(timer);
  }, [importQueueError, importQueuePreview, importQueuePasteAllError, importQueuePasteAllResult]);

  useEffect(() => {
    if (!createMediaResult && !deleteResult && !archiveError && !archiveResult) return;

    const timer = window.setTimeout(() => {
      clearWriteFeedback();
    }, 2400);

    return () => window.clearTimeout(timer);
  }, [archiveError, archiveResult, createMediaResult, deleteResult]);

  useEffect(() => {
    let alive = true;

    getAppSettings()
      .then(async (settings) => {
        const migratedPatch = readLegacyLocalSettingsPatch();
        const actualSettings =
          Object.keys(migratedPatch).length > 0
            ? await updateAppSettings(migratedPatch)
            : settings;
        if (!alive) return;
        applyAppSettings(actualSettings);
        clearLegacyLocalSettings();
        setSettingsError(null);
      })
      .catch((err: unknown) => {
        if (alive) setSettingsError(err instanceof Error ? err.message : String(err));
      });

    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;

    getCurrentWindow()
      .isAlwaysOnTop()
      .then((isTopmost) => {
        if (alive) setAlwaysOnTop(isTopmost);
      })
      .catch((err: unknown) => {
        if (alive) setTopmostError(err instanceof Error ? err.message : String(err));
      });

    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    getLaunchOnStartup()
      .then((enabled) => {
        if (alive) {
          setStartup(enabled);
          setStartupError(null);
        }
      })
      .catch((err: unknown) => {
        if (alive) setStartupError(err instanceof Error ? err.message : String(err));
      });

    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    if (!showSettings) return;

    let alive = true;
    getGlobalShortcutErrors()
      .then((errors) => {
        if (alive) setGlobalShortcutErrors(errors);
      })
      .catch((err: unknown) => {
        if (alive) {
          setGlobalShortcutErrors([err instanceof Error ? err.message : String(err)]);
        }
      });

    return () => {
      alive = false;
    };
  }, [showSettings]);

  useEffect(() => {
    const handleKeyDown = async (event: KeyboardEvent) => {
      if (
        event.defaultPrevented ||
        showComposer ||
        showSettings ||
        editingMessage ||
        deletingMessage ||
        previewImage
      ) {
        return;
      }
      if (!isMainPasteShortcut(event) || isEditableElement(event.target)) return;

      event.preventDefault();
      setCreateMediaError(null);
      try {
        const content = await readCurrentClipboard();
        if (content.kind === "text" && content.text) {
          setMediaTextDraft(content.text);
          setMediaFiles([]);
          setMediaPreviewImages([]);
          setMediaInputKey((key) => key + 1);
          setShowComposer(true);
        } else if (content.kind === "image" && content.image_data) {
          const file = new File(
            [new Uint8Array(content.image_data)],
            `clipboard-${Date.now()}.png`,
            { type: "image/png" },
          );
          setMediaTextDraft("");
          setMediaFiles([file]);
          setMediaInputKey((key) => key + 1);
          setShowComposer(true);
        }
      } catch (err: unknown) {
        setCreateMediaError(err instanceof Error ? err.message : String(err));
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [deletingMessage, editingMessage, previewImage, showComposer, showSettings]);

  function clearCopyFeedback() {
    setCopyError(null);
    setCopyResult(null);
    setCopyImageError(null);
    setCopyImageResult(null);
  }

  function clearImportFeedback() {
    setImportQueueError(null);
    setImportQueuePreview(null);
    setImportQueuePasteAllError(null);
    setImportQueuePasteAllResult(null);
    setImportQueuePasteArchiveResult(null);
  }

  function clearWriteFeedback() {
    setCreateMediaResult(null);
    setDeleteResult(null);
    setArchiveError(null);
    setArchiveResult(null);
  }

  function applyAppSettings(settings: AppSettings) {
    setAlwaysOnTop(settings.always_on_top);
    setCloseToTray(settings.close_to_tray);
    setArchiveAfterImport(settings.archive_after_import);
    setPasteIntervalMs(settings.paste_interval_ms);
    setHoverDelay(settings.hover_delay);
    setScrollLines(settings.scroll_lines);
    setFontScale(settings.font_scale);
    setSort(settings.sort);
    getCurrentWindow()
      .setAlwaysOnTop(settings.always_on_top)
      .catch((err: unknown) => setTopmostError(err instanceof Error ? err.message : String(err)));
  }

  async function persistAppSettings(patch: AppSettingsPatch, notice = "设置已自动保存到本机") {
    setSettingsError(null);
    try {
      const settings = await updateAppSettings(patch);
      applyAppSettings(settings);
      setSettingsNotice(notice);
      window.setTimeout(() => setSettingsNotice(null), 1800);
      return settings;
    } catch (err: unknown) {
      setSettingsError(err instanceof Error ? err.message : String(err));
      throw err;
    }
  }

  async function updateLaunchOnStartup(checked: boolean) {
    const previous = startup;
    setStartup(checked);
    setStartupError(null);
    setSettingsNotice(null);

    try {
      const actual = await setLaunchOnStartup(checked);
      setStartup(actual);
      setSettingsNotice(actual ? "开机自启动已启用" : "开机自启动已关闭");
      window.setTimeout(() => setSettingsNotice(null), 1800);
    } catch (err: unknown) {
      setStartup(previous);
      setStartupError(err instanceof Error ? err.message : String(err));
    }
  }

  function toggleImageExpansion(messageId: number) {
    setExpandedImageMessageIds((ids) =>
      ids.includes(messageId)
        ? ids.filter((id) => id !== messageId)
        : [...ids, messageId],
    );
  }

  async function toggleAlwaysOnTop() {
    const nextAlwaysOnTop = !alwaysOnTop;
    setTopmostError(null);

    try {
      const settings = await persistAppSettings(
        { always_on_top: nextAlwaysOnTop },
        nextAlwaysOnTop ? "窗口置顶已启用" : "窗口置顶已关闭",
      );
      await getCurrentWindow().setAlwaysOnTop(settings.always_on_top);
    } catch (err: unknown) {
      setTopmostError(err instanceof Error ? err.message : String(err));
    }
  }

  function updateSort(nextSort: SortOrder) {
    setSort(nextSort);
    persistAppSettings({ sort: nextSort }, "消息排序已应用").catch(() => setSort(sort));
  }

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

  async function refreshAppData({ preserveListScroll = true } = {}) {
    if (preserveListScroll) {
      pendingMessageListScrollTopRef.current = messageListRef.current?.scrollTop ?? null;
    }
    const [nextStats, nextPage] = await loadAppData(view, sort);
    setStats(nextStats);
    setPage(nextPage);
  }

  async function openLocalPath(path: string) {
    setOpenPathError(null);
    try {
      await openPath(path);
    } catch (err) {
      setOpenPathError(err instanceof Error ? err.message : String(err));
    }
  }

  async function openExternalUrl(url: string) {
    setOpenPathError(null);
    try {
      await openUrl(url);
    } catch (err) {
      setOpenPathError(err instanceof Error ? err.message : String(err));
    }
  }

  async function checkForUpdates() {
    setCheckingUpdate(true);
    setReleaseCheckError(null);
    setReleaseCheckResult(null);

    try {
      const response = await fetch(GITHUB_LATEST_RELEASE_API, {
        headers: { Accept: "application/vnd.github+json" },
      });
      if (!response.ok) {
        throw new Error(`GitHub Release 检查失败：HTTP ${response.status}`);
      }
      const payload = (await response.json()) as {
        body?: string;
        html_url?: string;
        tag_name?: string;
      };
      const latestVersion = normalizeReleaseVersion(payload.tag_name ?? "");
      if (!latestVersion) throw new Error("GitHub Release 响应缺少版本号");

      setReleaseCheckResult({
        currentVersion: CURRENT_VERSION,
        latestVersion,
        releaseUrl: payload.html_url || GITHUB_RELEASES_URL,
        body: payload.body ?? "",
        hasUpdate: compareVersions(latestVersion, CURRENT_VERSION) > 0,
      });
    } catch (err: unknown) {
      setReleaseCheckError(err instanceof Error ? err.message : String(err));
    } finally {
      setCheckingUpdate(false);
    }
  }

  async function runLegacyMigration() {
    if (migratingLegacyData) return;

    setMigratingLegacyData(true);
    setMigrationError(null);
    setMigrationResult(null);

    try {
      const result = await migrateLegacyData();
      setMigrationResult(result);
      setStats(result.stats);
      const nextPage = await listLegacyMessages({
        view,
        sort,
        offset: 0,
        limit: PAGE_LIMIT,
      });
      setPage(nextPage);
      setSettingsNotice(
        result.inserted_messages > 0
          ? `已迁移 ${result.inserted_messages} 条，跳过 ${result.skipped_messages} 条重复`
          : `没有新增数据，已跳过 ${result.skipped_messages} 条重复`,
      );
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      setMigrationError(err instanceof Error ? err.message : String(err));
    } finally {
      setMigratingLegacyData(false);
    }
  }

  function selectMediaFiles(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    setMediaFiles((currentFiles) => [...currentFiles, ...files]);
    setCreateMediaError(null);
    setCreateMediaResult(null);
  }

  function pasteMediaContent(event: ClipboardEvent<HTMLTextAreaElement>) {
    const pastedFiles = Array.from(event.clipboardData.files).filter((file) =>
      file.type.startsWith("image/"),
    );
    if (pastedFiles.length === 0) return;

    event.preventDefault();
    setMediaFiles((currentFiles) => [...currentFiles, ...pastedFiles]);
    setCreateMediaError(null);
    setCreateMediaResult(null);
  }

  async function createMediaMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const text = mediaTextDraft.trim();
    if ((!text && mediaFiles.length === 0) || creatingMediaMessage) return;

    setCreatingMediaMessage(true);
    setCreateMediaError(null);
    setCreateMediaResult(null);

    try {
      const imagesData = await filesToNumberArrays(mediaFiles);
      const result =
        text && imagesData.length > 0
          ? await createLegacyMixedMessage(text, imagesData)
          : text
            ? await createLegacyTextMessage(text)
            : await createLegacyImageMessage(imagesData);
      await refreshAppData();
      setMediaTextDraft("");
      setMediaFiles([]);
      setMediaInputKey((key) => key + 1);
      setCreateMediaResult(result);
      setShowComposer(false);
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
    setEditFiles((currentFiles) => [...currentFiles, ...Array.from(event.target.files ?? [])]);
    setEditError(null);
    setEditResult(null);
  }

  function pasteEditMediaContent(event: ClipboardEvent<HTMLTextAreaElement>) {
    const pastedFiles = Array.from(event.clipboardData.files).filter((file) =>
      file.type.startsWith("image/"),
    );
    if (pastedFiles.length === 0) return;

    event.preventDefault();
    setEditFiles((currentFiles) => [...currentFiles, ...pastedFiles]);
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
      await refreshAppData();
      setEditFiles([]);
      setEditInputKey((key) => key + 1);
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
      await refreshAppData();
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
      await refreshAppData();
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
      const result = await copyLegacyMessageTextToClipboard(message.id);
      setCopyResult({
        messageId: result.message_id,
        textLength: result.text_length,
      });
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

  async function openImportQueue(message: LegacyMessage) {
    if (loadingImportQueueMessageId !== null || pastingImportQueue) return;

    setLoadingImportQueueMessageId(message.id);
    setImportQueueError(null);
    setImportQueuePreview(null);
    setImportQueuePasteAllError(null);
    setImportQueuePasteAllResult(null);
    setImportQueuePasteArchiveResult(null);

    try {
      const preview = await previewLegacyMessageImportQueue(message.id);
      setImportQueuePreview(preview);
      await pasteImportQueue(preview);
    } catch (err) {
      setImportQueueError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoadingImportQueueMessageId(null);
    }
  }

  async function pasteImportQueue(preview: LegacyImportQueuePreview) {
    if (pastingImportQueue) return;

    setPastingImportQueue(true);
    setImportQueuePasteAllError(null);
    setImportQueuePasteAllResult(null);
    setImportQueuePasteArchiveResult(null);

    try {
      const result = await pasteLegacyImportQueueToRecentWindow({
        messageId: preview.message_id,
        delayMs: pasteIntervalMs,
        archiveAfterSuccess: archiveAfterImport,
      });
      setImportQueuePasteAllResult(result.paste);
      setImportQueuePasteArchiveResult(result);
      if (result.archive_result) {
        await refreshAppData();
      }
    } catch (err) {
      setImportQueuePasteAllError(err instanceof Error ? err.message : String(err));
    } finally {
      setPastingImportQueue(false);
    }
  }

  const canCreateMedia =
    (mediaTextDraft.trim().length > 0 || mediaFiles.length > 0) &&
    !creatingMediaMessage;
  const editText = editTextDraft.trim();
  const editWillHaveImages = (editingMessage?.images.length ?? 0) > 0 || editFiles.length > 0;
  const editHasContent = editText.length > 0 || editWillHaveImages;
  const canSaveEdit =
    !!editingMessage &&
    !savingEdit &&
    editHasContent &&
    (((editingMessage.text_content ?? null) !==
      (editText.length > 0 ? editText : null)) ||
      editFiles.length > 0);
  return (
    <main className="shell" style={{ fontSize: `${14 + fontScale}px` }}>
      <header className="app-topbar">
        <div className="brand-block">
          <span className="app-icon" aria-hidden="true">
            C
          </span>
          <div>
            <h1>需求暂存站</h1>
            <p>v{CURRENT_VERSION} @linjianglu</p>
          </div>
        </div>

        <nav className="top-actions" aria-label="应用操作">
          <button
            type="button"
            className={alwaysOnTop ? "active" : ""}
            onClick={toggleAlwaysOnTop}
            title={topmostError ? `置顶失败：${topmostError}` : undefined}
          >
            {alwaysOnTop ? "已置顶" : "置顶"}
          </button>
          <button type="button" onClick={() => setShowSettings(true)}>设置</button>
          <button
            type="button"
            className="primary-new"
            onClick={() => setShowComposer(true)}
          >
            + 新建
          </button>
        </nav>
      </header>

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
          {showComposer && (
            <div className="preview-backdrop edit-backdrop" role="presentation" onClick={() => setShowComposer(false)}>
              <section
                className="edit-dialog composer-dialog"
                role="dialog"
                aria-label="编辑新消息"
                aria-modal="true"
                onClick={(event) => event.stopPropagation()}
              >
                <header className="edit-header">
                  <div>
                    <p className="eyebrow">新消息</p>
                    <h2>编辑消息</h2>
                  </div>
                  <button type="button" className="preview-close" onClick={() => setShowComposer(false)} aria-label="关闭新消息">
                    ×
                  </button>
                </header>
                <form className="text-create-form" onSubmit={createMediaMessage}>
                  <section className="message-composer-box">
                    <textarea
                      id="new-media-message-text"
                      aria-label="消息内容"
                      value={mediaTextDraft}
                      onChange={(event) => setMediaTextDraft(event.target.value)}
                      onPaste={pasteMediaContent}
                      placeholder="输入文字，或直接粘贴图片"
                      rows={6}
                    />
                    {mediaFiles.length > 0 && (
                      <div className="composer-image-grid" aria-label="已选图片">
                        {mediaFiles.map((file, index) => (
                          <ComposerImageTile
                            file={file}
                            index={index}
                            key={`${file.name}-${file.size}-${index}`}
                            onPreview={setPreviewImage}
                            previewDelaySeconds={hoverDelay}
                            previewImages={mediaPreviewImages}
                          />
                        ))}
                      </div>
                    )}
                    <div className="composer-file-row">
                      <label className="composer-file-action" htmlFor="new-media-message-files">
                        选择图片
                      </label>
                      <span>
                        {mediaFiles.length > 0
                          ? `已选择 ${mediaFiles.length} 张图片`
                          : "可粘贴图片或选择文件"}
                      </span>
                      <input
                        key={mediaInputKey}
                        id="new-media-message-files"
                        className="composer-file-input"
                        type="file"
                        accept="image/*"
                        multiple
                        onChange={selectMediaFiles}
                      />
                    </div>
                  </section>

                  <div className="dialog-actions">
                    <button type="submit" className="write-submit" disabled={!canCreateMedia}>
                      {creatingMediaMessage ? "正在保存..." : "保存"}
                    </button>
                    <button type="button" className="secondary-action" onClick={() => setShowComposer(false)}>
                      取消
                    </button>
                  </div>
                </form>

                {createMediaError && (
                  <OperationFeedback variant="error" title="写入失败">
                    <p>{createMediaError}</p>
                  </OperationFeedback>
                )}

                {createMediaResult && (
                  <OperationFeedback variant="success" title={`已保存 #${createMediaResult.message.id}`}>
                    <p>
                      {createMediaResult.message.created_at} · 图片{" "}
                      {createMediaResult.message.images.length}
                    </p>
                  </OperationFeedback>
                )}
              </section>
            </div>
          )}

          <section className="toolbar" aria-label="消息列表控制">
            <div className="segmented" role="tablist" aria-label="消息视图">
              <button
                type="button"
                aria-label="普通"
                className={view === "normal" ? "active" : ""}
                onClick={() => setView("normal")}
              >
                <span>普通</span>
                <small>{stats.normal_count} 条消息</small>
              </button>
              <button
                type="button"
                aria-label="已归档"
                className={view === "archived" ? "active" : ""}
                onClick={() => setView("archived")}
              >
                <span>已归档</span>
                <small>{stats.archived_count} 条消息</small>
              </button>
            </div>

            <div className="segmented" aria-label="排序">
              <button
                type="button"
                className={sort === "newest" ? "active" : ""}
                onClick={() => updateSort("newest")}
              >
                最新
              </button>
              <button
                type="button"
                className={sort === "oldest" ? "active" : ""}
                onClick={() => updateSort("oldest")}
              >
                最早
              </button>
            </div>
          </section>

          {page && (
            <MessageList
              listRef={messageListRef}
              messages={page.messages}
              archivingMessageId={archivingMessageId}
              expandedImageMessageIds={expandedImageMessageIds}
              importingMessageId={loadingImportQueueMessageId}
              onDelete={openDeleteMessage}
              onEdit={openEditMessage}
              onArchive={toggleArchiveMessage}
              onCopyImage={copyMessageImage}
              onCopyText={copyMessageText}
              onToggleImages={toggleImageExpansion}
              onOpenImportQueue={openImportQueue}
              onPreview={setPreviewImage}
              previewDelaySeconds={hoverDelay}
              scrollLines={scrollLines}
              hasMore={page.has_more}
              loadingMore={loadingMore}
              onLoadMore={loadMore}
              onBlankDoubleClick={() => setShowComposer(true)}
            />
          )}

          {page && page.messages.length === 0 && (
            <button
              type="button"
              className="empty empty-create-target"
              onDoubleClick={() => setShowComposer(true)}
            >
              当前视图没有消息。双击空白处创建。
            </button>
          )}
        </>
      )}

      {previewImage && (
        <HoverImagePreview image={previewImage} />
      )}

      {editingMessage && (
        <EditMessageDialog
          error={editError}
          files={editFiles}
          inputKey={editInputKey}
          message={editingMessage}
          previewDelaySeconds={hoverDelay}
          previewImages={editPreviewImages}
          result={editResult}
          saving={savingEdit}
          textDraft={editTextDraft}
          canSave={canSaveEdit}
          onClose={closeEditMessage}
          onFileChange={selectEditFiles}
          onPaste={pasteEditMediaContent}
          onPreview={setPreviewImage}
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

      {showSettings && stats && (
        <SettingsDialog
          archiveAfterImport={archiveAfterImport}
          checkingUpdate={checkingUpdate}
          closeToTray={closeToTray}
          fontScale={fontScale}
          globalShortcutErrors={globalShortcutErrors}
          hoverDelay={hoverDelay}
          openPathError={openPathError}
          pasteIntervalMs={pasteIntervalMs}
          releaseCheckError={releaseCheckError}
          releaseCheckResult={releaseCheckResult}
          scrollLines={scrollLines}
          settingsError={settingsError}
          settingsNotice={settingsNotice}
          sort={sort}
          stats={stats}
          startup={startup}
          startupError={startupError}
          migrationError={migrationError}
          migrationResult={migrationResult}
          migratingLegacyData={migratingLegacyData}
          onArchiveAfterImportChange={(checked) => {
            setArchiveAfterImport(checked);
            persistAppSettings({ archive_after_import: checked }).catch(() => undefined);
          }}
          onCloseToTrayChange={(checked) => {
            setCloseToTray(checked);
            persistAppSettings(
              { close_to_tray: checked },
              checked ? "关闭窗口时将隐藏到托盘" : "关闭窗口时将退出应用",
            ).catch(() => undefined);
          }}
          onClose={() => setShowSettings(false)}
          onCheckUpdates={checkForUpdates}
          onFontScaleChange={(value) => {
            setFontScale(value);
            persistAppSettings({ font_scale: value }).catch(() => undefined);
          }}
          onHoverDelayChange={(value) => {
            setHoverDelay(value);
            persistAppSettings({ hover_delay: value }).catch(() => undefined);
          }}
          onOpenReleasePage={openExternalUrl}
          onOpenPath={openLocalPath}
          onMigrateLegacyData={runLegacyMigration}
          onPasteIntervalChange={(value) => {
            setPasteIntervalMs(value);
            persistAppSettings({ paste_interval_ms: value }).catch(() => undefined);
          }}
          onScrollLinesChange={(value) => {
            setScrollLines(value);
            persistAppSettings({ scroll_lines: value }).catch(() => undefined);
          }}
          onSettingsNotice={setSettingsNotice}
          onSortChange={updateSort}
          onStartupChange={setStartup}
          onStartupPersistChange={updateLaunchOnStartup}
        />
      )}

      {deleteResult && (
        <OperationFeedback
          dismissLabel="关闭删除提示"
          onDismiss={clearWriteFeedback}
          surface="floating"
          variant="success"
          title={`已删除 #${deleteResult.message.id}`}
        >
          <p>消息已移除。</p>
        </OperationFeedback>
      )}

      {createMediaResult && (
        <OperationFeedback
          dismissLabel="关闭写入提示"
          onDismiss={clearWriteFeedback}
          surface="floating"
          variant="success"
          title={`已保存 #${createMediaResult.message.id}`}
        >
          <p>
            {createMediaResult.message.created_at} · 图片{" "}
            {createMediaResult.message.images.length}
          </p>
        </OperationFeedback>
      )}

      {(archiveError || archiveResult) && (
        <OperationFeedback
          dismissLabel="关闭归档提示"
          onDismiss={clearWriteFeedback}
          surface="floating"
          variant={archiveError ? "error" : "success"}
          title={
            archiveError
              ? "归档操作失败"
              : archiveResult
                ? `${archiveResult.message.archived ? "已归档" : "已恢复"} #${archiveResult.message.id}`
                : ""
          }
        >
          {archiveError ? (
            <p>{archiveError}</p>
          ) : (
            archiveResult && (
              <>
                <p>{archiveResult.message.archived ? "消息已移入归档。" : "消息已恢复到普通列表。"}</p>
              </>
            )
          )}
        </OperationFeedback>
      )}

      {(copyError || copyResult) && (
        <OperationFeedback
          dismissLabel="关闭复制提示"
          onDismiss={clearCopyFeedback}
          surface="floating"
          variant={copyError ? "error" : "success"}
          title={copyError ? "复制失败" : copyResult ? `已复制 #${copyResult.messageId}` : ""}
        >
          {copyError ? (
            <p>{copyError}</p>
          ) : (
            copyResult && (
              <p>{copyResult.textLength} 个字符</p>
            )
          )}
        </OperationFeedback>
      )}

      {(copyImageError || copyImageResult) && (
        <OperationFeedback
          dismissLabel="关闭图片复制提示"
          onDismiss={clearCopyFeedback}
          surface="floating"
          variant={copyImageError ? "error" : "success"}
          title={copyImageError ? "复制图片失败" : "已复制图片"}
        >
          {copyImageError ? (
            <p>{copyImageError}</p>
          ) : (
            copyImageResult && (
              <p>
                {copyImageResult.filename} · {copyImageResult.width} ×{" "}
                {copyImageResult.height}
              </p>
            )
          )}
        </OperationFeedback>
      )}

      {(importQueueError || importQueuePreview) && (
        <OperationFeedback
          className="import-queue-result"
          dismissLabel="关闭导入提示"
          onDismiss={clearImportFeedback}
          surface="floating"
          variant={importQueueError ? "error" : "success"}
          title={
            importQueueError
              ? "导入读取失败"
              : importQueuePreview
                ? `导入 #${importQueuePreview.message_id}`
                : ""
          }
        >
          {importQueueError ? (
            <p>{importQueueError}</p>
          ) : (
            importQueuePreview && (
              <>
                <p>
                  {pastingImportQueue
                    ? "正在自动导入到上一个外部窗口。"
                    : "将按旧版顺序导入：文字先行，随后依次导入图片。"}
                </p>
                <p>
                  已准备 {importQueuePreview.item_count} 项，目标窗口使用最近一次激活的外部窗口。
                </p>
              </>
            )
          )}
        </OperationFeedback>
      )}

      {(importQueuePasteAllError || importQueuePasteAllResult) && (
        <OperationFeedback
          dismissLabel="关闭导入结果"
          onDismiss={clearImportFeedback}
          surface="floating"
          variant={importQueuePasteAllError ? "error" : "success"}
          title={
            importQueuePasteAllError
              ? "导入失败"
              : importQueuePasteAllResult
                ? `已导入 #${importQueuePasteAllResult.message_id} · ${importQueuePasteAllResult.completed_count} 项`
                : ""
          }
        >
          {importQueuePasteAllError ? (
            <p>{importQueuePasteAllError}</p>
          ) : (
            importQueuePasteAllResult && (
              <>
                <p>
                  {importQueuePasteAllResult.failure
                    ? `停在第 ${
                        (importQueuePasteAllResult.failed_item_index ?? 0) + 1
                      } 项：${importQueuePasteAllResult.failure}`
                    : `已发送到 ${importQueuePasteAllResult.target.title}，间隔 ${importQueuePasteAllResult.requested_delay_ms}ms`}
                </p>
                {importQueuePasteArchiveResult?.archive_result && (
                  <p>
                    导入后已自动归档。
                  </p>
                )}
                {importQueuePasteArchiveResult?.archive_error && (
                  <p>归档失败：{importQueuePasteArchiveResult.archive_error}</p>
                )}
              </>
            )
          )}
        </OperationFeedback>
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

function OperationFeedback({
  children,
  className = "",
  dismissLabel,
  onDismiss,
  surface = "inline",
  title,
  variant,
}: {
  children?: ReactNode;
  className?: string;
  dismissLabel?: string;
  onDismiss?: () => void;
  surface?: "inline" | "floating";
  title: string;
  variant: "error" | "success";
}) {
  const classes = [
    "operation-feedback",
    `operation-feedback-${variant}`,
    `operation-feedback-${surface}`,
    className,
  ]
    .filter(Boolean)
    .join(" ");

  return (
    <section
      className={classes}
      role={variant === "error" ? "alert" : "status"}
      onClick={onDismiss}
    >
      <strong>{title}</strong>
      {children}
      {onDismiss && (
        <button
          type="button"
          className="feedback-dismiss"
          aria-label={dismissLabel ?? "关闭提示"}
          onClick={(event) => {
            event.stopPropagation();
            onDismiss();
          }}
        >
          ×
        </button>
      )}
    </section>
  );
}

function SettingsDialog({
  archiveAfterImport,
  checkingUpdate,
  closeToTray,
  fontScale,
  globalShortcutErrors,
  hoverDelay,
  migrationError,
  migrationResult,
  migratingLegacyData,
  openPathError,
  pasteIntervalMs,
  releaseCheckError,
  releaseCheckResult,
  scrollLines,
  settingsError,
  settingsNotice,
  sort,
  stats,
  startup,
  startupError,
  onArchiveAfterImportChange,
  onCheckUpdates,
  onClose,
  onCloseToTrayChange,
  onFontScaleChange,
  onHoverDelayChange,
  onMigrateLegacyData,
  onOpenReleasePage,
  onOpenPath,
  onPasteIntervalChange,
  onScrollLinesChange,
  onSettingsNotice,
  onSortChange,
  onStartupChange,
  onStartupPersistChange,
}: {
  archiveAfterImport: boolean;
  checkingUpdate: boolean;
  closeToTray: boolean;
  fontScale: number;
  globalShortcutErrors: string[];
  hoverDelay: number;
  migrationError: string | null;
  migrationResult: AppMigrationResult | null;
  migratingLegacyData: boolean;
  openPathError: string | null;
  pasteIntervalMs: number;
  releaseCheckError: string | null;
  releaseCheckResult: ReleaseCheckResult | null;
  scrollLines: number;
  settingsError: string | null;
  settingsNotice: string | null;
  sort: SortOrder;
  stats: LegacyStats;
  startup: boolean;
  startupError: string | null;
  onArchiveAfterImportChange: (checked: boolean) => void;
  onCheckUpdates: () => void;
  onClose: () => void;
  onCloseToTrayChange: (checked: boolean) => void;
  onFontScaleChange: (value: number) => void;
  onHoverDelayChange: (value: number) => void;
  onMigrateLegacyData: () => void;
  onOpenReleasePage: (url: string) => void;
  onOpenPath: (path: string) => void;
  onPasteIntervalChange: (value: number) => void;
  onScrollLinesChange: (value: number) => void;
  onSettingsNotice: (message: string | null) => void;
  onSortChange: (sort: SortOrder) => void;
  onStartupChange: (checked: boolean) => void;
  onStartupPersistChange: (checked: boolean) => void;
}) {
  function showAutoSavedNotice(message = "设置已自动保存到本机") {
    onSettingsNotice(message);
    window.setTimeout(() => onSettingsNotice(null), 1800);
  }

  function changeHoverDelay(value: number) {
    onHoverDelayChange(value);
    onSettingsNotice(null);
    showAutoSavedNotice();
  }

  function changeScrollLines(value: number) {
    onScrollLinesChange(value);
    onSettingsNotice(null);
    showAutoSavedNotice();
  }

  function changeFontScale(value: number) {
    onFontScaleChange(value);
    onSettingsNotice(null);
    showAutoSavedNotice();
  }

  function changeSort(nextSort: SortOrder) {
    onSortChange(nextSort);
    showAutoSavedNotice("消息排序已应用");
  }

  function changeArchiveAfterImport(checked: boolean) {
    onArchiveAfterImportChange(checked);
    showAutoSavedNotice();
  }

  function changeCloseToTray(checked: boolean) {
    onCloseToTrayChange(checked);
    showAutoSavedNotice(checked ? "关闭窗口时将隐藏到托盘" : "关闭窗口时将退出应用");
  }

  function changePasteInterval(value: number) {
    onPasteIntervalChange(value);
    showAutoSavedNotice();
  }

  function changeStartup(checked: boolean) {
    onStartupChange(checked);
    onStartupPersistChange(checked);
  }

  return (
    <div className="preview-backdrop edit-backdrop" role="presentation" onClick={onClose}>
      <section
        aria-label="设置"
        aria-modal="true"
        className="edit-dialog settings-dialog"
        role="dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="edit-header">
          <div>
            <p className="eyebrow">设置</p>
            <h2>设置</h2>
          </div>
          <button type="button" className="preview-close" onClick={onClose} aria-label="关闭设置">
            ×
          </button>
        </header>

        <div className="settings-body">
          <section className="settings-form" aria-label="应用设置">
            <SettingSlider
              label="悬浮预览延迟"
              max={2}
              min={0}
              step={0.1}
              suffix="秒"
              value={hoverDelay}
              onChange={changeHoverDelay}
            />
            <p>鼠标放在图片上多久后显示预览</p>

            <SettingSlider
              label="滚动速度"
              max={8}
              min={1}
              step={1}
              suffix="行"
              value={scrollLines}
              onChange={changeScrollLines}
            />
            <p>鼠标滚轮每次滚动的行数</p>

            <SettingSlider
              label="粘贴间隔"
              max={3000}
              min={50}
              step={50}
              suffix="毫秒"
              value={pasteIntervalMs}
              onChange={changePasteInterval}
            />
            <p>导入图文消息时每一项之间的等待时间</p>

            <SettingSlider
              label="应用内文字大小"
              max={4}
              min={-4}
              step={1}
              prefix={fontScale > 0 ? "+" : ""}
              value={fontScale}
              onChange={changeFontScale}
            />
            <p>调整消息列表、按钮和状态栏文字</p>

            <label className="setting-row">
              <span>消息排序</span>
              <select value={sort} onChange={(event) => changeSort(event.target.value as SortOrder)}>
                <option value="newest">最新优先</option>
                <option value="oldest">最早优先</option>
              </select>
            </label>

            <SettingToggle
              checked={archiveAfterImport}
              label="快速导入后自动归档"
              description="导入完成后自动将消息移入已归档"
              onChange={changeArchiveAfterImport}
            />

            <SettingToggle
              checked={closeToTray}
              label="关闭窗口时隐藏到托盘"
              description="关闭主窗口后应用继续驻留，托盘菜单可彻底退出"
              onChange={changeCloseToTray}
            />

            <SettingToggle
              checked={startup}
              label="开机自启动"
              description="登录 Windows 后自动启动需求暂存站"
              onChange={changeStartup}
            />
            {startupError && <p className="inline-error">{startupError}</p>}

            <label className="setting-field">
              <span>呼出界面快捷键</span>
              <input value="<ctrl>+<shift>+v" readOnly />
            </label>

            <label className="setting-field">
              <span>导入当前剪切板快捷键</span>
              <input value="<ctrl>+<alt>+v" readOnly />
            </label>

            {globalShortcutErrors.map((error) => (
              <p className="inline-error" key={error}>
                {error}
              </p>
            ))}
            {settingsError && <p className="inline-error">{settingsError}</p>}

            <button
              type="button"
              className="check-update-action"
              onClick={onCheckUpdates}
              disabled={checkingUpdate}
            >
              {checkingUpdate ? "检查中..." : "检查更新"}
            </button>

            {releaseCheckResult && (
              <div className="settings-notice">
                <p>
                  {releaseCheckResult.hasUpdate
                    ? `发现新版本 ${releaseCheckResult.latestVersion}`
                    : `当前已是最新版本 ${releaseCheckResult.currentVersion}`}
                </p>
                {releaseCheckResult.body && <p>{releaseCheckResult.body}</p>}
                <button
                  type="button"
                  className="link-button"
                  onClick={() => onOpenReleasePage(releaseCheckResult.releaseUrl)}
                >
                  打开 Release 页面
                </button>
              </div>
            )}
            {releaseCheckError && (
              <div className="settings-notice">
                <p className="inline-error">{releaseCheckError}</p>
                <button
                  type="button"
                  className="link-button"
                  onClick={() => onOpenReleasePage(GITHUB_RELEASES_URL)}
                >
                  打开 Release 页面
                </button>
              </div>
            )}

            {settingsNotice && <p className="settings-notice">{settingsNotice}</p>}
          </section>

          <section className="paths settings-safety" aria-label="本地存储">
            <h3>本地存储</h3>
            <PathRow label="数据目录" value={stats.data_dir} ok={stats.db_exists} />
            <PathRow label="数据库" value={stats.db_path} ok={stats.db_exists} />
            <PathRow label="图片目录" value={stats.images_dir} ok={stats.images_dir_exists} />

            <div className="safety-actions">
              <button type="button" onClick={onMigrateLegacyData} disabled={migratingLegacyData}>
                {migratingLegacyData ? "迁移中..." : "迁移旧数据"}
              </button>
              <button type="button" onClick={() => onOpenPath(stats.data_dir)}>
                打开数据目录
              </button>
              <button type="button" onClick={() => onOpenPath(stats.images_dir)}>
                打开图片目录
              </button>
            </div>

            {migrationResult && (
              <p className="settings-notice">
                新增 {migrationResult.inserted_messages} 条，跳过{" "}
                {migrationResult.skipped_messages} 条重复，复制图片{" "}
                {migrationResult.copied_images} 张。
              </p>
            )}
            {migrationError && <p className="inline-error">迁移失败：{migrationError}</p>}
            {openPathError && <p className="inline-error">打开目录失败：{openPathError}</p>}
          </section>
        </div>

      </section>
    </div>
  );
}

function SettingSlider({
  label,
  max,
  min,
  onChange,
  prefix = "",
  step,
  suffix = "",
  value,
}: {
  label: string;
  max: number;
  min: number;
  onChange: (value: number) => void;
  prefix?: string;
  step: number;
  suffix?: string;
  value: number;
}) {
  return (
    <label className="setting-slider">
      <span>{label}</span>
      <strong>
        {prefix}
        {value}
        {suffix && ` ${suffix}`}
      </strong>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
      />
    </label>
  );
}

function SettingToggle({
  checked,
  description,
  label,
  onChange,
}: {
  checked: boolean;
  description: string;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="setting-toggle">
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span>
        <strong>{label}</strong>
        <small>{description}</small>
      </span>
    </label>
  );
}

function MessageList({
  archivingMessageId,
  expandedImageMessageIds,
  hasMore,
  importingMessageId,
  listRef,
  loadingMore,
  messages,
  onArchive,
  onCopyImage,
  onCopyText,
  onDelete,
  onEdit,
  onLoadMore,
  onOpenImportQueue,
  onBlankDoubleClick,
  onToggleImages,
  onPreview,
  previewDelaySeconds,
  scrollLines,
}: {
  archivingMessageId: number | null;
  expandedImageMessageIds: number[];
  hasMore: boolean;
  importingMessageId: number | null;
  listRef: RefObject<HTMLElement | null>;
  loadingMore: boolean;
  messages: LegacyMessage[];
  onArchive: (message: LegacyMessage) => void;
  onCopyImage: (image: LegacyMessageImage) => void;
  onCopyText: (message: LegacyMessage) => void;
  onDelete: (message: LegacyMessage) => void;
  onEdit: (message: LegacyMessage) => void;
  onLoadMore: () => void;
  onOpenImportQueue: (message: LegacyMessage) => void;
  onBlankDoubleClick: () => void;
  onToggleImages: (messageId: number) => void;
  onPreview: (image: PreviewImage | null) => void;
  previewDelaySeconds: number;
  scrollLines: number;
}) {
  function requestMoreIfNearBottom(element: HTMLElement) {
    if (!hasMore || loadingMore) return;

    const remaining = element.scrollHeight - element.scrollTop - element.clientHeight;
    if (remaining <= 160) {
      onLoadMore();
    }
  }

  function handleWheel(event: WheelEvent<HTMLElement>) {
    if (!listRef.current) return;
    if (scrollLines === 1) {
      window.setTimeout(() => {
        if (listRef.current) requestMoreIfNearBottom(listRef.current);
      }, 0);
      return;
    }

    event.preventDefault();
    listRef.current.scrollTop += event.deltaY * scrollLines;
    requestMoreIfNearBottom(listRef.current);
  }

  return (
    <section
      className="message-list"
      aria-label="消息列表"
      ref={listRef}
      onScroll={(event) => requestMoreIfNearBottom(event.currentTarget)}
      onWheel={handleWheel}
      onDoubleClick={(event) => {
        if (event.target === event.currentTarget) {
          onBlankDoubleClick();
        }
      }}
    >
      {messages.map((message) => {
        const isExpanded = expandedImageMessageIds.includes(message.id);
        const visibleImages = isExpanded ? message.images : message.images.slice(0, 3);
        const hiddenImageCount = message.images.length - visibleImages.length;
        const previewImages = buildPreviewImages(message.images);

        return (
          <article className="message-card" key={message.id}>
            <header className="message-meta">
              <div className="message-meta-text">
                <div className="message-time-line">
                  <strong>#{message.id}</strong>
                  <span>{message.created_at}</span>
                </div>
                {message.archived && (
                  <span className="archived-at">
                    归档于 {message.archived_at ?? "未知时间"}
                  </span>
                )}
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
                {!message.archived && (
                  <>
                    <button
                      type="button"
                      disabled={importingMessageId !== null}
                      onClick={() => onOpenImportQueue(message)}
                    >
                      {importingMessageId === message.id ? "准备中..." : "导入"}
                    </button>
                    <button type="button" onClick={() => onEdit(message)}>
                      编辑
                    </button>
                  </>
                )}
                {message.archived && (
                  <button type="button" className="danger-action" onClick={() => onDelete(message)}>
                    删除
                  </button>
                )}
              </div>
            </header>

            {message.text_content ? (
              <button
                type="button"
                className="message-text message-text-button"
                onClick={() => onCopyText(message)}
              >
                <span className="message-text-content">{message.text_content}</span>
              </button>
            ) : message.images.length === 0 ? (
              <p className="message-text empty-text">无文字内容</p>
            ) : (
              null
            )}

            {message.images.length > 0 && (
              <section className="message-images" aria-label={`消息 ${message.id} 图片`}>
                <div className="image-grid" aria-label="图片缩略图">
                  {visibleImages.map((image) => (
                    <MessageImageTile
                      image={image}
                      key={image.id}
                      onCopy={onCopyImage}
                      onPreview={onPreview}
                      previewDelaySeconds={previewDelaySeconds}
                      previewImages={previewImages}
                    />
                  ))}
                </div>
                {message.images.length > 3 && (
                  <button
                    type="button"
                    className="image-expand-action"
                    onClick={() => onToggleImages(message.id)}
                  >
                    {isExpanded ? "收起图片" : `展开 ${hiddenImageCount} 张图片`}
                  </button>
                )}
              </section>
            )}
          </article>
        );
      })}
    </section>
  );
}

function EditMessageDialog({
  canSave,
  error,
  files,
  inputKey,
  message,
  previewDelaySeconds,
  previewImages,
  result,
  saving,
  textDraft,
  onClose,
  onFileChange,
  onPaste,
  onPreview,
  onSubmit,
  onTextChange,
}: {
  canSave: boolean;
  error: string | null;
  files: File[];
  inputKey: number;
  message: LegacyMessage;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
  result: EditResult | null;
  saving: boolean;
  textDraft: string;
  onClose: () => void;
  onFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onPaste: (event: ClipboardEvent<HTMLTextAreaElement>) => void;
  onPreview: (image: PreviewImage | null) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onTextChange: (text: string) => void;
}) {
  return (
    <div className="preview-backdrop edit-backdrop" role="presentation" onClick={onClose}>
      <section
        aria-label={`编辑消息 ${message.id}`}
        aria-modal="true"
        className="edit-dialog composer-dialog edit-message-dialog"
        role="dialog"
        onClick={(event) => event.stopPropagation()}
      >
        <header className="edit-header">
          <div>
            <p className="eyebrow">消息 #{message.id}</p>
            <h2>编辑消息</h2>
          </div>
          <button type="button" className="preview-close" onClick={onClose} aria-label="关闭编辑">
            ×
          </button>
        </header>

        <form className="text-create-form" onSubmit={onSubmit}>
          <section className="message-composer-box">
            <textarea
              id="edit-message-text"
              aria-label="消息内容"
              value={textDraft}
              onChange={(event) => onTextChange(event.target.value)}
              onPaste={onPaste}
              placeholder="编辑文字，或选择图片替换原图片"
              rows={9}
            />

            {files.length > 0 && (
              <div className="composer-image-grid" aria-label="待替换图片">
                {files.map((file, index) => (
                  <ComposerImageTile
                    file={file}
                    index={index}
                    key={`${file.name}-${file.size}-${index}`}
                    onPreview={onPreview}
                    previewDelaySeconds={previewDelaySeconds}
                    previewImages={previewImages}
                  />
                ))}
              </div>
            )}

            <div className="composer-file-row">
              <label className="composer-file-action" htmlFor="edit-message-files">
                选择图片
              </label>
              <span>
                {files.length > 0
                  ? `将替换为 ${files.length} 张图片`
                  : message.images.length > 0
                    ? `保留原有 ${message.images.length} 张图片`
                    : "可选择图片替换原图片"}
              </span>
              <input
                key={inputKey}
                id="edit-message-files"
                className="composer-file-input"
                type="file"
                accept="image/*"
                multiple
                onChange={onFileChange}
              />
            </div>
          </section>
          <div className="dialog-actions">
            <button type="submit" className="write-submit" disabled={!canSave}>
              {saving ? "正在保存..." : "保存"}
            </button>
            <button type="button" className="secondary-action" onClick={onClose}>
              关闭
            </button>
          </div>
        </form>

        {error && (
          <OperationFeedback variant="error" title="保存失败">
            <p>{error}</p>
          </OperationFeedback>
        )}

        {result && (
          <OperationFeedback variant="success" title={`已保存 #${result.message.id}`}>
            <p>消息已更新。</p>
          </OperationFeedback>
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
          这会删除这条消息和关联图片。
        </p>

        <form className="text-create-form" onSubmit={onSubmit}>
          <label className="write-confirm">
            <input
              type="checkbox"
              checked={confirmed}
              onChange={(event) => onConfirmChange(event.target.checked)}
            />
            <span>确认删除这条消息。</span>
          </label>

          <div className="dialog-actions">
            <button
              type="submit"
              className="write-submit delete-submit"
              disabled={!confirmed || deleting}
            >
              {deleting ? "正在删除..." : "删除"}
            </button>
            <button type="button" className="secondary-action" onClick={onClose}>
              取消
            </button>
          </div>
        </form>

        {error && (
          <OperationFeedback variant="error" title="删除失败">
            <p>{error}</p>
          </OperationFeedback>
        )}
      </section>
    </div>
  );
}

function ComposerImageTile({
  file,
  index,
  onPreview,
  previewDelaySeconds,
  previewImages,
}: {
  file: File;
  index: number;
  onPreview: (image: PreviewImage | null) => void;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
}) {
  const previewTimerRef = useRef<number | null>(null);
  const previewImage = previewImages[index] ?? null;

  function clearPreviewTimer() {
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
  }

  function showPreview(target: HTMLButtonElement) {
    if (!previewImage) return;

    clearPreviewTimer();
    const img = target.querySelector("img");
    const naturalWidth = img?.naturalWidth && img.naturalWidth > 0 ? img.naturalWidth : 320;
    const naturalHeight = img?.naturalHeight && img.naturalHeight > 0 ? img.naturalHeight : 240;
    const anchor = target.getBoundingClientRect();

    previewTimerRef.current = window.setTimeout(() => {
      previewTimerRef.current = null;
      showHoverPreviewWindow({
        ...previewImage,
        images: previewImages,
        index,
        position: calculatePreviewPosition(anchor, naturalWidth, naturalHeight),
        total: previewImages.length,
      }, anchor).catch(() => {
        onPreview({
          ...previewImage,
          images: previewImages,
          index,
          position: calculatePreviewPosition(anchor, naturalWidth, naturalHeight),
          total: previewImages.length,
        });
      });
    }, Math.max(0, previewDelaySeconds * 1000));
  }

  function hidePreview() {
    clearPreviewTimer();
    closeHoverPreviewWindow();
    onPreview(null);
  }

  return (
    <button
      type="button"
      className="composer-image-tile"
      onMouseEnter={(event) => showPreview(event.currentTarget)}
      onMouseLeave={hidePreview}
      onFocus={(event) => showPreview(event.currentTarget)}
      onBlur={hidePreview}
      title={`${file.name} · ${formatBytes(file.size)}`}
    >
      {previewImage && <img alt={file.name} src={previewImage.src} />}
      <span>{file.name}</span>
    </button>
  );
}

function MessageImageTile({
  image,
  onCopy,
  onPreview,
  previewDelaySeconds,
  previewImages,
}: {
  image: LegacyMessageImage;
  onCopy: (image: LegacyMessageImage) => void;
  onPreview: (image: PreviewImage | null) => void;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
}) {
  const [broken, setBroken] = useState(false);
  const previewTimerRef = useRef<number | null>(null);
  const canRenderImage = image.exists && !broken;
  const src = canRenderImage ? getAssetSrc(image.path) : "";
  const previewIndex = previewImages.findIndex((item) => item.path === image.path);

  function clearPreviewTimer() {
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
  }

  function showPreview(target: HTMLButtonElement) {
    clearPreviewTimer();
    const img = target.querySelector("img");
    const naturalWidth = img?.naturalWidth && img.naturalWidth > 0 ? img.naturalWidth : 320;
    const naturalHeight = img?.naturalHeight && img.naturalHeight > 0 ? img.naturalHeight : 240;
    const anchor = target.getBoundingClientRect();

    previewTimerRef.current = window.setTimeout(() => {
      previewTimerRef.current = null;
      const nextPreview = {
        filename: image.filename,
        images: previewImages,
        index: previewIndex >= 0 ? previewIndex : 0,
        path: image.path,
        position: calculatePreviewPosition(
          anchor,
          naturalWidth,
          naturalHeight,
        ),
        src,
        total: previewImages.length,
      };
      showHoverPreviewWindow(nextPreview, anchor).catch(() => onPreview(nextPreview));
    }, Math.max(0, previewDelaySeconds * 1000));
  }

  function hidePreview() {
    clearPreviewTimer();
    closeHoverPreviewWindow();
    onPreview(null);
  }

  if (canRenderImage && src) {
    return (
      <div className="image-tile" title={image.path}>
        <button
          type="button"
          className="image-preview-action"
          onClick={() => onCopy(image)}
          onMouseEnter={(event) => showPreview(event.currentTarget)}
          onMouseLeave={hidePreview}
          onFocus={(event) => showPreview(event.currentTarget)}
          onBlur={hidePreview}
        >
          <img
            alt={image.filename}
            loading="lazy"
            src={src}
            onError={() => setBroken(true)}
          />
        </button>
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

function HoverImagePreview({ image }: {
  image: PreviewImage;
}) {
  return (
    <aside
      className="hover-preview"
      role="tooltip"
      aria-label={image.filename}
      style={
        image.position
          ? {
              height: image.position.height,
              left: image.position.left,
              top: image.position.top,
              width: image.position.width,
            }
          : undefined
      }
    >
      <img alt={image.filename} src={image.src} />
    </aside>
  );
}

async function showHoverPreviewWindow(image: PreviewImage, anchor: DOMRect) {
  const dimensions = await loadImageDimensions(image.src);
  const position = calculateScreenPreviewPosition(anchor, dimensions.width, dimensions.height);
  const key = `clipstash.preview.${Date.now()}.${Math.random().toString(36).slice(2)}`;
  localStorage.setItem(
    key,
    JSON.stringify({
      filename: image.filename,
      src: image.src,
    }),
  );

  await closeHoverPreviewWindow();
  hoverPreviewStorageKey = key;

  const previewWindow = new WebviewWindow("image-preview", {
    alwaysOnTop: true,
    decorations: false,
    focus: false,
    height: position.height,
    resizable: false,
    skipTaskbar: true,
    title: image.filename,
    url: `/image-preview.html?key=${encodeURIComponent(key)}`,
    visible: true,
    width: position.width,
    x: position.left,
    y: position.top,
  });

  hoverPreviewWindow = previewWindow;
  previewWindow.once("tauri://destroyed", () => {
    localStorage.removeItem(key);
    if (hoverPreviewStorageKey === key) {
      hoverPreviewStorageKey = null;
    }
    if (hoverPreviewWindow === previewWindow) {
      hoverPreviewWindow = null;
    }
  });
}

async function closeHoverPreviewWindow() {
  const existingWindow = hoverPreviewWindow ?? (await WebviewWindow.getByLabel("image-preview"));
  const existingKey = hoverPreviewStorageKey;
  hoverPreviewWindow = null;
  hoverPreviewStorageKey = null;
  if (existingKey) {
    localStorage.removeItem(existingKey);
  }
  if (existingWindow) {
    await existingWindow.close().catch(() => undefined);
  }
}

function loadImageDimensions(src: string) {
  return new Promise<{ height: number; width: number }>((resolve, reject) => {
    const image = new Image();
    const fallbackTimer = window.setTimeout(() => {
      resolve({ height: 240, width: 320 });
    }, 120);
    image.onload = () => {
      window.clearTimeout(fallbackTimer);
      resolve({
        height: image.naturalHeight || 240,
        width: image.naturalWidth || 320,
      });
    };
    image.onerror = () => {
      window.clearTimeout(fallbackTimer);
      reject();
    };
    image.src = src;
  });
}

function calculateScreenPreviewPosition(
  anchor: DOMRect,
  naturalWidth: number,
  naturalHeight: number,
): PreviewPosition {
  const gap = 8;
  const screenPadding = 8;
  const screenRect = getAvailableScreenRect();
  const safeNaturalWidth = Math.max(1, naturalWidth);
  const safeNaturalHeight = Math.max(1, naturalHeight);
  const longSideRatio = Math.min(1, 1000 / Math.max(safeNaturalWidth, safeNaturalHeight));
  const screenRatio = Math.min(
    1,
    (screenRect.width - screenPadding * 2) / (safeNaturalWidth * longSideRatio),
    (screenRect.height - screenPadding * 2) / (safeNaturalHeight * longSideRatio),
  );
  const ratio = longSideRatio * screenRatio;
  const width = Math.max(1, Math.round(safeNaturalWidth * ratio));
  const height = Math.max(1, Math.round(safeNaturalHeight * ratio));
  const anchorRect = getScreenAnchorRect(anchor);
  const rightLeft = anchorRect.right + gap;
  const leftLeft = anchorRect.left - gap - width;
  const rightFits = rightLeft + width <= screenRect.right - screenPadding;
  const leftFits = leftLeft >= screenRect.left + screenPadding;
  const bottomTop = anchorRect.bottom + gap;
  const topTop = anchorRect.top - gap - height;
  const bottomFits = bottomTop + height <= screenRect.bottom - screenPadding;
  const topFits = topTop >= screenRect.top + screenPadding;
  let left = rightLeft;
  let top = anchorRect.top;

  if (rightFits || leftFits) {
    left = rightFits ? rightLeft : leftLeft;
    top = clamp(anchorRect.top, screenRect.top + screenPadding, screenRect.bottom - height - screenPadding);
  } else if (bottomFits || topFits) {
    left = clamp(anchorRect.left, screenRect.left + screenPadding, screenRect.right - width - screenPadding);
    top = bottomFits ? bottomTop : topTop;
  } else {
    const preferRight = screenRect.right - anchorRect.right >= anchorRect.left - screenRect.left;
    left = preferRight ? rightLeft : leftLeft;
    top = anchorRect.bottom + gap;
    if (top + height > screenRect.bottom - screenPadding) {
      top = anchorRect.top - gap - height;
    }
    left = clamp(left, screenRect.left + screenPadding, screenRect.right - width - screenPadding);
    top = clamp(top, screenRect.top + screenPadding, screenRect.bottom - height - screenPadding);
  }

  return { height, left, top, width };
}

function getAvailableScreenRect(): ScreenRect {
  const screenWithOrigin = window.screen as Screen & {
    availLeft?: number;
    availTop?: number;
  };
  const left = screenWithOrigin.availLeft ?? 0;
  const top = screenWithOrigin.availTop ?? 0;
  const width = screenWithOrigin.availWidth || screenWithOrigin.width || window.innerWidth;
  const height = screenWithOrigin.availHeight || screenWithOrigin.height || window.innerHeight;

  return {
    bottom: top + height,
    height,
    left,
    right: left + width,
    top,
    width,
  };
}

function getScreenAnchorRect(anchor: DOMRect): ScreenRect {
  const left = window.screenX + anchor.left;
  const top = window.screenY + anchor.top;
  const width = anchor.width;
  const height = anchor.height;

  return {
    bottom: top + height,
    height,
    left,
    right: left + width,
    top,
    width,
  };
}

function calculatePreviewPosition(
  anchor: DOMRect,
  naturalWidth: number,
  naturalHeight: number,
): PreviewPosition {
  const gap = 8;
  const screenPadding = 8;
  const viewportWidth = window.innerWidth;
  const viewportHeight = window.innerHeight;
  const safeNaturalWidth = Math.max(1, naturalWidth);
  const safeNaturalHeight = Math.max(1, naturalHeight);
  const longSideRatio = Math.min(1, 1000 / Math.max(safeNaturalWidth, safeNaturalHeight));
  const viewportRatio = Math.min(
    1,
    (viewportWidth - screenPadding * 2) / (safeNaturalWidth * longSideRatio),
    (viewportHeight - screenPadding * 2) / (safeNaturalHeight * longSideRatio),
  );
  const ratio = longSideRatio * viewportRatio;
  const width = Math.max(1, Math.round(safeNaturalWidth * ratio));
  const height = Math.max(1, Math.round(safeNaturalHeight * ratio));
  const rightLeft = anchor.right + gap;
  const leftLeft = anchor.left - gap - width;
  const rightFits = rightLeft + width <= viewportWidth - screenPadding;
  const leftFits = leftLeft >= screenPadding;
  const bottomTop = anchor.bottom + gap;
  const topTop = anchor.top - gap - height;
  const bottomFits = bottomTop + height <= viewportHeight - screenPadding;
  const topFits = topTop >= screenPadding;
  let left = rightLeft;
  let top = anchor.top;

  if (rightFits || leftFits) {
    left = rightFits ? rightLeft : leftLeft;
    top = clamp(anchor.top, screenPadding, viewportHeight - height - screenPadding);
  } else if (bottomFits || topFits) {
    left = clamp(anchor.left, screenPadding, viewportWidth - width - screenPadding);
    top = bottomFits ? bottomTop : topTop;
  } else {
    const preferRight = viewportWidth - anchor.right >= anchor.left;
    left = preferRight ? rightLeft : leftLeft;
    top = anchor.bottom + gap;
    if (top + height > viewportHeight - screenPadding) {
      top = anchor.top - gap - height;
    }
    left = clamp(left, screenPadding, viewportWidth - width - screenPadding);
    top = clamp(top, screenPadding, viewportHeight - height - screenPadding);
  }

  return { height, left, top, width };
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

function buildPreviewImages(images: LegacyMessageImage[]) {
  return images
    .filter((image) => image.exists)
    .map((image) => ({
      filename: image.filename,
      path: image.path,
      src: getAssetSrc(image.path),
    }))
    .filter((image) => image.src.length > 0);
}

function shiftPreviewImage(current: PreviewImage | null, offset: number) {
  if (!current || current.images.length === 0) return current;

  const nextIndex =
    (current.index + offset + current.images.length) % current.images.length;
  const nextImage = current.images[nextIndex];
  return {
    ...nextImage,
    images: current.images,
    index: nextIndex,
    total: current.images.length,
  };
}

function getAssetSrc(path: string) {
  try {
    return convertFileSrc(path);
  } catch {
    return "";
  }
}

function loadAppData(view: MessageView, sort: SortOrder) {
  return Promise.all([
    getLegacyStats(),
    listLegacyMessages({ view, sort, offset: 0, limit: PAGE_LIMIT }),
  ]);
}

function getStoredSort(): SortOrder {
  return localStorage.getItem("clipstash.setting.messageSort") === "oldest"
    ? "oldest"
    : "newest";
}

function readLegacyLocalSettingsPatch(): AppSettingsPatch {
  const patch: AppSettingsPatch = {};
  const sort = localStorage.getItem("clipstash.setting.messageSort");
  const autoArchive = localStorage.getItem("clipstash.setting.autoArchive");
  const hoverDelay = readStoredNumber("clipstash.setting.hoverDelay");
  const scrollLines = readStoredNumber("clipstash.setting.scrollLines");
  const fontScale = readStoredNumber("clipstash.setting.fontScale");

  if (sort === "oldest" || sort === "newest") patch.sort = sort;
  if (autoArchive === "true" || autoArchive === "false") {
    patch.archive_after_import = autoArchive === "true";
  }
  if (hoverDelay !== null) patch.hover_delay = hoverDelay;
  if (scrollLines !== null) patch.scroll_lines = scrollLines;
  if (fontScale !== null) patch.font_scale = fontScale;

  return patch;
}

function clearLegacyLocalSettings() {
  [
    "clipstash.setting.messageSort",
    "clipstash.setting.autoArchive",
    "clipstash.setting.hoverDelay",
    "clipstash.setting.scrollLines",
    "clipstash.setting.fontScale",
  ].forEach((key) => localStorage.removeItem(key));
}

function readStoredNumber(key: string) {
  const raw = localStorage.getItem(key);
  if (raw === null) return null;
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : null;
}

function normalizeReleaseVersion(value: string) {
  return value.trim().replace(/^v/i, "");
}

function compareVersions(left: string, right: string) {
  const leftParts = left.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const rightParts = right.split(".").map((part) => Number.parseInt(part, 10) || 0);
  const length = Math.max(leftParts.length, rightParts.length);

  for (let index = 0; index < length; index += 1) {
    const diff = (leftParts[index] ?? 0) - (rightParts[index] ?? 0);
    if (diff !== 0) return diff;
  }

  return 0;
}

async function filesToNumberArrays(files: File[]) {
  return Promise.all(
    files.map(async (file) => {
      const buffer = await file.arrayBuffer();
      return Array.from(new Uint8Array(buffer));
    }),
  );
}

function fileToDataUrl(file: File) {
  return new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result ?? ""));
    reader.onerror = () => reject(reader.error ?? new Error("读取图片预览失败"));
    reader.readAsDataURL(file);
  });
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function isMainPasteShortcut(event: KeyboardEvent) {
  const key = event.key.toLowerCase();
  return (event.ctrlKey && key === "v") || (event.shiftKey && event.key === "Insert");
}

function isEditableElement(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false;
  const tagName = target.tagName.toLowerCase();
  return (
    target.isContentEditable ||
    tagName === "input" ||
    tagName === "textarea" ||
    tagName === "select"
  );
}


export default App;
