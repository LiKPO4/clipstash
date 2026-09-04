import {
  type ChangeEvent,
  type ClipboardEvent,
  type DragEvent as ReactDragEvent,
  type FocusEvent,
  type FormEvent,
  type KeyboardEvent as ReactKeyboardEvent,
  type MouseEvent,
  type ReactNode,
  type TouchEvent,
  type RefObject,
  memo,
  useCallback,
  useMemo,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
} from "react";
import { useVirtualizer, defaultRangeExtractor } from "@tanstack/react-virtual";
import { DataTransferProgress } from "./transferProgress";
import { importZipFile } from "./zipFileUpload";
import { useCommittedCallback } from "./useCommittedCallback";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { showDesktopPreview, hideDesktopPreview } from "./desktopPreview";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";
import { formatLocalTime } from "./formatTime";
import { ThumbnailProvider, useThumbnail } from "./useThumbnail";
import {
  archiveExportedMessages,
  copyLegacyMessageTextToClipboard,
  createLegacyImageMessage,
  createLegacyMixedMessage,
  createLegacyTextMessage,
  deleteLegacyMessage,
  downloadAndOpenUpdateInstaller,
  exportNormalDataZip,
  exportNormalDataZipBytes,
  exportNormalDataZipFile,
  fetchLatestGithubRelease,
  getAppSettings,
  getGlobalShortcutErrors,
  getLegacyMessage,
  getLegacyStats,
  getLaunchOnStartup,
  importDataZipFromPath,
  importAndroidShare,
  listLegacyMessages,
  migrateLegacyData,
  moveAppDataToSelectedDir,
  openAppPath,
  pasteLegacyImportQueueToRecentWindow,
  previewDataZip,
  readCurrentClipboard,
  readDroppedFileBytes,
  readLegacyImageBytes,
  repairAppDataDir,
  replaceLegacyMessageImages,
  setLegacyMessageArchived,
  setLaunchOnStartup,
  splitLegacyMessage,
  previewLegacyMessageImportQueue,
  updateLegacyMessageText,
  updateAppSettings,
} from "./api/legacy";
import type {
  AppSettings,
  AppSettingsPatch,
  DataExportResult,
  DataImportPreview,
  DataImportResult,
  GithubReleaseInfo,
  LegacyMessageImage,
  LegacyMessage,
  LegacyMessagePage,
  LegacyStats,
  LegacyArchiveMessageResult,
  MessageDoubleClickAction,
  AppMigrationResult,
  LegacyCreateTextMessageResult,
  LegacyImportQueuePasteArchiveResult,
  LegacyImportQueuePasteResult,
  LegacyImportQueuePreview,
  LegacyReplaceImagesResult,
  MessageView,
  SortOrder,
} from "./api/types";

const PAGE_LIMIT = 30;
const CURRENT_VERSION = "2.2.1";
const APP_TITLE = `需求暂存站 v${CURRENT_VERSION}  @linjianglu`;
const IS_ANDROID = /Android/i.test(navigator.userAgent);
const DEFAULT_EDIT_TEXTAREA_HEIGHT = 360;
const MIN_EDIT_TEXTAREA_HEIGHT = 180;
const MAX_EDIT_TEXTAREA_HEIGHT = 700;
const GITHUB_RELEASE_API_URL = "https://api.github.com/repos/LiKPO4/clipstash/releases/latest";
const GITHUB_RELEASES_URL = "https://github.com/LiKPO4/clipstash/releases/latest";
const IS_TEST_ENV = import.meta.env.MODE === "test";
const ANDROID_BACK_EVENT = "clipstash-android-back";
const ANDROID_SHARE_EVENT = "clipstash-android-share-ready";
const ANDROID_WIDGET_ACTION_EVENT = "clipstash-android-widget-action-ready";
const ANDROID_UPDATE_EVENT = "clipstash-android-update";
const ANDROID_UPDATE_POLL_MS = 250;
const ANDROID_UPDATE_TIMEOUT_MS = 20_000;

declare global {
  interface Window {
    ClipStashAndroid?: {
      consumePendingShare?: () => string;
      consumePendingWidgetAction?: () => string;
      consumePendingUpdate?: () => string;
      checkForUpdates?: () => boolean;
      copyText?: (text: string) => boolean | string;
      downloadAndInstallApk?: (downloadUrl: string, filename: string) => boolean;
      refreshWidgets?: () => void;
      shareZip?: (path: string) => void;
    };
  }
}

type PreviewImage = {
  externalWindow?: boolean;
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
  file?: File;
  filename: string;
  path: string;
  src: string;
};

type ComposerImageItem =
  | {
      file: File;
      id: string;
      kind: "file";
    }
  | {
      id: string;
      image: LegacyMessageImage;
      kind: "existing";
    };

type EditResult = LegacyCreateTextMessageResult | LegacyReplaceImagesResult;

type CopyResult = {
  messageId: number;
  textLength: number;
};

type ImportQueuePasteAllResult = LegacyImportQueuePasteResult;

type ImportQueuePasteArchiveResult = LegacyImportQueuePasteArchiveResult;

type AndroidSharedPayload = {
  shareId?: string;
  error?: string;
  text?: string;
  images?: AndroidSharedImage[];
};

type AndroidSharedImage = {
  mimeType?: string;
  data: string;
};

type AndroidUpdateEventDetail = {
  message?: string;
  release?: GithubReleaseInfo;
  status?: "checked" | "downloading" | "installing" | "permission" | "error";
};

type ReleaseCheckResult = {
  currentVersion: string;
  downloadAsset: ReleaseDownloadAsset | null;
  latestVersion: string;
  releaseUrl: string;
  hasUpdate: boolean;
};

type ReleaseDownloadAsset = {
  downloadUrl: string;
  filename: string;
};

type PerformanceMeasurement = {
  measure: string;
  start: number;
};

// 预览窗口创建代次：close 时递增；show 的异步流程每次 await 后校验，代次过期则放弃创建，
// 避免鼠标已移开（或已按 Escape）后异步流程仍把窗口创建出来造成残留。
const MESSAGE_DOUBLE_CLICK_DELAY_MS = 220;

function App() {
  return <ThumbnailProvider><AppContent /></ThumbnailProvider>;
}

