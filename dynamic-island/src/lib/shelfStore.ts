import { listen } from "@tauri-apps/api/event";
import { ipc } from "./ipc";
import type { ShelfFile } from "../types";

/**
 * Shared shelf state that lives outside of React.
 *
 * The expanded panel unmounts whenever the island collapses (AnimatePresence
 * swaps the expanded/collapsed trees), so keeping the file list inside
 * `ShelfWidget` meant every remount repainted an empty shelf and then waited
 * for `get_shelf_items` to come back — the visible "disappear and reappear".
 * Holding the list here makes remounts paint the last known items instantly.
 */

let files: ShelfFile[] = [];
let loaded = false;
let inflight: Promise<void> | null = null;
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of listeners) listener();
}

function sameFiles(a: ShelfFile[], b: ShelfFile[]): boolean {
  if (a.length !== b.length) return false;
  return a.every((item, index) => {
    const other = b[index];
    return (
      item.id === other.id &&
      item.name === other.name &&
      item.size_bytes === other.size_bytes &&
      item.thumb_path === other.thumb_path &&
      item.thumb_kind === other.thumb_kind &&
      item.pinned === other.pinned &&
      item.parent_id === other.parent_id &&
      item.meta_image === other.meta_image &&
      item.meta_favicon === other.meta_favicon &&
      item.meta_site === other.meta_site
    );
  });
}

export const shelfStore = {
  getSnapshot: (): ShelfFile[] => files,
  isLoaded: (): boolean => loaded,

  subscribe(listener: () => void): () => void {
    listeners.add(listener);
    return () => {
      listeners.delete(listener);
    };
  },

  /** Shows freshly dropped items right away, before the round trip finishes. */
  upsert(items: ShelfFile[]): void {
    const known = new Set(files.map((file) => file.id));
    const additions = items.filter((item) => !known.has(item.id));
    if (!additions.length) return;
    // Newest first, matching the backend ordering.
    files = [...additions, ...files];
    loaded = true;
    emit();
  },

  /** Optimistic local removal so the tile leaves immediately. */
  remove(id: string): void {
    const next = files.filter((file) => file.id !== id);
    if (next.length === files.length) return;
    files = next;
    emit();
  },

  clear(): void {
    if (!files.length) return;
    files = [];
    emit();
  },

  /** Re-reads the shelf; concurrent callers share a single request. */
  refresh(): Promise<void> {
    if (inflight) return inflight;
    inflight = (async () => {
      try {
        const next = await ipc.shelfItems();
        loaded = true;
        if (!sameFiles(files, next)) {
          files = next;
          emit();
        }
      } finally {
        inflight = null;
      }
    })();
    return inflight;
  },
};

let subscribed = false;

/** Subscribes to backend shelf updates (e.g. previews restored at startup). */
export function ensureShelfEvents(): void {
  if (subscribed) return;
  subscribed = true;
  listen("shelf-changed", () => {
    void shelfStore.refresh();
  }).catch(() => {
    subscribed = false;
  });
}
