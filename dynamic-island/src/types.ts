export type WidgetKey =
  | "clipboard"
  | "shelf"
  | "search"
  | "todo"
  | "media"
  | "pomodoro";

export interface ClipboardResult {
  path: string;
  extension: string;
  kind: string;
}

export interface ClipboardEntry {
  id: number;
  kind: string;
  preview: string;
  content: string;
  image_path: string;
  created_at: string;
}

export interface SynthesizedFile {
  path: string;
  extension: string;
  kind: string;
}

export interface AppSettings {
  default_text_format: string;
  default_image_format: string;
  minified_island: boolean;
  /** Hours before unpinned shelf items expire; 0 means never. */
  shelf_retention_hours: number;
  /** `ctrl+alt` | `ctrl+shift` | `alt+shift` | `ctrl+alt+shift` | `off` */
  click_through_shortcut: string;
  /** Builtin engine id (`google` | `youtube` | `duckduckgo`) or a custom id. */
  search_engine: string;
  /** User-defined engines; `url` must contain the `{query}` placeholder. */
  search_engines: SearchEngine[];
  /** `default` or the absolute path of a browser executable. */
  search_browser: string;
  /** Everything HTTP server port. */
  search_everything_port: number;
  /** Optional absolute path to `es.exe`; empty means auto-detect. */
  search_es_path: string;
  /** `glass` (translucent + blur), `amoled` (pure black) or `custom`. */
  island_background: string;
  /** Custom surface opacity in percent (10-100). */
  island_opacity: number;
  /** Custom surface backdrop blur radius in px (0-40). */
  island_blur: number;
}

export interface SearchEngine {
  id: string;
  name: string;
  /** URL template; `{query}` is replaced with the encoded search term. */
  url: string;
}

export interface SearchResult {
  name: string;
  path: string;
  extension: string;
  size: number;
  /** Raw date string reported by Everything. */
  modified: string;
}

export interface BrowserOption {
  /** `default` or the absolute path of the browser executable. */
  id: string;
  name: string;
}

export interface SearchStatus {
  everything: boolean;
  /** `es` or `http` when Everything was located. */
  everything_via: string;
  everything_detail: string;
  browsers: BrowserOption[];
}

/** Disk usage reported by Settings → Storage. */
export interface StorageUsage {
  database_bytes: number;
  /** Clipboard image captures cached as PNGs. */
  clipboard_images_bytes: number;
  /** Shelf attachments (inline images, HTML assets). */
  shelf_files_bytes: number;
  previews_bytes: number;
  total_bytes: number;
  clipboard_items: number;
  shelf_items: number;
  pinned_items: number;
  /** Non-empty when the database had to be rebuilt at start-up. */
  database_note: string;
}

/** Where this copy of Pill gets its updates from. */
export type UpdateSource = "github" | "store";

/** Resolved once at start-up: Store copy or direct download. */
export interface UpdateChannel {
  source: UpdateSource;
  /** `GitHub` or `Microsoft Store`. */
  label: string;
  current_version: string;
  /** Windows owns updates: never download or launch an installer. */
  store_managed: boolean;
  /** The process runs from an MSIX package. */
  packaged: boolean;
  package_family_name: string;
  /** Store product page for Store copies, release page otherwise. */
  channel_url: string;
}

/** Newer release, when one exists. */
export interface UpdateInfo {
  current_version: string;
  latest_version: string;
  name: string;
  notes: string;
  release_url: string;
  download_name: string;
  published_at: string;
  source: UpdateSource;
  /** Button text picked by the backend for this install type. */
  action_label: string;
  /** Installer download or the Microsoft Store product page. */
  action_url: string;
}

export interface TodoItem {
  id: string;
  label: string;
  done: boolean;
}

export interface ShelfFile {
  id: string;
  /** `text` | `url` | `html` | `image` | `file` | `folder` */
  kind: string;
  /** Absolute path for `file`/`image`, empty for inline kinds. */
  path: string;
  /** Inline payload for `text`, `url` and `html`. */
  content: string;
  name: string;
  extension: string;
  mime: string;
  size_bytes: number;
  thumb_path: string;
  /** `cover` for photo previews, `contain` for Explorer thumbnails/icons. */
  thumb_kind: string;
  preview: string;
  assets: string[];
  pinned: boolean;
  /** Unix milliseconds. */
  created_at: number;
  /** Folder id when the item sits inside an organisational folder. */
  parent_id: string;
  /** Title the user typed; empty when the title is automatic. */
  custom_title: string;
  meta_title: string;
  meta_description: string;
  meta_site: string;
  /** Local og:image path. */
  meta_image: string;
  /** Local favicon path. */
  meta_favicon: string;
  meta_fetched_at: number;
}

/** One normalised drag/clipboard payload handed to the backend. */
export interface DropPayload {
  files?: string[];
  text?: string | null;
  html?: string | null;
  url?: string | null;
  image_base64?: string | null;
  image_mime?: string | null;
}

export interface ExportedFile {
  id: string;
  path: string;
  name: string;
}

export interface ShelfHistoryEntry {
  id: number;
  name: string;
  path: string;
  kind: string;
  created_at: number;
}