function AppContent() {
  const [stats, setStats] = useState<LegacyStats | null>(null);
  const [page, setPage] = useState<LegacyMessagePage | null>(null);
  const [view, setView] = useState<MessageView>("normal");
  const [sort, setSort] = useState<SortOrder>(() => getStoredSort());
  const [searchOpen, setSearchOpen] = useState(false);
  const [searchDraft, setSearchDraft] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [loadingMore, setLoadingMore] = useState(false);
  const [previewImage, setPreviewImage] = useState<PreviewImage | null>(null);
  const [hoverDelay, setHoverDelay] = useState(0.8);
  const [scrollLines, setScrollLines] = useState(1);
  const [fontScale, setFontScale] = useState(0);
  const [editTextareaHeight, setEditTextareaHeight] = useState(DEFAULT_EDIT_TEXTAREA_HEIGHT);
  const [pasteIntervalMs, setPasteIntervalMs] = useState(250);
  const [showHotkey, setShowHotkey] = useState("Ctrl+Shift+V");
  const [captureHotkey, setCaptureHotkey] = useState("Ctrl+Alt+V");
  const [expandedImageMessageIds, setExpandedImageMessageIds] = useState<number[]>([]);
  const [mediaTextDraft, setMediaTextDraft] = useState("");
  const [mediaFiles, setMediaFiles] = useState<File[]>([]);
  const [mediaPreviewImages, setMediaPreviewImages] = useState<PreviewImageItem[]>([]);
  const [mediaInputKey, setMediaInputKey] = useState(0);
  const [creatingMediaMessage, setCreatingMediaMessage] = useState(false);
  const [createMediaError, setCreateMediaError] = useState<string | null>(null);
  const [editingMessage, setEditingMessage] = useState<LegacyMessage | null>(null);
  const [editTextDraft, setEditTextDraft] = useState("");
  const [editImageItems, setEditImageItems] = useState<ComposerImageItem[]>([]);
  const [editPreviewImages, setEditPreviewImages] = useState<PreviewImageItem[]>([]);
  const [editInputKey, setEditInputKey] = useState(0);
  const [savingEdit, setSavingEdit] = useState(false);
  const [splittingEdit, setSplittingEdit] = useState(false);
  const [editError, setEditError] = useState<string | null>(null);
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
  const [androidShareError, setAndroidShareError] = useState<string | null>(null);
  const [androidShareResult, setAndroidShareResult] = useState<LegacyMessage | null>(null);
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
  const [archiveAfterExport, setArchiveAfterExport] = useState(false);
  const [matchBlankLinesToImages, setMatchBlankLinesToImages] = useState(false);
  const [appSettingsReady, setAppSettingsReady] = useState(false);
  const [importQueuePasteArchiveResult, setImportQueuePasteArchiveResult] =
    useState<ImportQueuePasteArchiveResult | null>(null);
  const [showComposer, setShowComposer] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [messageDoubleClickAction, setMessageDoubleClickAction] =
    useState<MessageDoubleClickAction>("edit");
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
  const [downloadingUpdate, setDownloadingUpdate] = useState(false);
  const [globalShortcutErrors, setGlobalShortcutErrors] = useState<string[]>([]);
  const [migratingLegacyData, setMigratingLegacyData] = useState(false);
  const [movingAppData, setMovingAppData] = useState(false);
  const [repairingAppData, setRepairingAppData] = useState(false);
  const [migrationResult, setMigrationResult] = useState<AppMigrationResult | null>(null);
  const [migrationError, setMigrationError] = useState<string | null>(null);
  const [exportingData, setExportingData] = useState(false);
  const [importingDataPackage, setImportingDataPackage] = useState(false);
  const [dataExportResult, setDataExportResult] = useState<DataExportResult | null>(null);
  const [dataImportResult, setDataImportResult] = useState<DataImportResult | null>(null);
  const [dataImportPreview, setDataImportPreview] = useState<DataImportPreview | null>(null);
  const [dataTransferError, setDataTransferError] = useState<string | null>(null);
  const messageListRef = useRef<HTMLElement | null>(null);
  const loadMoreInFlightRef = useRef(false);
  const androidShareTaskRef = useRef<Promise<void>>(Promise.resolve());
  // 上次实际调用 setAlwaysOnTop IPC 时的值，用于跳过无变化的重复调用（null 表示尚未调用过）。
  const lastAlwaysOnTopValueRef = useRef<boolean | null>(null);
  const dataPackageInputRef = useRef<HTMLInputElement | null>(null);
  const pendingMessageListScrollTopRef = useRef<number | null>(null);
  const requestEpochRef = useRef(0);
  const pageCommitMeasurementRef = useRef<PerformanceMeasurement | null>(null);
  const [dataPackageInputKey, setDataPackageInputKey] = useState(0);

  function commitPage(nextPage: LegacyMessagePage) {
    finishPerformanceMeasurement(pageCommitMeasurementRef.current);
    pageCommitMeasurementRef.current = startPerformanceMeasurement("page-dom-commit");
    setPage(nextPage);
  }

  useEffect(() => {
    document.title = APP_TITLE;
  }, []);

  useEffect(() => {
    return () => {
      finishPerformanceMeasurement(pageCommitMeasurementRef.current);
      pageCommitMeasurementRef.current = null;
    };
  }, []);

  const saveAndroidShare = useCommittedCallback(createMessageFromAndroidShare);
  useEffect(() => {
    if (!IS_ANDROID) return;
    function consumeAndroidShare() {
      let rawPayload = window.ClipStashAndroid?.consumePendingShare?.();
      while (rawPayload) {
        const raw = rawPayload;
        // Only small identifiers enter this queue. Saving and refreshing are serialized.
        androidShareTaskRef.current = androidShareTaskRef.current.then(async () => {
          const payload = JSON.parse(raw) as AndroidSharedPayload;
          if (payload.error) throw new Error(payload.error);
          await saveAndroidShare(payload);
        }).catch((err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          setAndroidShareError((previous) => [previous, message].filter(Boolean).join("\n").split("\n").slice(-5).join("\n"));
          setAndroidShareResult(null);
        });
        rawPayload = window.ClipStashAndroid?.consumePendingShare?.();
      }
    }
    consumeAndroidShare();
    window.addEventListener(ANDROID_SHARE_EVENT, consumeAndroidShare);
    return () => window.removeEventListener(ANDROID_SHARE_EVENT, consumeAndroidShare);
  }, [saveAndroidShare]);

  useEffect(() => {
    if (!IS_ANDROID) return;

    function handleAndroidUpdate(event: Event) {
      const detail = (event as CustomEvent<AndroidUpdateEventDetail>).detail;
      window.ClipStashAndroid?.consumePendingUpdate?.();
      const message = detail?.message?.trim();
      if (detail?.status === "checked" && detail.release) {
        try {
          applyReleaseCheckResult(detail.release);
          setReleaseCheckError(null);
        } catch (err) {
          setReleaseCheckError(err instanceof Error ? err.message : String(err));
        }
        setCheckingUpdate(false);
        return;
      }
      if (detail?.status === "error") {
        setReleaseCheckError(message || "Android 更新失败");
        setSettingsNotice(null);
        setCheckingUpdate(false);
        return;
      }
      setReleaseCheckError(null);
      if (message) setSettingsNotice(message);
    }

    window.addEventListener(ANDROID_UPDATE_EVENT, handleAndroidUpdate);
    return () => window.removeEventListener(ANDROID_UPDATE_EVENT, handleAndroidUpdate);
  }, []);

  useEffect(() => {
    if (!IS_ANDROID || !checkingUpdate) return;

    function consumePendingUpdate() {
      const rawPayload = window.ClipStashAndroid?.consumePendingUpdate?.();
      if (!rawPayload) return;

      try {
        const detail = JSON.parse(rawPayload) as AndroidUpdateEventDetail;
        window.dispatchEvent(new CustomEvent(ANDROID_UPDATE_EVENT, { detail }));
      } catch (err) {
        setReleaseCheckError(err instanceof Error ? err.message : String(err));
        setCheckingUpdate(false);
      }
    }

    consumePendingUpdate();
    const pollTimer = window.setInterval(consumePendingUpdate, ANDROID_UPDATE_POLL_MS);
    const timeoutTimer = window.setTimeout(() => {
      setReleaseCheckError("检查更新超时，请检查网络后重试");
      setCheckingUpdate(false);
    }, ANDROID_UPDATE_TIMEOUT_MS);

    return () => {
      window.clearInterval(pollTimer);
      window.clearTimeout(timeoutTimer);
    };
  }, [checkingUpdate]);

  useEffect(() => {
    if (!IS_ANDROID) return;

    async function consumeWidgetAction() {
      if (!appSettingsReady) return;
      const action = window.ClipStashAndroid?.consumePendingWidgetAction?.();
      if (!action) return;

      setShowSettings(false);
      setEditingMessage(null);
      setDeletingMessage(null);
      setPreviewImage(null);
      if (action === "export") {
        exportDataPackage().catch(() => undefined);
        return;
      }
      if (action.startsWith("edit:")) {
        const messageId = Number(action.slice("edit:".length));
        if (!Number.isSafeInteger(messageId) || messageId <= 0) return;
        try {
          const message = await getLegacyMessage(messageId);
          openEditMessage(message);
        } catch (err) {
          setError(err instanceof Error ? err.message : String(err));
        }
        return;
      }
      if (action !== "create") return;
      setCreateMediaError(null);
      setShowComposer(true);
    }

    consumeWidgetAction();
    window.addEventListener(ANDROID_WIDGET_ACTION_EVENT, consumeWidgetAction);
    return () => window.removeEventListener(ANDROID_WIDGET_ACTION_EVENT, consumeWidgetAction);
  }, [appSettingsReady, archiveAfterExport, exportingData]);

  useEffect(() => {
    let alive = true;
    const requestEpoch = ++requestEpochRef.current;

    setError(null);
    setLoadingMore(false);

    loadAppData(view, sort, searchQuery)
      .then(([nextStats, nextPage]) => {
        if (!alive || requestEpochRef.current !== requestEpoch) return;
        setStats(nextStats);
        commitPage(nextPage);
        setError(null);
      })
      .catch((err: unknown) => {
        if (!alive || requestEpochRef.current !== requestEpoch) return;
        setError(err instanceof Error ? err.message : String(err));
        setPage(null);
      })
    return () => {
      alive = false;
    };
  }, [view, sort, searchQuery]);

  useLayoutEffect(() => {
    if (pageCommitMeasurementRef.current) {
      finishPerformanceMeasurement(pageCommitMeasurementRef.current);
      pageCommitMeasurementRef.current = null;
    }

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
        closeHoverPreviewWindow();
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
    const previews = mediaFiles.map((file, index) => ({
      filename: file.name, path: `composer:${index}:${file.name}:${file.size}`,
      src: URL.createObjectURL(file), file,
    }));
    setMediaPreviewImages(previews);
    return () => previews.forEach((image) => URL.revokeObjectURL(image.src));
  }, [mediaFiles]);

  useEffect(() => {
    const previews = editImageItems.map(composerImageItemToPreview);
    setEditPreviewImages(previews);
    return () => previews.forEach((image) => { if (image.file) URL.revokeObjectURL(image.src); });
  }, [editImageItems]);

  useEffect(() => {
    if (!copyError && !copyResult) return;

    const timer = window.setTimeout(() => {
      clearCopyFeedback();
    }, 2400);

    return () => window.clearTimeout(timer);
  }, [copyError, copyResult]);

  useEffect(() => {
    if (!androidShareError && !androidShareResult) return;

    const timer = window.setTimeout(() => {
      setAndroidShareError(null);
      setAndroidShareResult(null);
    }, 2400);

    return () => window.clearTimeout(timer);
  }, [androidShareError, androidShareResult]);

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
    if (!deleteResult && !archiveError && !archiveResult) return;

    const timer = window.setTimeout(() => {
      clearWriteFeedback();
    }, 2400);

    return () => window.clearTimeout(timer);
  }, [archiveError, archiveResult, deleteResult]);

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
        setAppSettingsReady(true);
      })
      .catch((err: unknown) => {
        if (alive) {
          setSettingsError(err instanceof Error ? err.message : String(err));
          setAppSettingsReady(true);
        }
      });

    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    let alive = true;
    if (IS_ANDROID) {
      return () => {
        alive = false;
      };
    }

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
    if (IS_ANDROID) return;

    let alive = true;
    let cleanup: (() => void) | null = null;
    const currentWindow = getCurrentWindow();
    if (typeof currentWindow.onDragDropEvent !== "function") {
      return () => {
        alive = false;
      };
    }

    currentWindow
      .onDragDropEvent((event) => {
        if (!alive || event.payload.type !== "drop") return;
        handleNativeDroppedPaths(event.payload.paths).catch((err: unknown) => {
          const message = err instanceof Error ? err.message : String(err);
          if (showComposer) {
            setCreateMediaError(message);
          } else if (editingMessage) {
            setEditError(message);
          }
        });
      })
      .then((unlisten) => {
        cleanup = unlisten;
        if (!alive) unlisten();
      })
      .catch(() => undefined);

    return () => {
      alive = false;
      cleanup?.();
    };
  }, [showComposer, editingMessage?.id]);

  useEffect(() => {
    let alive = true;
    if (IS_ANDROID) {
      setStartup(false);
      return () => {
        alive = false;
      };
    }
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
    if (IS_ANDROID) {
      setGlobalShortcutErrors([]);
      return;
    }

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
    if (IS_ANDROID) return;

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

  useEffect(() => {
    if (!IS_ANDROID) return;

    const handleAndroidBack = (event: Event) => {
      if (previewImage) {
        event.preventDefault();
        setPreviewImage(null);
        return;
      }

      if (deletingMessage) {
        event.preventDefault();
        closeDeleteMessage();
        return;
      }

      if (editingMessage) {
        event.preventDefault();
        closeEditMessage();
        return;
      }

      if (showComposer) {
        event.preventDefault();
        if (!creatingMediaMessage) {
          setShowComposer(false);
          setCreateMediaError(null);
        }
        return;
      }

      if (showSettings) {
        event.preventDefault();
        setShowSettings(false);
      }
    };

    window.addEventListener(ANDROID_BACK_EVENT, handleAndroidBack);
    return () => window.removeEventListener(ANDROID_BACK_EVENT, handleAndroidBack);
  }, [
    creatingMediaMessage,
    deletingMessage,
    editingMessage,
    previewImage,
    showComposer,
    showSettings,
  ]);

  function clearCopyFeedback() {
    setCopyError(null);
    setCopyResult(null);
  }

  function clearImportFeedback() {
    setImportQueueError(null);
    setImportQueuePreview(null);
    setImportQueuePasteAllError(null);
    setImportQueuePasteAllResult(null);
    setImportQueuePasteArchiveResult(null);
  }

  function clearWriteFeedback() {
    setDeleteResult(null);
    setArchiveError(null);
    setArchiveResult(null);
  }

  function applyAppSettings(settings: AppSettings) {
    setAlwaysOnTop(settings.always_on_top);
    setCloseToTray(settings.close_to_tray);
    setStartup(settings.launch_on_startup);
    setArchiveAfterImport(settings.archive_after_import);
    setArchiveAfterExport(settings.archive_after_export);
    setMatchBlankLinesToImages(settings.match_blank_lines_to_images);
    setPasteIntervalMs(settings.paste_interval_ms);
    setHoverDelay(settings.hover_delay);
    setScrollLines(settings.scroll_lines);
    setFontScale(settings.font_scale);
    setEditTextareaHeight(settings.edit_textarea_height);
    setShowHotkey(settings.show_hotkey);
    setCaptureHotkey(settings.capture_hotkey);
    setSort(settings.sort);
    setMessageDoubleClickAction(settings.message_double_click_action ?? "edit");
    if (!IS_ANDROID && settings.always_on_top !== lastAlwaysOnTopValueRef.current) {
      lastAlwaysOnTopValueRef.current = settings.always_on_top;
      getCurrentWindow()
        .setAlwaysOnTop(settings.always_on_top)
        .catch((err: unknown) => setTopmostError(err instanceof Error ? err.message : String(err)));
    }
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

  function persistEditTextareaHeight(height: number) {
    const nextHeight = clampEditTextareaHeight(height);
    if (nextHeight === editTextareaHeight) return;

    setEditTextareaHeight(nextHeight);
    updateAppSettings({ edit_textarea_height: nextHeight })
      .then(applyAppSettings)
      .catch((err: unknown) => {
        setSettingsError(err instanceof Error ? err.message : String(err));
      });
  }

  async function loadMore() {
    if (!page || !page.has_more || loadingMore || loadMoreInFlightRef.current) return;
    loadMoreInFlightRef.current = true;
    const requestEpoch = ++requestEpochRef.current;
    const measurement = startPerformanceMeasurement("page-list-ready");

    setLoadingMore(true);
    setError(null);

    try {
      const nextPage = await listLegacyMessages({
        view,
        sort,
        offset: page.offset + page.messages.length,
        limit: PAGE_LIMIT,
        search: searchQuery,
      });
      if (requestEpochRef.current !== requestEpoch) return;
      commitPage({
        ...nextPage,
        offset: 0,
        messages: [...page.messages, ...nextPage.messages],
      });
    } catch (err) {
      if (requestEpochRef.current !== requestEpoch) return;
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      finishPerformanceMeasurement(measurement);
      loadMoreInFlightRef.current = false;
      setLoadingMore(false);
    }
  }

  async function refreshAppData({ preserveListScroll = true } = {}) {
    const requestEpoch = ++requestEpochRef.current;
    if (preserveListScroll) {
      pendingMessageListScrollTopRef.current = messageListRef.current?.scrollTop ?? null;
    }
    const [nextStats, nextPage] = await loadAppData(
      view, sort, searchQuery, preserveListScroll ? Math.max(PAGE_LIMIT, page?.messages.length ?? 0) : PAGE_LIMIT,
    );
    if (requestEpochRef.current !== requestEpoch) return;
    setStats(nextStats);
    commitPage(nextPage);
  }

  function refreshAndroidWidgets() {
    if (IS_ANDROID) {
      window.ClipStashAndroid?.refreshWidgets?.();
    }
  }

  async function openLocalPath(path: string) {
    setOpenPathError(null);
    try {
      await openAppPath(path);
    } catch (err) {
      setOpenPathError(err instanceof Error ? err.message : String(err));
    }
  }

  function submitSearch(event?: FormEvent<HTMLFormElement>) {
    event?.preventDefault();
    setSearchQuery(searchDraft.trim());
  }

  function clearSearch() {
    setSearchDraft("");
    setSearchQuery("");
    setSearchOpen(false);
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

    if (IS_ANDROID) {
      const bridge = window.ClipStashAndroid;
      if (!bridge?.checkForUpdates) {
        setReleaseCheckError("当前 Android 安装包不支持应用内检查更新");
        setCheckingUpdate(false);
        return;
      }
      // 必须在注入对象上直接调用，解绑后调用会触发 WebView "non-injected object" 错误
      bridge.consumePendingUpdate?.();
      let started = false;
      try {
        started = bridge.checkForUpdates();
        if (!started) {
          setReleaseCheckError("无法启动 Android 更新检查");
        }
      } catch (err: unknown) {
        setReleaseCheckError(err instanceof Error ? err.message : String(err));
      } finally {
        if (!started) {
          setCheckingUpdate(false);
        }
      }
      return;
    }

    try {
      const payload = await fetchLatestReleaseForCheck();
      applyReleaseCheckResult(payload);
    } catch (err: unknown) {
      setReleaseCheckError(err instanceof Error ? err.message : String(err));
    } finally {
      setCheckingUpdate(false);
    }
  }

  function applyReleaseCheckResult(payload: GithubReleaseInfo) {
    const latestVersion = normalizeReleaseVersion(payload.tag_name ?? "");
    if (!latestVersion) throw new Error("GitHub Release 响应缺少版本号");

    setReleaseCheckResult({
      currentVersion: CURRENT_VERSION,
      downloadAsset: IS_ANDROID
        ? selectAndroidApkAsset(payload.assets ?? [])
        : selectWindowsInstallerAsset(payload.assets ?? []),
      latestVersion,
      releaseUrl: payload.html_url || GITHUB_RELEASES_URL,
      hasUpdate: compareVersions(latestVersion, CURRENT_VERSION) > 0,
    });
  }

  async function fetchLatestReleaseForCheck(): Promise<GithubReleaseInfo> {
    if (!IS_ANDROID && !IS_TEST_ENV && typeof fetch === "function") {
      const response = await fetch(GITHUB_RELEASE_API_URL, {
        cache: "no-store",
        headers: {
          Accept: "application/vnd.github+json",
        },
      });
      if (!response.ok) {
        throw new Error(`GitHub Release 检查失败：HTTP ${response.status}`);
      }
      return response.json() as Promise<GithubReleaseInfo>;
    }

    return fetchLatestGithubRelease();
  }

  async function downloadUpdate() {
    if (downloadingUpdate || !releaseCheckResult?.downloadAsset) return;

    setDownloadingUpdate(true);
    setReleaseCheckError(null);
    try {
      if (IS_ANDROID) {
        const bridge = window.ClipStashAndroid;
        if (!bridge?.downloadAndInstallApk) throw new Error("当前 Android 安装包不支持应用内更新");
        // 必须在注入对象上直接调用，解绑后调用会触发 WebView "non-injected object" 错误
        const started = bridge.downloadAndInstallApk(
          releaseCheckResult.downloadAsset.downloadUrl,
          releaseCheckResult.downloadAsset.filename,
        );
        if (!started) throw new Error("无法启动 Android 更新下载");
        setSettingsNotice("正在下载更新安装包");
        return;
      }
      await downloadAndOpenUpdateInstaller(
        releaseCheckResult.downloadAsset.downloadUrl,
        releaseCheckResult.downloadAsset.filename,
      );
      setSettingsNotice("安装包已打开，请按安装向导完成更新");
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err: unknown) {
      setReleaseCheckError(err instanceof Error ? err.message : String(err));
    } finally {
      setDownloadingUpdate(false);
    }
  }

  async function runLegacyMigration() {
    if (migratingLegacyData) return;
    const requestEpoch = ++requestEpochRef.current;

    setMigratingLegacyData(true);
    setMigrationError(null);
    setMigrationResult(null);

    try {
      const result = await migrateLegacyData();
      if (requestEpochRef.current !== requestEpoch) return;
      setMigrationResult(result);
      setStats(result.stats);
      const nextPage = await listLegacyMessages({
        view,
        sort,
        offset: 0,
        limit: PAGE_LIMIT,
      });
      if (requestEpochRef.current !== requestEpoch) return;
      commitPage(nextPage);
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

  async function refreshPageAfterDataChange(nextStats: LegacyStats) {
    const requestEpoch = ++requestEpochRef.current;
    const nextPage = await listLegacyMessages({
      view,
      sort,
      offset: 0,
      limit: PAGE_LIMIT,
    });
    if (requestEpochRef.current !== requestEpoch) return;
    setStats(nextStats);
    commitPage(nextPage);
  }

  async function exportDataPackage() {
    if (exportingData) return;

    setExportingData(true);
    setDataTransferError(null);
    setDataExportResult(null);
    if (IS_ANDROID) {
      setSettingsNotice("正在准备导出数据包");
    }

    try {
      const result = IS_ANDROID
        ? await exportAndroidDataPackage()
        : await exportNormalDataZip();
      setDataExportResult(result);
      setSettingsNotice(
        IS_ANDROID && archiveAfterExport && result.message_count > 0
          ? `已导出 ${result.message_count} 条普通消息并自动归档`
          : `已导出 ${result.message_count} 条普通消息`,
      );
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message !== "已取消导出数据") {
        setDataTransferError(message);
      }
    } finally {
      setExportingData(false);
    }
  }

  async function exportAndroidDataPackage() {
    const result = window.ClipStashAndroid?.shareZip
      ? await exportNormalDataZipFile()
      : await exportNormalDataZipBytes();
    if (window.ClipStashAndroid?.shareZip) {
      window.ClipStashAndroid.shareZip(result.export.path);
    } else if ("bytes" in result) {
      await openExportedDataPackage(result.filename, result.bytes as number[], result.export.path);
    }
    if (archiveAfterExport && result.message_ids.length > 0) {
      const nextStats = await archiveExportedMessages(result.message_ids);
      await refreshPageAfterDataChange(nextStats);
      refreshAndroidWidgets();
    }
    return result.export;
  }

  async function importDataPackage() {
    if (importingDataPackage) return;

    if (IS_ANDROID) {
      dataPackageInputRef.current?.click();
      return;
    }

    setImportingDataPackage(true);
    setDataTransferError(null);
    setDataImportResult(null);
    setDataImportPreview(null);

    try {
      const preview = await previewDataZip();
      setDataImportPreview(preview);
      setSettingsNotice(null);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message !== "已取消导入数据") {
        setDataTransferError(message);
      }
    } finally {
      setImportingDataPackage(false);
    }
  }

  async function confirmDataPackageImport() {
    if (importingDataPackage || !dataImportPreview) return;

    setImportingDataPackage(true);
    setDataTransferError(null);
    setDataImportResult(null);

    try {
      const result = await importDataZipFromPath(dataImportPreview.path);
      setDataImportResult(result);
      setDataImportPreview(null);
      await refreshPageAfterDataChange(result.stats);
      refreshAndroidWidgets();
      setSettingsNotice(
        result.inserted_messages > 0
          ? `已导入 ${result.inserted_messages} 条，跳过 ${result.skipped_messages} 条重复`
          : `没有新增数据，已跳过 ${result.skipped_messages} 条重复`,
      );
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message !== "已取消导入数据") {
        setDataTransferError(message);
      }
    } finally {
      setImportingDataPackage(false);
    }
  }

  async function importDataPackageFile(file: File) {
    if (importingDataPackage) return;

    setImportingDataPackage(true);
    setDataTransferError(null);
    setDataImportResult(null);
    setDataImportPreview(null);

    try {
      if (!isZipPath(file.name)) {
        throw new Error("导入数据包必须是 .zip 文件");
      }
      const result = await importZipFile(file);
      setDataImportResult(result);
      await refreshPageAfterDataChange(result.stats);
      refreshAndroidWidgets();
      setSettingsNotice(
        result.inserted_messages > 0
          ? `已导入 ${result.inserted_messages} 条，跳过 ${result.skipped_messages} 条重复`
          : `没有新增数据，已跳过 ${result.skipped_messages} 条重复`,
      );
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      setDataTransferError(err instanceof Error ? err.message : String(err));
    } finally {
      setImportingDataPackage(false);
      setDataPackageInputKey((key) => key + 1);
    }
  }

  function selectDataPackageFile(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    if (!file) return;
    importDataPackageFile(file).catch((err: unknown) => {
      setDataTransferError(err instanceof Error ? err.message : String(err));
    });
  }

  async function importDataPackageFromPath(path: string) {
    if (importingDataPackage) return;

    setImportingDataPackage(true);
    setDataTransferError(null);
    setDataImportResult(null);
    setDataImportPreview(null);

    try {
      const result = await importDataZipFromPath(path);
      setDataImportResult(result);
      await refreshPageAfterDataChange(result.stats);
      refreshAndroidWidgets();
      setSettingsNotice(
        result.inserted_messages > 0
          ? `已导入 ${result.inserted_messages} 条，跳过 ${result.skipped_messages} 条重复`
          : `没有新增数据，已跳过 ${result.skipped_messages} 条重复`,
      );
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      setDataTransferError(err instanceof Error ? err.message : String(err));
    } finally {
      setImportingDataPackage(false);
    }
  }

  async function moveAppDataDir() {
    if (movingAppData) return;
    const requestEpoch = ++requestEpochRef.current;

    setMovingAppData(true);
    setOpenPathError(null);
    setSettingsNotice(null);

    try {
      const result = await moveAppDataToSelectedDir();
      if (requestEpochRef.current !== requestEpoch) return;
      setStats(result.stats);
      const nextPage = await listLegacyMessages({
        view,
        sort,
        offset: 0,
        limit: PAGE_LIMIT,
      });
      if (requestEpochRef.current !== requestEpoch) return;
      commitPage(nextPage);
      setSettingsNotice("数据目录已迁移，原目录已保留");
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      if (message !== "已取消选择数据目录") {
        setOpenPathError(message);
      }
    } finally {
      setMovingAppData(false);
    }
  }

  async function repairAppData() {
    if (repairingAppData) return;
    const requestEpoch = ++requestEpochRef.current;

    setRepairingAppData(true);
    setOpenPathError(null);
    setSettingsNotice(null);

    try {
      const result = await repairAppDataDir();
      if (requestEpochRef.current !== requestEpoch) return;
      setStats(result.stats);
      const nextPage = await listLegacyMessages({
        view,
        sort,
        offset: 0,
        limit: PAGE_LIMIT,
      });
      if (requestEpochRef.current !== requestEpoch) return;
      commitPage(nextPage);
      setSettingsNotice(
        result.copied_images > 0 || result.copied_db
          ? `已修复数据目录，补回 ${result.copied_images} 张图片`
          : "数据目录无需修复",
      );
      window.setTimeout(() => setSettingsNotice(null), 2400);
    } catch (err) {
      setOpenPathError(err instanceof Error ? err.message : String(err));
    } finally {
      setRepairingAppData(false);
    }
  }

  function selectMediaFiles(event: ChangeEvent<HTMLInputElement>) {
    const files = Array.from(event.target.files ?? []);
    setMediaFiles((currentFiles) => [...currentFiles, ...files]);
    setCreateMediaError(null);
  }

  function pasteMediaContent(event: ClipboardEvent<HTMLTextAreaElement>) {
    const pastedFiles = Array.from(event.clipboardData.files).filter((file) =>
      file.type.startsWith("image/"),
    );
    if (pastedFiles.length === 0) return;

    event.preventDefault();
    setMediaFiles((currentFiles) => [...currentFiles, ...pastedFiles]);
    setCreateMediaError(null);
  }

  function dropMediaFiles(files: File[]) {
    setMediaFiles((currentFiles) => [...currentFiles, ...files]);
    setCreateMediaError(null);
  }

  function removeMediaFile(indexToRemove: number) {
    setMediaFiles((currentFiles) =>
      currentFiles.filter((_, index) => index !== indexToRemove),
    );
    setMediaInputKey((key) => key + 1);
    setCreateMediaError(null);
  }

  async function createMediaMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const text = mediaTextDraft.trim();
    if ((!text && mediaFiles.length === 0) || creatingMediaMessage) return;

    setCreatingMediaMessage(true);
    setCreateMediaError(null);

    try {
      const imagesData = await filesToNumberArrays(mediaFiles);
      if (text && imagesData.length > 0) {
        await createLegacyMixedMessage(text, imagesData);
      } else if (text) {
        await createLegacyTextMessage(text);
      } else {
        await createLegacyImageMessage(imagesData);
      }
      await refreshAppData();
      refreshAndroidWidgets();
      setMediaTextDraft("");
      setMediaFiles([]);
      setMediaInputKey((key) => key + 1);
      setShowComposer(false);
    } catch (err) {
      setCreateMediaError(err instanceof Error ? err.message : String(err));
    } finally {
      setCreatingMediaMessage(false);
    }
  }

  async function createMessageFromAndroidShare(payload: AndroidSharedPayload) {
    const text = payload.text?.trim() ?? "";
    const imagesData = (payload.images ?? [])
      .map((image) => base64ToNumberArray(image.data))
      .filter((bytes) => bytes.length > 0);
    if (!payload.shareId && !text && imagesData.length === 0) {
      throw new Error("分享内容为空");
    }

    const requestEpoch = ++requestEpochRef.current;
    setAndroidShareResult(null);
    setShowComposer(false);
    setEditingMessage(null);
    setSearchDraft("");
    setSearchQuery("");

    const result = payload.shareId
      ? { message: await importAndroidShare(payload.shareId) }
      : text && imagesData.length > 0
        ? await createLegacyMixedMessage(text, imagesData)
        : text
          ? await createLegacyTextMessage(text)
          : await createLegacyImageMessage(imagesData);

    const [nextStats, nextPage] = await loadAppData("normal", sort, "");
    if (requestEpochRef.current === requestEpoch) {
      setView("normal");
      setStats(nextStats);
      commitPage(nextPage);
    }
    setAndroidShareResult(result.message);
    refreshAndroidWidgets();
  }

  function openEditMessage(message: LegacyMessage) {
    setEditingMessage(message);
    setEditTextDraft(message.text_content ?? "");
    setEditImageItems(message.images.map(existingImageToComposerItem));
    setEditInputKey((key) => key + 1);
    setEditError(null);
  }

  function handleMessageDoubleClick(message: LegacyMessage) {
    if (messageDoubleClickAction === "create") {
      setShowComposer(true);
      return;
    }
    if (messageDoubleClickAction === "edit") {
      openEditMessage(message);
    }
  }

  function closeEditMessage() {
    if (savingEdit || splittingEdit) return;
    setEditingMessage(null);
    setEditError(null);
  }

  function selectEditFiles(event: ChangeEvent<HTMLInputElement>) {
    const selectedFiles = Array.from(event.target.files ?? []);
    setEditImageItems((currentItems) => [
      ...currentItems,
      ...selectedFiles.map((file, index) =>
        fileToComposerItem(file, `edit:${currentItems.length + index}`),
      ),
    ]);
    setEditError(null);
  }

  function pasteEditMediaContent(event: ClipboardEvent<HTMLTextAreaElement>) {
    const pastedFiles = Array.from(event.clipboardData.files).filter((file) =>
      file.type.startsWith("image/"),
    );
    if (pastedFiles.length === 0) return;

    event.preventDefault();
    setEditImageItems((currentItems) => [
      ...currentItems,
      ...pastedFiles.map((file, index) =>
        fileToComposerItem(file, `edit-paste:${currentItems.length + index}`),
      ),
    ]);
    setEditError(null);
  }

  function dropEditFiles(files: File[]) {
    setEditImageItems((currentItems) => [
      ...currentItems,
      ...files.map((file, index) =>
        fileToComposerItem(file, `edit-drop:${currentItems.length + index}`),
      ),
    ]);
    setEditError(null);
  }

  async function handleNativeDroppedPaths(paths: string[]) {
    if (paths.length === 0) return;

    if (!showComposer && !editingMessage) {
      if (paths.length === 1 && isZipPath(paths[0])) {
        await importDataPackageFromPath(paths[0]);
      }
      return;
    }

    const imagePaths = paths.filter(isImagePath);
    const textPaths = paths.filter((path) => !isImagePath(path));

    if (imagePaths.length > 0) {
      const files = await Promise.all(imagePaths.map(droppedPathToFile));
      if (showComposer) {
        dropMediaFiles(files);
      } else if (editingMessage) {
        dropEditFiles(files);
      }
    }

    if (textPaths.length > 0) {
      const pathText = textPaths.join("\n");
      if (showComposer) {
        setMediaTextDraft((currentText) => appendTextBlock(currentText, pathText));
      } else if (editingMessage) {
        setEditTextDraft((currentText) => appendTextBlock(currentText, pathText));
      }
    }
  }

  function removeEditFile(indexToRemove: number) {
    setEditImageItems((currentItems) =>
      currentItems.filter((_, index) => index !== indexToRemove),
    );
    setEditInputKey((key) => key + 1);
    setEditError(null);
  }

  async function saveEditedMessage(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!editingMessage || !canSaveEdit) return;

    setSavingEdit(true);
    setEditError(null);

    try {
      let result: EditResult | null = null;
      const text = editTextDraft.trim();
      const normalizedText = text.length > 0 ? text : null;
      if ((editingMessage.text_content ?? null) !== normalizedText) {
        result = await updateLegacyMessageText(editingMessage.id, normalizedText);
      }
      if (editImagesChanged(editingMessage, editImageItems)) {
        const imagesData = await composerImageItemsToNumberArrays(editImageItems);
        result = await replaceLegacyMessageImages(editingMessage.id, imagesData);
      }
      if (!result) {
        throw new Error("没有需要保存的变更");
      }
      await refreshAppData();
      refreshAndroidWidgets();
      setEditImageItems([]);
      setEditInputKey((key) => key + 1);
      setEditingMessage(null);
    } catch (err) {
      setEditError(err instanceof Error ? err.message : String(err));
    } finally {
      setSavingEdit(false);
    }
  }

  async function splitEditedMessage() {
    if (!editingMessage || splittingEdit || savingEdit) return;

    const lines = splitMessageLines(editTextDraft);
    if (lines.length < 2) {
      setEditError("至少需要两行非空文字才能拆分");
      return;
    }

    setSplittingEdit(true);
    setEditError(null);
    try {
      const imagesData = await composerImageItemsToNumberArrays(editImageItems);
      await splitLegacyMessage(editingMessage.id, lines.join("\n"), imagesData);
      await refreshAppData();
      refreshAndroidWidgets();
      setEditImageItems([]);
      setEditInputKey((key) => key + 1);
      setEditingMessage(null);
    } catch (err) {
      setEditError(err instanceof Error ? err.message : String(err));
    } finally {
      setSplittingEdit(false);
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
      refreshAndroidWidgets();
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
      refreshAndroidWidgets();
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
      if (IS_ANDROID) {
        const bridge = window.ClipStashAndroid;
        if (!bridge?.copyText) {
          throw new Error("写入 Android 系统剪贴板失败");
        }
        // 必须在注入对象上直接调用，解绑后调用会触发 WebView "non-injected object" 错误
        const copyStatus = bridge.copyText(text);
        if (copyStatus !== true && copyStatus !== "ok") {
          const detail = typeof copyStatus === "string" && copyStatus.startsWith("error:")
            ? `：${copyStatus.slice("error:".length)}`
            : "";
          throw new Error(`写入 Android 系统剪贴板失败${detail}`);
        }
        setCopyResult({
          messageId: message.id,
          textLength: Array.from(text).length,
        });
        return;
      }

      const result = await copyLegacyMessageTextToClipboard(message.id);
      setCopyResult({
        messageId: result.message_id,
        textLength: result.text_length,
      });
    } catch (err) {
      setCopyError(err instanceof Error ? err.message : String(err));
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
      const preview = await previewLegacyMessageImportQueue(message.id, matchBlankLinesToImages);
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
        matchBlankLinesToImages,
      });
      setImportQueuePasteAllResult(result.paste);
      setImportQueuePasteArchiveResult(result);
      if (result.archive_result) {
        await refreshAppData();
        refreshAndroidWidgets();
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
  const canSplitEdit = !!editingMessage && splitMessageLines(editTextDraft).length >= 2;
  const editWillHaveImages = editImageItems.length > 0;
  const editHasContent = editText.length > 0 || editWillHaveImages;
  const canSaveEdit =
    !!editingMessage &&
    !savingEdit &&
    !splittingEdit &&
    editHasContent &&
    (((editingMessage.text_content ?? null) !==
      (editText.length > 0 ? editText : null)) ||
      editImagesChanged(editingMessage, editImageItems));
  return (
    <main
      className={IS_ANDROID ? "shell shell-android" : "shell"}
      style={{ fontSize: `${14 + fontScale}px` }}
    >
      <DataTransferProgress />
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
            className={searchOpen || searchQuery ? "icon-action active" : "icon-action"}
            onClick={() => {
              setSearchOpen((open) => !open);
            }}
            aria-label="搜索消息"
            title="搜索消息"
          >
            <svg
              className="search-icon"
              aria-hidden="true"
              viewBox="0 0 24 24"
              focusable="false"
            >
              <circle cx="10.5" cy="10.5" r="6.5" />
              <path d="M15.5 15.5 21 21" />
            </svg>
          </button>
          {IS_ANDROID ? (
            <button type="button" onClick={exportDataPackage} disabled={exportingData}>
              {exportingData ? "导出中" : "导出"}
            </button>
          ) : (
            <button
              type="button"
              className={alwaysOnTop ? "active" : ""}
              onClick={toggleAlwaysOnTop}
              title={topmostError ? `置顶失败：${topmostError}` : undefined}
            >
              {alwaysOnTop ? "已置顶" : "置顶"}
            </button>
          )}
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

      {searchOpen && (
        <form className="message-search" role="search" onSubmit={submitSearch}>
          <input
            type="search"
            value={searchDraft}
            onChange={(event) => setSearchDraft(event.target.value)}
            placeholder="搜索消息内容"
            autoFocus
            aria-label="搜索消息内容"
          />
          {searchQuery && (
            <button type="button" className="secondary-action" onClick={clearSearch}>
              清空
            </button>
          )}
          <button type="submit" className="write-submit">
            搜索
          </button>
        </form>
      )}

      {IS_ANDROID && (
        <input
          key={dataPackageInputKey}
          ref={dataPackageInputRef}
          className="composer-file-input"
          type="file"
          accept=".zip,application/zip"
          onChange={selectDataPackageFile}
          aria-hidden="true"
          tabIndex={-1}
        />
      )}

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
            <MessageComposerDialog
              canSave={canCreateMedia}
              autoFocus
              closeAriaLabel="关闭新消息"
              dialogLabel="编辑新消息"
              error={createMediaError}
              errorTitle="写入失败"
              eyebrow="新消息"
              fileInputId="new-media-message-files"
              imageItems={mediaFiles.map((file, index) =>
                fileToComposerItem(file, `composer:${index}`),
              )}
              inputKey={mediaInputKey}
              isAndroid={IS_ANDROID}
              onClose={() => setShowComposer(false)}
              onDropFiles={dropMediaFiles}
              onFileChange={selectMediaFiles}
              onPaste={pasteMediaContent}
              onPreview={setPreviewImage}
              onRemoveFile={removeMediaFile}
              onSubmit={createMediaMessage}
              onTextAreaHeightCommit={persistEditTextareaHeight}
              onTextChange={setMediaTextDraft}
              placeholder="输入文字，或直接粘贴图片"
              previewDelaySeconds={hoverDelay}
              previewImages={mediaPreviewImages}
              saving={creatingMediaMessage}
              textAreaHeight={editTextareaHeight}
              textAreaId="new-media-message-text"
              textDraft={mediaTextDraft}
              title="编辑消息"
            />
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
              isAndroid={IS_ANDROID}
              importingMessageId={loadingImportQueueMessageId}
              onDelete={openDeleteMessage}
              onEdit={openEditMessage}
              onMessageDoubleClick={handleMessageDoubleClick}
              onArchive={toggleArchiveMessage}
              onCopyText={copyMessageText}
              onToggleImages={toggleImageExpansion}
              onOpenImportQueue={openImportQueue}
              onPreview={setPreviewImage}
              showExternalImport={!IS_ANDROID}
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
              {searchQuery ? "没有匹配的消息。" : "当前视图没有消息。双击空白处创建。"}
            </button>
          )}
        </>
      )}

      {previewImage && (
        IS_ANDROID ? (
          <AndroidImagePreview image={previewImage} onClose={() => setPreviewImage(null)} />
        ) : (
          <>
            <div className="hover-preview-dim" />
            {!previewImage.externalWindow && <HoverImagePreview image={previewImage} />}
          </>
        )
      )}

      {editingMessage && (
        <EditMessageDialog
          error={editError}
          imageItems={editImageItems}
          inputKey={editInputKey}
          message={editingMessage}
          previewDelaySeconds={hoverDelay}
          previewImages={editPreviewImages}
          saving={savingEdit}
          splitting={splittingEdit}
          textAreaHeight={editTextareaHeight}
          textDraft={editTextDraft}
          canSplit={canSplitEdit}
          canSave={canSaveEdit}
          onClose={closeEditMessage}
          onDropFiles={dropEditFiles}
          onFileChange={selectEditFiles}
          isAndroid={IS_ANDROID}
          onPaste={pasteEditMediaContent}
          onPreview={setPreviewImage}
          onRemoveFile={removeEditFile}
          onSubmit={saveEditedMessage}
          onSplit={splitEditedMessage}
          onTextAreaHeightCommit={persistEditTextareaHeight}
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
          archiveAfterExport={archiveAfterExport}
          archiveAfterImport={archiveAfterImport}
          checkingUpdate={checkingUpdate}
          closeToTray={closeToTray}
          downloadingUpdate={downloadingUpdate}
          fontScale={fontScale}
          globalShortcutErrors={globalShortcutErrors}
          hoverDelay={hoverDelay}
          openPathError={openPathError}
          pasteIntervalMs={pasteIntervalMs}
          matchBlankLinesToImages={matchBlankLinesToImages}
          showHotkey={showHotkey}
          releaseCheckError={releaseCheckError}
          releaseCheckResult={releaseCheckResult}
          scrollLines={scrollLines}
          settingsError={settingsError}
          settingsNotice={settingsNotice}
          sort={sort}
          stats={stats}
          startup={startup}
          startupError={startupError}
          captureHotkey={captureHotkey}
          migrationError={migrationError}
          migrationResult={migrationResult}
          migratingLegacyData={migratingLegacyData}
          movingAppData={movingAppData}
          repairingAppData={repairingAppData}
          dataExportResult={dataExportResult}
          dataImportResult={dataImportResult}
          dataImportPreview={dataImportPreview}
          dataTransferError={dataTransferError}
          exportingData={exportingData}
          importingDataPackage={importingDataPackage}
          isAndroid={IS_ANDROID}
          onArchiveAfterExportChange={(checked) => {
            setArchiveAfterExport(checked);
            persistAppSettings({ archive_after_export: checked }).catch(() => undefined);
          }}
          onArchiveAfterImportChange={(checked) => {
            setArchiveAfterImport(checked);
            persistAppSettings({ archive_after_import: checked }).catch(() => undefined);
          }}
          onMatchBlankLinesToImagesChange={(checked) => {
            setMatchBlankLinesToImages(checked);
            persistAppSettings({ match_blank_lines_to_images: checked }).catch(() => undefined);
          }}
          onCloseToTrayChange={(checked) => {
            setCloseToTray(checked);
            persistAppSettings(
              { close_to_tray: checked },
              checked ? "关闭窗口时将隐藏到托盘" : "关闭窗口时将退出应用",
            ).catch(() => undefined);
          }}
          onClose={() => {
            setShowSettings(false);
            setDataImportPreview(null);
          }}
          onCheckUpdates={checkForUpdates}
          onDownloadUpdate={downloadUpdate}
          onFontScaleChange={(value) => {
            setFontScale(value);
            persistAppSettings({ font_scale: value }).catch(() => undefined);
          }}
          onHoverDelayChange={(value) => {
            setHoverDelay(value);
            persistAppSettings({ hover_delay: value }).catch(() => undefined);
          }}
          onShowHotkeyChange={(value) => {
            setShowHotkey(value);
            persistAppSettings({ show_hotkey: value }, "呼出界面快捷键已应用").catch(() => undefined);
          }}
          onCaptureHotkeyChange={(value) => {
            setCaptureHotkey(value);
            persistAppSettings({ capture_hotkey: value }, "导入剪切板快捷键已应用").catch(() => undefined);
          }}
          onOpenReleasePage={openExternalUrl}
          onOpenPath={openLocalPath}
          onMigrateLegacyData={runLegacyMigration}
          onExportData={exportDataPackage}
          onImportData={importDataPackage}
          onCancelDataImportPreview={() => setDataImportPreview(null)}
          onConfirmDataImportPreview={confirmDataPackageImport}
          onMessageDoubleClickActionChange={(value) => {
            setMessageDoubleClickAction(value);
            persistAppSettings(
              { message_double_click_action: value },
              "消息双击行为已应用",
            ).catch(() => undefined);
          }}
          onMoveAppDataDir={moveAppDataDir}
          onRepairAppData={repairAppData}
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
          messageDoubleClickAction={messageDoubleClickAction}
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

      {IS_ANDROID && !showSettings && (dataTransferError || dataExportResult) && (
        <OperationFeedback
          dismissLabel="关闭数据包提示"
          onDismiss={() => {
            setDataTransferError(null);
            setDataExportResult(null);
          }}
          surface="floating"
          variant={dataTransferError ? "error" : "success"}
          title={dataTransferError ? "数据包处理失败" : "数据包已导出"}
        >
          {dataTransferError ? (
            <p>{dataTransferError}</p>
          ) : (
            dataExportResult && (
              <p>
                {dataExportResult.message_count} 条普通消息，图片{" "}
                {dataExportResult.image_count} 张
                {archiveAfterExport && dataExportResult.message_count > 0
                  ? "，已自动归档。"
                  : "。"}
              </p>
            )
          )}
        </OperationFeedback>
      )}

      {IS_ANDROID && (androidShareError || androidShareResult) && (
        <OperationFeedback
          dismissLabel="关闭分享提示"
          onDismiss={() => {
            setAndroidShareError(null);
            setAndroidShareResult(null);
          }}
          surface="floating"
          variant={androidShareError ? "error" : "success"}
          title={androidShareError ? "分享保存失败" : "分享已保存"}
        >
          {androidShareError ? (
            <>
              <p>{androidShareError}</p>
              {androidShareResult && <p>随后已创建 #{androidShareResult.id}。</p>}
            </>
          ) : (
            androidShareResult && <p>已创建 #{androidShareResult.id}</p>
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
                    : matchBlankLinesToImages
                      ? "将按空行匹配图片，未匹配图片会依次追加到末尾。"
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

function formatDisplayPath(value: string) {
  const extendedUncPrefix = "\\\\?\\UNC\\";
  const extendedPathPrefix = "\\\\?\\";

  if (value.startsWith(extendedUncPrefix)) {
    return `\\\\${value.slice(extendedUncPrefix.length)}`;
  }
  if (value.startsWith(extendedPathPrefix)) {
    return value.slice(extendedPathPrefix.length);
  }
  return value;
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
  const displayValue = formatDisplayPath(value);

  return (
    <div className="path-row">
      <span className={ok ? "dot ok-dot" : "dot error-dot"} />
      <span className="path-label">{label}</span>
      <code title={displayValue}>{displayValue}</code>
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
  archiveAfterExport,
  archiveAfterImport,
  checkingUpdate,
  closeToTray,
  dataExportResult,
  dataImportResult,
  dataImportPreview,
  dataTransferError,
  downloadingUpdate,
  exportingData,
  fontScale,
  globalShortcutErrors,
  hoverDelay,
  importingDataPackage,
  isAndroid,
  migrationError,
  migrationResult,
  messageDoubleClickAction,
  matchBlankLinesToImages,
  migratingLegacyData,
  movingAppData,
  repairingAppData,
  openPathError,
  pasteIntervalMs,
  showHotkey,
  releaseCheckError,
  releaseCheckResult,
  scrollLines,
  settingsError,
  settingsNotice,
  sort,
  stats,
  startup,
  startupError,
  captureHotkey,
  onArchiveAfterExportChange,
  onArchiveAfterImportChange,
  onCheckUpdates,
  onDownloadUpdate,
  onExportData,
  onImportData,
  onCancelDataImportPreview,
  onConfirmDataImportPreview,
  onMessageDoubleClickActionChange,
  onMatchBlankLinesToImagesChange,
  onClose,
  onCloseToTrayChange,
  onFontScaleChange,
  onHoverDelayChange,
  onShowHotkeyChange,
  onCaptureHotkeyChange,
  onMigrateLegacyData,
  onMoveAppDataDir,
  onRepairAppData,
  onOpenReleasePage,
  onOpenPath,
  onPasteIntervalChange,
  onScrollLinesChange,
  onSettingsNotice,
  onSortChange,
  onStartupChange,
  onStartupPersistChange,
}: {
  archiveAfterExport: boolean;
  archiveAfterImport: boolean;
  checkingUpdate: boolean;
  closeToTray: boolean;
  dataExportResult: DataExportResult | null;
  dataImportResult: DataImportResult | null;
  dataImportPreview: DataImportPreview | null;
  dataTransferError: string | null;
  downloadingUpdate: boolean;
  exportingData: boolean;
  fontScale: number;
  globalShortcutErrors: string[];
  hoverDelay: number;
  importingDataPackage: boolean;
  isAndroid: boolean;
  migrationError: string | null;
  migrationResult: AppMigrationResult | null;
  messageDoubleClickAction: MessageDoubleClickAction;
  matchBlankLinesToImages: boolean;
  migratingLegacyData: boolean;
  movingAppData: boolean;
  repairingAppData: boolean;
  openPathError: string | null;
  pasteIntervalMs: number;
  showHotkey: string;
  releaseCheckError: string | null;
  releaseCheckResult: ReleaseCheckResult | null;
  scrollLines: number;
  settingsError: string | null;
  settingsNotice: string | null;
  sort: SortOrder;
  stats: LegacyStats;
  startup: boolean;
  startupError: string | null;
  captureHotkey: string;
  onArchiveAfterExportChange: (checked: boolean) => void;
  onArchiveAfterImportChange: (checked: boolean) => void;
  onCheckUpdates: () => void;
  onDownloadUpdate: () => void;
  onExportData: () => void;
  onImportData: () => void;
  onCancelDataImportPreview: () => void;
  onConfirmDataImportPreview: () => void;
  onMessageDoubleClickActionChange: (value: MessageDoubleClickAction) => void;
  onMatchBlankLinesToImagesChange: (checked: boolean) => void;
  onClose: () => void;
  onCloseToTrayChange: (checked: boolean) => void;
  onFontScaleChange: (value: number) => void;
  onHoverDelayChange: (value: number) => void;
  onShowHotkeyChange: (value: string) => void;
  onCaptureHotkeyChange: (value: string) => void;
  onMigrateLegacyData: () => void;
  onMoveAppDataDir: () => void;
  onRepairAppData: () => void;
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

  function changeMessageDoubleClickAction(value: MessageDoubleClickAction) {
    onMessageDoubleClickActionChange(value);
    showAutoSavedNotice("消息双击行为已应用");
  }

  function changeArchiveAfterImport(checked: boolean) {
    onArchiveAfterImportChange(checked);
    showAutoSavedNotice();
  }

  function changeMatchBlankLinesToImages(checked: boolean) {
    onMatchBlankLinesToImagesChange(checked);
    showAutoSavedNotice();
  }

  function changeArchiveAfterExport(checked: boolean) {
    onArchiveAfterExportChange(checked);
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

  function changeShowHotkey(value: string) {
    onShowHotkeyChange(value);
    showAutoSavedNotice("呼出界面快捷键已应用");
  }

  function changeCaptureHotkey(value: string) {
    onCaptureHotkeyChange(value);
    showAutoSavedNotice("导入剪切板快捷键已应用");
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
            <h2>设置</h2>
          </div>
          <button type="button" className="preview-close" onClick={onClose} aria-label="关闭设置">
            ×
          </button>
        </header>

        <div className="settings-body">
          <section className="settings-form" aria-label="应用设置">
            {!isAndroid && (
              <>
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
              </>
            )}

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

            <label className="setting-row setting-row-segmented">
              <span>消息双击</span>
              <div className="segmented" aria-label="消息双击行为">
                <button
                  type="button"
                  className={messageDoubleClickAction === "edit" ? "active" : ""}
                  onClick={() => changeMessageDoubleClickAction("edit")}
                >
                  编辑
                </button>
                <button
                  type="button"
                  className={messageDoubleClickAction === "create" ? "active" : ""}
                  onClick={() => changeMessageDoubleClickAction("create")}
                >
                  新建
                </button>
                <button
                  type="button"
                  className={messageDoubleClickAction === "none" ? "active" : ""}
                  onClick={() => changeMessageDoubleClickAction("none")}
                >
                  无效果
                </button>
              </div>
            </label>

            {!isAndroid && (
              <>
                <SettingToggle
                  checked={archiveAfterImport}
                  label="快速导入后自动归档"
                  description="导入完成后自动将消息移入已归档"
                  onChange={changeArchiveAfterImport}
                />

                <SettingToggle
                  checked={matchBlankLinesToImages}
                  label="空行与图片匹配导入"
                  description="每个空行按顺序替换为一张图片，未匹配图片仍追加到末尾"
                  onChange={changeMatchBlankLinesToImages}
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

                <HotkeyField label="呼出界面快捷键" value={showHotkey} onChange={changeShowHotkey} />

                <HotkeyField
                  label="导入当前剪切板快捷键"
                  value={captureHotkey}
                  onChange={changeCaptureHotkey}
                />

                {globalShortcutErrors.map((error) => (
                  <p className="inline-error" key={error}>
                    {error}
                  </p>
                ))}
              </>
            )}
            {isAndroid && (
              <SettingToggle
                checked={archiveAfterExport}
                label="导出后自动归档"
                description="数据包分享后自动将本次导出的消息移入已归档"
                onChange={changeArchiveAfterExport}
              />
            )}
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
                {releaseCheckResult.hasUpdate && releaseCheckResult.downloadAsset ? (
                  <button
                    type="button"
                    className="link-button"
                    onClick={onDownloadUpdate}
                    disabled={downloadingUpdate}
                  >
                    {downloadingUpdate ? "下载中..." : isAndroid ? "下载并安装" : "下载更新"}
                  </button>
                ) : (
                  releaseCheckResult.hasUpdate && (
                    <button
                      type="button"
                      className="link-button"
                      onClick={() => onOpenReleasePage(releaseCheckResult.releaseUrl)}
                    >
                      打开 Release 页面
                    </button>
                  )
                )}
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
              <button type="button" onClick={onExportData} disabled={exportingData}>
                {exportingData ? "导出中..." : "导出数据"}
              </button>
              <button type="button" onClick={onImportData} disabled={importingDataPackage}>
                {importingDataPackage ? "导入中..." : "导入数据"}
              </button>
              {!isAndroid && (
                <>
                  <button type="button" onClick={onMigrateLegacyData} disabled={migratingLegacyData}>
                    {migratingLegacyData ? "迁移中..." : "迁移旧数据"}
                  </button>
                  <button type="button" onClick={onMoveAppDataDir} disabled={movingAppData}>
                    {movingAppData ? "迁移中..." : "迁移数据目录"}
                  </button>
                  <button type="button" onClick={onRepairAppData} disabled={repairingAppData}>
                    {repairingAppData ? "修复中..." : "修复数据目录"}
                  </button>
                  <button type="button" onClick={() => onOpenPath(stats.data_dir)}>
                    打开数据目录
                  </button>
                  <button type="button" onClick={() => onOpenPath(stats.images_dir)}>
                    打开图片目录
                  </button>
                </>
              )}
            </div>

            {migrationResult && (
              <p className="settings-notice">
                新增 {migrationResult.inserted_messages} 条，跳过{" "}
                {migrationResult.skipped_messages} 条重复，复制图片{" "}
                {migrationResult.copied_images} 张。
              </p>
            )}
            {dataExportResult && (
              <p className="settings-notice">
                已导出 {dataExportResult.message_count} 条普通消息，图片{" "}
                {dataExportResult.image_count} 张。
              </p>
            )}
            {dataImportResult && (
              <p className="settings-notice">
                已导入 {dataImportResult.inserted_messages} 条，跳过{" "}
                {dataImportResult.skipped_messages} 条重复，图片{" "}
                {dataImportResult.imported_images} 张。
              </p>
            )}
            {migrationError && <p className="inline-error">迁移失败：{migrationError}</p>}
            {dataTransferError && <p className="inline-error">数据包处理失败：{dataTransferError}</p>}
            {openPathError && <p className="inline-error">打开目录失败：{openPathError}</p>}
          </section>
        </div>

      </section>
      {dataImportPreview && (
        <DataImportPreviewDialog
          importing={importingDataPackage}
          preview={dataImportPreview}
          onCancel={onCancelDataImportPreview}
          onConfirm={onConfirmDataImportPreview}
        />
      )}
    </div>
  );
}

function DataImportPreviewDialog({
  importing,
  preview,
  onCancel,
  onConfirm,
}: {
  importing: boolean;
  preview: DataImportPreview;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  return (
    <section
      aria-label="确认导入数据包"
      aria-modal="true"
      className="edit-dialog import-preview-dialog"
      role="dialog"
      onClick={(event) => event.stopPropagation()}
    >
      <header className="edit-header">
        <div>
          <p className="eyebrow">导入预览</p>
          <h2>确认导入数据包</h2>
        </div>
        <button type="button" className="preview-close" onClick={onCancel} aria-label="关闭导入预览">
          ×
        </button>
      </header>
      <div className="import-preview-summary">
        <p>数据包包含 {preview.total_messages} 条消息，图片 {preview.image_count} 张。</p>
        <p>
          将导入 {preview.inserted_messages} 条，跳过 {preview.skipped_messages} 条重复。
        </p>
        <code>{preview.path}</code>
      </div>
      <div className="dialog-actions">
        <button type="button" className="secondary-action" onClick={onCancel} disabled={importing}>
          取消
        </button>
        <button type="button" className="write-submit" onClick={onConfirm} disabled={importing}>
          {importing ? "导入中..." : "确认导入"}
        </button>
      </div>
    </section>
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

function HotkeyField({
  label,
  onChange,
  value,
}: {
  label: string;
  onChange: (value: string) => void;
  value: string;
}) {
  function handleKeyDown(event: ReactKeyboardEvent<HTMLInputElement>) {
    event.preventDefault();
    event.stopPropagation();

    const hotkey = formatKeyboardShortcut(event);
    if (hotkey) onChange(hotkey);
  }

  return (
    <label className="setting-field">
      <span>{label}</span>
      <input
        aria-label={label}
        value={value}
        readOnly
        onKeyDown={handleKeyDown}
        onFocus={(event) => event.currentTarget.select()}
      />
    </label>
  );
}

type MessageListProps = {
  archivingMessageId: number | null;
  expandedImageMessageIds: number[];
  hasMore: boolean;
  isAndroid: boolean;
  importingMessageId: number | null;
  listRef: RefObject<HTMLElement | null>;
  loadingMore: boolean;
  messages: LegacyMessage[];
  onArchive: (message: LegacyMessage) => void;
  onCopyText: (message: LegacyMessage) => void;
  onDelete: (message: LegacyMessage) => void;
  onEdit: (message: LegacyMessage) => void;
  onMessageDoubleClick: (message: LegacyMessage) => void;
  onLoadMore: () => void;
  onOpenImportQueue: (message: LegacyMessage) => void;
  onBlankDoubleClick: () => void;
  onToggleImages: (messageId: number) => void;
  onPreview: (image: PreviewImage | null) => void;
  previewDelaySeconds: number;
  scrollLines: number;
  showExternalImport: boolean;
};

export function MessageList({
  archivingMessageId,
  expandedImageMessageIds,
  hasMore,
  isAndroid,
  importingMessageId,
  listRef,
  loadingMore,
  messages,
  onArchive,
  onCopyText,
  onDelete,
  onEdit,
  onMessageDoubleClick,
  onLoadMore,
  onOpenImportQueue,
  onBlankDoubleClick,
  onToggleImages,
  onPreview,
  previewDelaySeconds,
  scrollLines,
  showExternalImport,
}: MessageListProps) {
  const virtualized = messages.length > 60;
  const [focusedId, setFocusedId] = useState<number | null>(null);
  const getItemKey = useCallback((index: number) => messages[index].id, [messages]);
  const focusedIndex = useMemo(() => messages.findIndex((message) => message.id === focusedId), [messages, focusedId]);
  const rangeExtractor = useCallback((range: Parameters<typeof defaultRangeExtractor>[0]) => {
    const indexes = defaultRangeExtractor(range);
    if (focusedIndex >= 0 && !indexes.includes(focusedIndex)) indexes.push(focusedIndex);
    return indexes.sort((a, b) => a - b);
  }, [focusedIndex]);
  const virtualizer = useVirtualizer({
    count: messages.length, getScrollElement: () => listRef.current,
    getItemKey, estimateSize: () => 180, overscan: 5, gap: 10,
    enabled: virtualized, rangeExtractor,
    initialRect: { width: 370, height: 600 },
    initialOffset: () => listRef.current?.scrollTop ?? 0,
  });
  useEffect(() => {
    const element = listRef.current;
    if (!virtualized || !element || typeof ResizeObserver === "undefined") return;
    let width = element.clientWidth;
    const observer = new ResizeObserver(() => {
      if (element.clientWidth !== width) {
        width = element.clientWidth;
        virtualizer.measure();
      }
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [virtualized, virtualizer, listRef]);
  const textCopyTimerRef = useRef<number | null>(null);

  function clearTextCopyTimer() {
    if (textCopyTimerRef.current !== null) {
      window.clearTimeout(textCopyTimerRef.current);
      textCopyTimerRef.current = null;
    }
  }

  function scheduleTextCopy(message: LegacyMessage) {
    clearTextCopyTimer();
    textCopyTimerRef.current = window.setTimeout(() => {
      textCopyTimerRef.current = null;
      onCopyText(message);
    }, MESSAGE_DOUBLE_CLICK_DELAY_MS);
  }

  function requestMoreIfNearBottom(element: HTMLElement) {
    if (!hasMore || loadingMore) return;

    const remaining = element.scrollHeight - element.scrollTop - element.clientHeight;
    if (remaining <= 160) {
      onLoadMore();
    }
  }

  const stableArchive = useCommittedCallback(onArchive);
  const stableDelete = useCommittedCallback(onDelete);
  const stableEdit = useCommittedCallback(onEdit);
  const stableMessageDoubleClick = useCommittedCallback(onMessageDoubleClick);
  const stableOpenImportQueue = useCommittedCallback(onOpenImportQueue);
  const stableToggleImages = useCommittedCallback(onToggleImages);
  const stablePreview = useCommittedCallback(onPreview);
  const stableCopy = useCommittedCallback(scheduleTextCopy);
  const stableCancelCopy = useCommittedCallback(clearTextCopyTimer);
  useEffect(() => () => clearTextCopyTimer(), []);
  function renderCard(message: LegacyMessage) {
    return <MessageCard key={message.id} message={message}
      isExpanded={expandedImageMessageIds.includes(message.id)}
      archivingMessageId={archivingMessageId} importingMessageId={importingMessageId}
      isAndroid={isAndroid} previewDelaySeconds={previewDelaySeconds}
      showExternalImport={showExternalImport} onCopyText={stableCopy} onCancelTextCopy={stableCancelCopy}
      onArchive={stableArchive}
      onDelete={stableDelete}
      onEdit={stableEdit}
      onMessageDoubleClick={stableMessageDoubleClick}
      onOpenImportQueue={stableOpenImportQueue}
      onToggleImages={stableToggleImages}
      onPreview={stablePreview}
    />;
  }
  const requestMore = useCommittedCallback(requestMoreIfNearBottom);

  // React 19 在 root 上以 passive:true 绑定合成 wheel 事件，preventDefault() 会被忽略，
  // 导致 scrollLines>1 时原生滚动与手动滚动叠加；改为在列表容器上用非 passive 原生监听，
  // scrollLines===1 时不做拦截、保持原生滚动语义。
  useEffect(() => {
    const element = listRef.current;
    if (!element) return;

    const handleNativeWheel = (event: globalThis.WheelEvent) => {
      if (scrollLines === 1) {
        window.setTimeout(() => {
          if (listRef.current) requestMore(listRef.current);
        }, 0);
        return;
      }

      event.preventDefault();
      if (listRef.current) {
        listRef.current.scrollTop += event.deltaY * scrollLines;
        requestMore(listRef.current);
      }
    };

    element.addEventListener("wheel", handleNativeWheel, { passive: false });
    return () => element.removeEventListener("wheel", handleNativeWheel);
  }, [scrollLines, listRef, requestMore]);

  return (
    <section
      className={`message-list${virtualized ? " message-list-virtual" : ""}`}
      onFocusCapture={(event) => {
        const row = (event.target as HTMLElement).closest<HTMLElement>("[data-message-id]");
        setFocusedId(row ? Number(row.dataset.messageId) : null);
      }}
      onBlurCapture={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget as Node | null)) setFocusedId(null);
      }}
      aria-label="消息列表"
      ref={listRef}
      onScroll={(event) => {
        // A virtual row can unmount without a mouseleave event. Cancel its outstanding preview.
        if (!isAndroid) {
          void closeHoverPreviewWindow();
          stablePreview(null);
        }
        requestMoreIfNearBottom(event.currentTarget);
      }}
      onDoubleClick={(event) => {
        if (event.target === event.currentTarget || (event.target as HTMLElement).classList.contains("message-virtual-space")) {
          onBlankDoubleClick();
        }
      }}
    >
      {virtualized ? (
        <div className="message-virtual-space" style={{ height: virtualizer.getTotalSize() }}>
          {virtualizer.getVirtualItems().map((row) => (
            <div key={row.key} data-index={row.index} data-message-id={messages[row.index].id}
              ref={virtualizer.measureElement} className="message-virtual-row"
              style={{ transform: `translateY(${row.start}px)` }}>
              {renderCard(messages[row.index])}
            </div>
          ))}
        </div>
      ) : messages.map(renderCard)}
    </section>
  );
}

const MessageCard = memo(function MessageCard({
  message, isExpanded, onCancelTextCopy,
  archivingMessageId, importingMessageId, isAndroid, onArchive, onCopyText, onDelete, onEdit, onMessageDoubleClick, onOpenImportQueue, onToggleImages, onPreview, previewDelaySeconds, showExternalImport,
}: Pick<MessageListProps, "archivingMessageId" | "importingMessageId" | "isAndroid" | "onArchive" | "onCopyText" | "onDelete" | "onEdit" | "onMessageDoubleClick" | "onOpenImportQueue" | "onToggleImages" | "onPreview" | "previewDelaySeconds" | "showExternalImport"> & { message: LegacyMessage; isExpanded: boolean; onCancelTextCopy: () => void }) {
  const visibleImages = isExpanded ? message.images : message.images.slice(0, 3);
  const hiddenImageCount = message.images.length - visibleImages.length;
  const previewImages = buildPreviewImages(message.images);
  return (
          <article
            className="message-card"
            key={message.id}
            onDoubleClick={(event) => {
              const target = event.target as HTMLElement;
              if (target.closest(".message-actions") || target.closest(".image-expand-action")) {
                return;
              }
              onMessageDoubleClick(message);
            }}
          >
            <header className="message-meta">
              <div className="message-meta-text">
                <div className="message-time-line">
                  <strong>#{message.id}</strong>
                  <span>{formatLocalTime(message.created_at)}</span>
                </div>
                {message.archived && (
                  <span className="archived-at">
                    归档于 {message.archived_at ? formatLocalTime(message.archived_at) : "未知时间"}
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
                    {showExternalImport && (
                      <button
                        type="button"
                        disabled={importingMessageId !== null}
                        onClick={() => onOpenImportQueue(message)}
                      >
                        {importingMessageId === message.id ? "准备中..." : "导入"}
                      </button>
                    )}
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
                onDoubleClick={onCancelTextCopy}
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
                      isAndroid={isAndroid}
                      key={image.id}
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
});

function EditMessageDialog({
  canSplit,
  canSave,
  error,
  imageItems,
  inputKey,
  isAndroid,
  message,
  previewDelaySeconds,
  previewImages,
  saving,
  splitting,
  textAreaHeight,
  textDraft,
  onClose,
  onDropFiles,
  onFileChange,
  onPaste,
  onPreview,
  onRemoveFile,
  onSubmit,
  onSplit,
  onTextAreaHeightCommit,
  onTextChange,
}: {
  canSplit: boolean;
  canSave: boolean;
  error: string | null;
  imageItems: ComposerImageItem[];
  inputKey: number;
  isAndroid: boolean;
  message: LegacyMessage;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
  saving: boolean;
  splitting: boolean;
  textAreaHeight: number;
  textDraft: string;
  onClose: () => void;
  onDropFiles: (files: File[]) => void;
  onFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onPaste: (event: ClipboardEvent<HTMLTextAreaElement>) => void;
  onPreview: (image: PreviewImage | null) => void;
  onRemoveFile: (index: number) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onSplit: () => void;
  onTextAreaHeightCommit: (height: number) => void;
  onTextChange: (text: string) => void;
}) {
  return (
    <MessageComposerDialog
      canSplit={canSplit}
      canSave={canSave}
      closeAriaLabel="关闭编辑"
      dialogLabel={`编辑消息 ${message.id}`}
      error={error}
      errorTitle="保存失败"
      eyebrow={`消息 #${message.id}`}
      fileInputId="edit-message-files"
      imageItems={imageItems}
      inputKey={inputKey}
      isAndroid={isAndroid}
      onClose={onClose}
      onDropFiles={onDropFiles}
      onFileChange={onFileChange}
      onPaste={onPaste}
      onPreview={onPreview}
      onRemoveFile={onRemoveFile}
      onSubmit={onSubmit}
      onSplit={onSplit}
      onTextAreaHeightCommit={onTextAreaHeightCommit}
      onTextChange={onTextChange}
      placeholder="编辑文字，或选择图片替换原图片"
      previewDelaySeconds={previewDelaySeconds}
      previewImages={previewImages}
      saving={saving}
      splitting={splitting}
      textAreaHeight={textAreaHeight}
      textAreaId="edit-message-text"
      textDraft={textDraft}
      title="编辑消息"
    />
  );
}

function MessageComposerDialog({
  autoFocus = false,
  canSplit = false,
  canSave,
  closeAriaLabel,
  dialogLabel,
  error,
  errorTitle,
  eyebrow,
  fileInputId,
  imageItems,
  inputKey,
  isAndroid = false,
  onClose,
  onDropFiles,
  onFileChange,
  onPaste,
  onPreview,
  onRemoveFile,
  onSubmit,
  onSplit,
  onTextAreaHeightCommit,
  onTextChange,
  placeholder,
  previewDelaySeconds,
  previewImages,
  saving,
  splitting = false,
  textAreaHeight,
  textAreaId,
  textDraft,
  title,
}: {
  autoFocus?: boolean;
  canSplit?: boolean;
  canSave: boolean;
  closeAriaLabel: string;
  dialogLabel: string;
  error: string | null;
  errorTitle: string;
  eyebrow: string;
  fileInputId: string;
  imageItems: ComposerImageItem[];
  inputKey: number;
  isAndroid?: boolean;
  onClose: () => void;
  onDropFiles: (files: File[]) => void;
  onFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onPaste: (event: ClipboardEvent<HTMLTextAreaElement>) => void;
  onPreview: (image: PreviewImage | null) => void;
  onRemoveFile: (index: number) => void;
  onSubmit: (event: FormEvent<HTMLFormElement>) => void;
  onSplit?: () => void;
  onTextAreaHeightCommit: (height: number) => void;
  onTextChange: (text: string) => void;
  placeholder: string;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
  saving: boolean;
  splitting?: boolean;
  textAreaHeight: number;
  textAreaId: string;
  textDraft: string;
  title: string;
}) {
  const textAreaRef = useRef<HTMLTextAreaElement | null>(null);
  const formId = `${textAreaId}-form`;

  useEffect(() => {
    if (!autoFocus) return;
    window.setTimeout(() => textAreaRef.current?.focus(), 0);
  }, [autoFocus]);

  function commitTextAreaHeight(
    event:
      | FocusEvent<HTMLTextAreaElement>
      | MouseEvent<HTMLTextAreaElement>
      | TouchEvent<HTMLTextAreaElement>,
  ) {
    const height = readTextAreaHeight(event.currentTarget);
    if (Number.isFinite(height) && height > 0) {
      onTextAreaHeightCommit(height);
    }
  }

  function handleDragOver(event: ReactDragEvent<HTMLElement>) {
    if (event.dataTransfer.types.includes("Files")) {
      event.preventDefault();
      event.dataTransfer.dropEffect = "copy";
    }
  }

  function handleDrop(event: ReactDragEvent<HTMLElement>) {
    const droppedFiles = Array.from(event.dataTransfer.files ?? []);
    if (droppedFiles.length === 0) return;

    event.preventDefault();
    event.stopPropagation();

    const imageFiles = droppedFiles.filter(isImageFile);
    const pathLines = droppedFiles
      .filter((file) => !isImageFile(file))
      .map(fileToDroppedPath)
      .filter(Boolean);

    if (imageFiles.length > 0) {
      onDropFiles(imageFiles);
    }
    if (pathLines.length > 0) {
      onTextChange(insertTextBlock(textDraft, pathLines.join("\n"), textAreaRef.current));
    }
  }

  function handleClose() {
    onPreview(null);
    onClose();
  }

  return (
    <div className="preview-backdrop edit-backdrop" role="presentation" onClick={handleClose}>
      <section
        aria-label={dialogLabel}
        aria-modal="true"
        className={`edit-dialog composer-dialog edit-message-dialog${isAndroid ? " composer-dialog-android" : ""}`}
        role="dialog"
        onDragOver={handleDragOver}
        onDrop={handleDrop}
        onClick={(event) => event.stopPropagation()}
      >
        <header className="edit-header composer-header">
          <div>
            <p className="eyebrow">{eyebrow}</p>
            <h2>{title}</h2>
          </div>
          {isAndroid ? (
            <div className="composer-mobile-actions">
              {onSplit && (
                <button
                  type="button"
                  className="split-action"
                  disabled={!canSplit || saving || splitting}
                  onClick={onSplit}
                >
                  {splitting ? "拆分中" : "拆分"}
                </button>
              )}
              <label className="composer-file-action" htmlFor={fileInputId} aria-label="选择图片" title="选择图片">
                图片
              </label>
              <button type="submit" form={formId} className="write-submit" disabled={!canSave}>
                {saving ? "保存中" : "保存"}
              </button>
              <button type="button" className="preview-close" onClick={handleClose} aria-label={closeAriaLabel}>
                ×
              </button>
            </div>
          ) : (
            <div className="composer-header-actions">
              {onSplit && (
                <button
                  type="button"
                  className="split-action"
                  disabled={!canSplit || saving || splitting}
                  onClick={onSplit}
                >
                  {splitting ? "拆分中" : "拆分"}
                </button>
              )}
              <button type="button" className="preview-close" onClick={handleClose} aria-label={closeAriaLabel}>
                ×
              </button>
            </div>
          )}
        </header>

        <form id={formId} className="text-create-form" onSubmit={onSubmit}>
          <section className="message-composer-box">
            <textarea
              ref={textAreaRef}
              id={textAreaId}
              aria-label="消息内容"
              value={textDraft}
              onChange={(event) => onTextChange(event.target.value)}
              onBlur={commitTextAreaHeight}
              onMouseUp={commitTextAreaHeight}
              onPaste={onPaste}
              onTouchEnd={commitTextAreaHeight}
              placeholder={placeholder}
              rows={9}
              style={{
                fontSize: isAndroid ? "1.3em" : undefined,
                height: `${textAreaHeight}px`,
              }}
            />

            {imageItems.length > 0 && (
              <div className="composer-image-grid" aria-label="已选图片">
                {imageItems.map((item, index) => (
                  <ComposerImageTile
                    item={item}
                    index={index}
                    isAndroid={isAndroid}
                    key={`${item.id}-${index}`}
                    onPreview={onPreview}
                    onRemove={onRemoveFile}
                    previewDelaySeconds={previewDelaySeconds}
                    previewImages={previewImages}
                  />
                ))}
              </div>
            )}
          </section>

          <input
            key={inputKey}
            id={fileInputId}
            className="composer-file-input"
            type="file"
            accept="image/*"
            multiple
            onChange={onFileChange}
          />

          {!isAndroid && (
            <div className="dialog-actions edit-dialog-actions">
            <label className="composer-file-action" htmlFor={fileInputId}>
              选择图片
            </label>
            <button type="button" className="secondary-action" onClick={handleClose}>
              关闭
            </button>
            <button type="submit" className="write-submit" disabled={!canSave}>
              {saving ? "正在保存..." : "保存"}
            </button>
            </div>
          )}
        </form>

        {error && (
          <OperationFeedback variant="error" title={errorTitle}>
            <p>{error}</p>
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
            <p className="eyebrow">删除确认</p>
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
  item,
  index,
  isAndroid = false,
  onPreview,
  onRemove,
  previewDelaySeconds,
  previewImages,
}: {
  item: ComposerImageItem;
  index: number;
  isAndroid?: boolean;
  onPreview: (image: PreviewImage | null) => void;
  onRemove?: (index: number) => void;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
}) {
  const thumbnail = useThumbnail(item.kind === "existing" ? item.image : null);
  const previewRequestRef = useRef(0);
  useEffect(() => () => clearPreviewTimer(), []);
  const previewTimerRef = useRef<number | null>(null);
  const previewImage = previewImages[index] ?? null;
  const tileSrc = item.kind === "existing" ? thumbnail.src : previewImage?.src;
  const filename = composerImageItemFilename(item);
  const title =
    item.kind === "file"
      ? `${item.file.name} · ${formatBytes(item.file.size)}`
      : `${item.image.filename} · ${item.image.path}`;

  function clearPreviewTimer() {
    previewRequestRef.current += 1;
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
  }

  function showPreview(target: HTMLButtonElement) {
    if (!previewImage || (item.kind === "existing" ? !item.image.exists : !tileSrc)) return;

    clearPreviewTimer();
    const img = target.querySelector("img");
    const naturalWidth = img?.naturalWidth && img.naturalWidth > 0 ? img.naturalWidth : 320;
    const naturalHeight = img?.naturalHeight && img.naturalHeight > 0 ? img.naturalHeight : 240;
    const anchor = target.getBoundingClientRect();
    const nextPreview = {
      ...previewImage,
      images: previewImages,
      index,
      position: calculatePreviewPosition(anchor, naturalWidth, naturalHeight),
      total: previewImages.length,
    };

    if (isAndroid) {
      onPreview(nextPreview);
      return;
    }

    const request = previewRequestRef.current;
    previewTimerRef.current = window.setTimeout(() => {
      previewTimerRef.current = null;
      showHoverPreviewWindow(nextPreview, anchor).catch(() => {
        if (request === previewRequestRef.current) onPreview(nextPreview);
      });
    }, Math.max(0, previewDelaySeconds * 1000));
  }

  function hidePreview() {
    clearPreviewTimer();
    if (!isAndroid) {
      closeHoverPreviewWindow();
    }
    onPreview(null);
  }

  function removeFile(event: MouseEvent<HTMLButtonElement>) {
    event.preventDefault();
    event.stopPropagation();
    hidePreview();
    onRemove?.(index);
  }

  return (
    <div className="composer-image-tile-wrap" ref={thumbnail.element}>
      <button
        type="button"
        className="composer-image-tile"
        onClick={(event) => {
          if (isAndroid) {
            event.preventDefault();
            event.stopPropagation();
            showPreview(event.currentTarget);
          }
        }}
        onMouseEnter={isAndroid ? undefined : (event) => showPreview(event.currentTarget)}
        onMouseLeave={isAndroid ? undefined : hidePreview}
        onFocus={isAndroid ? undefined : (event) => showPreview(event.currentTarget)}
        onBlur={isAndroid ? undefined : hidePreview}
        title={title}
      >
        {tileSrc ? (
          <img alt={filename} src={tileSrc} decoding="async" />
        ) : (
          <span className="composer-image-placeholder">
            {item.kind === "existing" && !item.image.exists ? "文件缺失" : thumbnail.failed ? "无法读取" : "加载中"}
          </span>
        )}
        <span>{filename}</span>
      </button>
      {onRemove && (
        <button
          type="button"
          className="composer-image-remove"
          onClick={removeFile}
          aria-label={`删除图片 ${filename}`}
          title="删除图片"
        >
          ×
        </button>
      )}
    </div>
  );
}

function MessageImageTile({
  image,
  isAndroid = false,
  onPreview,
  previewDelaySeconds,
  previewImages,
}: {
  image: LegacyMessageImage;
  isAndroid?: boolean;
  onPreview: (image: PreviewImage | null) => void;
  previewDelaySeconds: number;
  previewImages: PreviewImageItem[];
}) {
  const { element, src, failed } = useThumbnail(image);
  const [broken, setBroken] = useState(false);
  const previewTimerRef = useRef<number | null>(null);
  const previewRequestRef = useRef(0);
  useEffect(() => { setBroken(false); }, [src]);
  useEffect(() => () => clearPreviewTimer(), []);
  const imageFailed = failed || broken;
  const canRenderImage = image.exists && !imageFailed;
  const imageSrc = canRenderImage ? src : "";
  const previewIndex = previewImages.findIndex((item) => item.path === image.path);

  function clearPreviewTimer() {
    previewRequestRef.current += 1;
    if (previewTimerRef.current !== null) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
  }

  function readImagePreview(target: HTMLButtonElement) {
    const img = target.querySelector("img");
    const naturalWidth = img?.naturalWidth && img.naturalWidth > 0 ? img.naturalWidth : 320;
    const naturalHeight = img?.naturalHeight && img.naturalHeight > 0 ? img.naturalHeight : 240;
    const anchor = target.getBoundingClientRect();
    const preview = {
      filename: image.filename,
      images: previewImages,
      index: previewIndex >= 0 ? previewIndex : 0,
      path: image.path,
      position: calculatePreviewPosition(
        anchor,
        naturalWidth,
        naturalHeight,
      ),
      src: "",
      total: previewImages.length,
    };

    return { anchor, preview };
  }

  function showPreview(target: HTMLButtonElement) {
    if (!image.exists) return;

    clearPreviewTimer();
    const { anchor, preview } = readImagePreview(target);
    const request = previewRequestRef.current;

    previewTimerRef.current = window.setTimeout(() => {
      previewTimerRef.current = null;
      onPreview({ ...preview, externalWindow: true });
      showHoverPreviewWindow(preview, anchor).catch(() => {
        if (request === previewRequestRef.current) onPreview(preview);
      });
    }, Math.max(0, previewDelaySeconds * 1000));
  }

  function openPreview(target: HTMLButtonElement) {
    if (!image.exists) return;

    clearPreviewTimer();
    const { preview } = readImagePreview(target);
    onPreview(preview);
  }

  function hidePreview() {
    clearPreviewTimer();
    closeHoverPreviewWindow();
    onPreview(null);
  }

  if (image.exists) {
    return (
      <div className="image-tile" title={image.path} ref={element}>
        <button
          type="button"
          className="image-preview-action"
          aria-label={image.filename}
          onClick={isAndroid ? (event) => openPreview(event.currentTarget) : undefined}
          onMouseEnter={isAndroid ? undefined : (event) => showPreview(event.currentTarget)}
          onMouseLeave={isAndroid ? undefined : hidePreview}
          onFocus={isAndroid ? undefined : (event) => showPreview(event.currentTarget)}
          onBlur={isAndroid ? undefined : hidePreview}
        >
          {imageSrc ? <img
            alt={image.filename}
            loading="lazy"
            decoding="async"
            src={imageSrc}
            onError={() => setBroken(true)}
          /> : <span className="image-placeholder">{imageFailed ? "无法读取" : "加载中"}</span>}
        </button>
        <span className="image-caption">{image.filename}</span>
      </div>
    );
  }

  return (
    <div className="image-tile image-tile-missing" title={image.path} ref={element}>
      <span className="image-placeholder">
        {!image.exists ? "文件缺失" : imageFailed ? "无法读取" : "加载中"}
      </span>
      <span className="image-caption">{image.filename}</span>
    </div>
  );
}

let originalReadTask: Promise<void> = Promise.resolve();

function readPreviewOriginal(filename: string, isCurrent: () => boolean) {
  const result = originalReadTask.then(async () => {
    if (!isCurrent()) throw new Error("Preview request released");
    const bytes = await readLegacyImageBytes(filename);
    if (!isCurrent()) throw new Error("Preview request released");
    return bytes;
  });
  // Keep only the queue tail, never a resolved promise containing a previous original's bytes.
  originalReadTask = result.then(() => undefined, () => undefined);
  return result;
}

function useOriginalPreview(image: PreviewImage): { src: string; failed?: boolean } {
  const [loaded, setLoaded] = useState<{ image: PreviewImage; src: string; failed?: boolean } | null>(null);
  useEffect(() => {
    if (image.src) return;
    let alive = true;
    let url: string | undefined;
    readPreviewOriginal(image.filename, () => alive).then((bytes) => {
      if (!alive) return;
      url = URL.createObjectURL(new Blob([new Uint8Array(bytes)], { type: mimeTypeFromImagePath(image.filename) }));
      setLoaded({ image, src: url });
    }).catch(() => {
      if (alive) setLoaded({ image, src: "", failed: true });
    });
    return () => { alive = false; if (url) URL.revokeObjectURL(url); };
  }, [image]);
  return image.src ? { src: image.src } : loaded?.image === image ? loaded : { src: "" };
}

function HoverImagePreview({ image }: {
  image: PreviewImage;
}) {
  const original = useOriginalPreview(image);
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
      {original.src ? <img alt={image.filename} src={original.src} /> : <span>{original.failed ? "无法读取原图" : "加载原图中"}</span>}
    </aside>
  );
}

function AndroidImagePreview({
  image,
  onClose,
}: {
  image: PreviewImage;
  onClose: () => void;
}) {
  const original = useOriginalPreview(image);
  return (
    <button
      type="button"
      className="android-image-preview"
      aria-label={`关闭图片预览 ${image.filename}`}
      onClick={onClose}
    >
      {original.src ? <img alt={image.filename} src={original.src} draggable={false} /> : <span>{original.failed ? "无法读取原图" : "加载原图中"}</span>}
    </button>
  );
}

async function showHoverPreviewWindow(image: PreviewImage, anchor: DOMRect) {
  await showDesktopPreview({ filename: image.filename, path: image.path,
    file: image.images[image.index]?.file },
    (width, height) => calculateScreenPreviewPosition(anchor, width, height));
}

async function closeHoverPreviewWindow() {
  await hideDesktopPreview().catch(() => undefined);
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
      src: "",
    }));
}

function fileToComposerItem(file: File, prefix: string): ComposerImageItem {
  return {
    file,
    id: `${prefix}:${file.name}:${file.size}:${file.lastModified}`,
    kind: "file",
  };
}

function existingImageToComposerItem(image: LegacyMessageImage): ComposerImageItem {
  return {
    id: `existing:${image.id}:${image.filename}`,
    image,
    kind: "existing",
  };
}

function composerImageItemToPreview(item: ComposerImageItem): PreviewImageItem {
  if (item.kind === "file") {
    return {
      filename: item.file.name,
      path: item.id,
      src: URL.createObjectURL(item.file),
      file: item.file,
    };
  }

  return {
    filename: item.image.filename,
    path: item.image.path,
    src: "",
  };
}

function composerImageItemFilename(item: ComposerImageItem) {
  return item.kind === "file" ? item.file.name : item.image.filename;
}

function isImageFile(file: File) {
  return file.type.startsWith("image/") || isImagePath(file.name);
}

function isImagePath(path: string) {
  return /\.(avif|bmp|gif|jpe?g|png|webp)$/i.test(path);
}

function isZipPath(path: string) {
  return /\.zip$/i.test(path);
}

function fileToDroppedPath(file: File) {
  const fileWithPath = file as File & { path?: string; webkitRelativePath?: string };
  return fileWithPath.path || fileWithPath.webkitRelativePath || "";
}

async function droppedPathToFile(path: string) {
  const bytes = await readDroppedFileBytes(path);
  const filename = path.split(/[\\/]/).pop() || "dropped-image.png";
  return new File([new Uint8Array(bytes)], filename, {
    type: mimeTypeFromImagePath(path),
  });
}

function mimeTypeFromImagePath(path: string) {
  const lowerPath = path.toLowerCase();
  if (lowerPath.endsWith(".jpg") || lowerPath.endsWith(".jpeg")) return "image/jpeg";
  if (lowerPath.endsWith(".gif")) return "image/gif";
  if (lowerPath.endsWith(".webp")) return "image/webp";
  if (lowerPath.endsWith(".bmp")) return "image/bmp";
  if (lowerPath.endsWith(".avif")) return "image/avif";
  return "image/png";
}

function appendTextBlock(currentText: string, textBlock: string) {
  if (!textBlock) return currentText;
  if (!currentText) return textBlock;
  const separator = currentText.endsWith("\n") ? "" : "\n";
  return `${currentText}${separator}${textBlock}`;
}

function splitMessageLines(text: string) {
  return text
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);
}

function insertTextBlock(
  currentText: string,
  textBlock: string,
  textArea: HTMLTextAreaElement | null,
) {
  if (!textBlock) return currentText;

  if (textArea && document.activeElement === textArea) {
    const start = textArea.selectionStart ?? currentText.length;
    const end = textArea.selectionEnd ?? start;
    const before = currentText.slice(0, start);
    const after = currentText.slice(end);
    return `${before}${textBlock}${after}`;
  }

  return appendTextBlock(currentText, textBlock);
}

function editImagesChanged(message: LegacyMessage | null, items: ComposerImageItem[]) {
  if (!message) return false;
  if (message.images.length !== items.length) return true;

  return items.some((item, index) => {
    const original = message.images[index];
    return item.kind !== "existing" || !original || item.image.id !== original.id;
  });
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

function base64ToNumberArray(value: string) {
  const binary = window.atob(value);
  const bytes = new Array<number>(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

async function loadAppData(view: MessageView, sort: SortOrder, search: string, retainedCount = PAGE_LIMIT) {
  const measurement = startPerformanceMeasurement("page-data-ready");
  try {
    return await Promise.all([
      getLegacyStats(),
      loadRetainedMessagePage(view, sort, search, retainedCount),
    ]);
  } finally {
    finishPerformanceMeasurement(measurement);
  }
}

async function loadRetainedMessagePage(view: MessageView, sort: SortOrder, search: string, count: number) {
  // The backend caps each query at 100. Rebuild only the prefix the user already loaded.
  const first = await listLegacyMessages({ view, sort, offset: 0, limit: Math.min(count, 100), search });
  if (count <= PAGE_LIMIT) return first;
  let result = first;
  while (result.has_more && result.messages.length < count) {
    const next = await listLegacyMessages({ view, sort, offset: result.messages.length,
      limit: Math.min(count - result.messages.length, 100), search });
    result = { ...next, offset: 0, messages: [...result.messages, ...next.messages] };
    if (!next.messages.length) break;
  }
  return result;
}

function startPerformanceMeasurement(name: string): PerformanceMeasurement | null {
  if (typeof performance === "undefined" || !performance.measure) return null;

  try {
    return {
      measure: `clipstash:${name}`,
      start: performance.now(),
    };
  } catch {
    return null;
  }
}

function finishPerformanceMeasurement(measurement: PerformanceMeasurement | null) {
  if (!measurement || typeof performance === "undefined") return;

  try {
    // Keep the most recent result for each bounded metric name available to diagnostics.
    performance.clearMeasures?.(measurement.measure);
    performance.measure(measurement.measure, {
      end: performance.now(),
      start: measurement.start,
    });
  } catch {
    // Performance entries are optional diagnostics and must never block data rendering.
  }
}

function getStoredSort(): SortOrder {
  return localStorage.getItem("clipstash.setting.messageSort") === "oldest"
    ? "oldest"
    : "newest";
}

function clampEditTextareaHeight(height: number) {
  return Math.round(
    Math.min(MAX_EDIT_TEXTAREA_HEIGHT, Math.max(MIN_EDIT_TEXTAREA_HEIGHT, height)),
  );
}

function readTextAreaHeight(element: HTMLTextAreaElement) {
  return Math.round(
    element.getBoundingClientRect().height ||
      element.clientHeight ||
      Number.parseFloat(element.style.height),
  );
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

function selectWindowsInstallerAsset(
  assets: Array<{ browser_download_url?: string; name?: string }>,
): ReleaseDownloadAsset | null {
  const candidates = assets
    .map((asset) => ({
      downloadUrl: asset.browser_download_url ?? "",
      filename: asset.name ?? "",
    }))
    .filter((asset) => asset.downloadUrl && asset.filename);

  const setup = candidates.find((asset) => /x64[-_.]?setup\.exe$/i.test(asset.filename));
  if (setup) return setup;

  const exe = candidates.find((asset) => /\.exe$/i.test(asset.filename));
  if (exe) return exe;

  return candidates.find((asset) => /\.msi$/i.test(asset.filename)) ?? null;
}

function selectAndroidApkAsset(
  assets: Array<{ browser_download_url?: string; name?: string }>,
): ReleaseDownloadAsset | null {
  const candidates = assets
    .map((asset) => ({
      downloadUrl: asset.browser_download_url ?? "",
      filename: asset.name ?? "",
    }))
    .filter((asset) => asset.downloadUrl && /\.apk$/i.test(asset.filename));

  const signedUniversal = candidates.find((asset) =>
    /android.*universal.*release.*signed\.apk$/i.test(asset.filename),
  );
  if (signedUniversal) return signedUniversal;

  return candidates.find((asset) => /universal.*\.apk$/i.test(asset.filename)) ?? candidates[0] ?? null;
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
  return Promise.all(files.map(fileToNumberArray));
}

async function composerImageItemsToNumberArrays(items: ComposerImageItem[]) {
  return Promise.all(
    items.map((item) =>
      item.kind === "file"
        ? fileToNumberArray(item.file)
        : existingImageToNumberArray(item.image),
    ),
  );
}

async function fileToNumberArray(file: File) {
  const buffer = await file.arrayBuffer();
  return Array.from(new Uint8Array(buffer));
}

async function openExportedDataPackage(filename: string, bytes: number[], path: string) {
  if (IS_ANDROID && window.ClipStashAndroid?.shareZip) {
    window.ClipStashAndroid.shareZip(path);
    return;
  }

  const buffer = new Uint8Array(bytes).buffer;
  const blob = new Blob([buffer], { type: "application/zip" });
  const file = new File([blob], filename, { type: "application/zip" });
  const shareData = {
    files: [file],
    title: "ClipStash 数据包",
  };
  const shareNavigator = navigator as Navigator & {
    canShare?: (data: typeof shareData) => boolean;
    share?: (data: typeof shareData) => Promise<void>;
  };

  if (shareNavigator.share && shareNavigator.canShare?.(shareData)) {
    await shareNavigator.share(shareData);
    return;
  }

  try {
    await openPath(path);
  } catch (err) {
    throw new Error(
      `系统分享不可用，且无法打开导出的 zip：${err instanceof Error ? err.message : String(err)}`,
    );
  }
}

async function existingImageToNumberArray(image: LegacyMessageImage) {
  if (!image.exists) {
    throw new Error(`图片文件不存在，不能保存：${image.filename}`);
  }
  return Array.from(await readLegacyImageBytes(image.filename));
}

function formatBytes(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

function formatKeyboardShortcut(event: ReactKeyboardEvent<HTMLInputElement>) {
  const key = event.key;
  if (
    key === "Control" ||
    key === "Shift" ||
    key === "Alt" ||
    key === "Meta" ||
    key === "Process" ||
    key === "Unidentified"
  ) {
    return "";
  }

  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.shiftKey) parts.push("Shift");
  if (event.altKey) parts.push("Alt");
  if (event.metaKey) parts.push("Super");

  const normalizedKey = key.length === 1 ? key.toUpperCase() : key;
  parts.push(normalizedKey);
  return parts.join("+");
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
