import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  ClipboardEntry,
  DropPayload,
  ExportedFile,
  SearchResult,
  SearchStatus,
  ShelfFile,
  ShelfHistoryEntry,
  StorageUsage,
  SynthesizedFile,
  TodoItem,
  UpdateChannel,
  UpdateInfo,
} from "../types";

export const ipc = {
  listClips: () => invoke<ClipboardEntry[]>("list_clipboard_entries"),
  getClip: (id: number) => invoke<ClipboardEntry>("get_clipboard_entry", { id }),
  updateClip: (id: number, content: string) =>
    invoke<ClipboardEntry>("update_clipboard_entry", { id, content }),
  deleteClip: (id: number) => invoke<boolean>("delete_clipboard_entry", { id }),
  clearClips: () => invoke<boolean>("clear_clipboard_entries"),
  exportClips: (ids: number[], format: string) =>
    invoke<SynthesizedFile[]>("export_clipboard_entries", { ids, format }),
  saveConvertedTextImage: (dataBase64: string, format: string) =>
    invoke<SynthesizedFile>("save_converted_text_image", { dataBase64, format }),
  openClipEditor: (id: number) => invoke<void>("open_clip_editor", { id }),
  clipboardWatchEnabled: () => invoke<boolean>("get_clipboard_watch_enabled"),
  setClipboardWatch: (enabled: boolean) =>
    invoke<boolean>("set_clipboard_watch_enabled", { enabled }),

  getSettings: () => invoke<AppSettings>("get_settings"),
  setSettings: (settings: AppSettings) =>
    invoke<AppSettings>("set_settings", { settings }),
  /** Whether Windows launches the island when the user signs in. */
  getAutostart: () => invoke<boolean>("get_autostart"),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  setMinifiedIsland: (enabled: boolean) =>
    invoke<void>("set_minified_island", { enabled }),
  /** Persists the active search engine without touching other settings. */
  setSearchEngine: (engineId: string) =>
    invoke<string>("set_search_engine", { engineId }),
  setIslandBounds: (expanded: boolean, minified: boolean, timerActive: boolean) =>
    invoke<void>("set_island_window_bounds", { expanded, minified, timerActive }),
  setIslandRegion: (width: number, height: number, pill: boolean) =>
    invoke<void>("set_island_window_region", { width, height, pill }),
  openSettingsWindow: () => invoke<void>("open_settings_window"),

  shelfItems: () => invoke<ShelfFile[]>("get_shelf_items"),
  /** Captures one or more drops/pastes; returns the stored inbox items. */
  ingestDrop: (payloads: DropPayload[]) => invoke<ShelfFile[]>("ingest_drop", { payloads }),
  /** Writes real files for the given items so they can be dragged out. */
  exportShelfItems: (ids: string[]) =>
    invoke<ExportedFile[]>("export_shelf_items", { ids }),
  removeShelfItem: (id: string) => invoke<boolean>("remove_shelf_item", { id }),
  /** Groups items into a folder (empty `targetId` creates an empty folder). */
  createShelfFolder: (targetId: string, sourceIds: string[]) =>
    invoke<ShelfFile[]>("create_shelf_folder", { targetId, sourceIds }),
  /** Moves one item in or out of a folder; empty `parentId` = main list. */
  moveShelfItem: (id: string, parentId: string) =>
    invoke<ShelfFile>("move_shelf_item", { id, parentId }),
  /** Renames a tile; empty title restores the automatic name. */
  renameShelfItem: (id: string, title: string) =>
    invoke<ShelfFile>("rename_shelf_item", { id, title }),
  /** Fetches (or refreshes) title/description/image for a link. */
  refreshLinkMetadata: (id: string) =>
    invoke<ShelfFile>("refresh_link_metadata", { id }),
  clearShelf: () => invoke<boolean>("clear_shelf_items"),
  openShelfItem: (id: string) => invoke<void>("open_shelf_item", { id }),
  setShelfPinned: (id: string, pinned: boolean) =>
    invoke<ShelfFile>("set_shelf_pinned", { id, pinned }),
  copyShelfFiles: (ids: string[]) => invoke<number>("copy_shelf_items", { ids }),
  shelfHistory: () => invoke<ShelfHistoryEntry[]>("list_shelf_history"),

  copyClip: (id: number) => invoke<void>("copy_clipboard_entry", { id }),
  /** Copies any file the backend handed us (export, shelf item) to `target`. */
  saveFileAs: (source: string, target: string) =>
    invoke<string>("save_shelf_item", { path: source, target }),

  listTodos: () => invoke<TodoItem[]>("list_todos"),
  addTodo: (label: string) => invoke<TodoItem>("add_todo", { label }),
  setTodoDone: (id: string, done: boolean) =>
    invoke<TodoItem>("set_todo_done", { id, done }),
  deleteTodo: (id: string) => invoke<boolean>("delete_todo", { id }),

  /** Detects Everything and the browsers installed on this machine. */
  searchStatus: () => invoke<SearchStatus>("search_status"),
  /** Disk usage of the database, attachments and generated previews. */
  storageUsage: () => invoke<StorageUsage>("storage_usage"),
  /** Clears history + unpinned items and compacts the database. */
  clearStorage: () => invoke<StorageUsage>("clear_storage"),
  /** Newer release, or `null` when the app is up to date. */
  checkForUpdate: () => invoke<UpdateInfo | null>("check_for_update"),
  /** Where this copy is updated from (Store vs direct download). */
  updateChannel: () => invoke<UpdateChannel>("update_channel"),
  /** Live file lookup through Everything; rejects when it is not available. */
  everythingSearch: (query: string, limit = 40) =>
    invoke<SearchResult[]>("everything_search", { query, limit }),
  /** Opens a search URL in the configured (or default) browser. */
  openExternal: (url: string, browser?: string | null) =>
    invoke<void>("open_external", { url, browser: browser ?? null }),
  openSearchResult: (path: string) => invoke<void>("open_search_result", { path }),
};

/** Turns a filesystem path into a URL the webview can render. */
export function fileUrl(path: string): string {
  if (!path) return "";
  try {
    // `asset` must match the enabled Tauri asset protocol in the app config.
    return convertFileSrc(path, "asset");
  } catch {
    return "";
  }
}

/**
 * Starts a native OS drag operation for the given files so they can be
 * dropped into Explorer, Slack, VS Code, or a browser.
 */
export async function dragFilesOut(paths: string[]): Promise<void> {
  if (!paths.length) return;

  const { startDrag } = await import("@crabnebula/tauri-plugin-drag");
  const icon = await dragPreviewIcon();
  await startDrag({ item: paths, icon });
}

let cachedDragIcon: string | null = null;

/** Warms the cached drag icon so the first drag-out starts without a hitch. */
export async function prewarmDragIcon(): Promise<void> {
  try {
    await dragPreviewIcon();
  } catch {
    // Only available inside the Tauri runtime.
  }
}

async function dragPreviewIcon(): Promise<string> {
  if (!cachedDragIcon) {
    cachedDragIcon = await invoke<string>("get_drag_icon");
  }
  return cachedDragIcon;
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kb = bytes / 1024;
  if (kb < 1024) return `${kb.toFixed(0)} KB`;
  return `${(kb / 1024).toFixed(1)} MB`;
}
