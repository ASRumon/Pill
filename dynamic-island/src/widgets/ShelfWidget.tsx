import { useCallback, useEffect, useMemo, useRef, useState, useSyncExternalStore } from "react";
import type { MouseEvent } from "react";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { dragFilesOut, fileUrl, formatBytes, ipc, prewarmDragIcon } from "../lib/ipc";
import { ensureShelfEvents, shelfStore } from "../lib/shelfStore";
import { requestIslandCollapse } from "../lib/islandControl";
import { useSelectionKeys } from "../lib/selectionKeys";
import { ContextMenu } from "../components/ContextMenu";
import type { ContextMenuItem } from "../components/ContextMenu";
import { ShelfDetail, relativeTime } from "../components/ShelfDetail";
import { ActionIcon } from "../components/WidgetIcon";
import type { ExportedFile, ShelfFile, ShelfHistoryEntry } from "../types";

interface ShelfWidgetProps {
  dropSession?: {
    token: number;
    addedIds: string[];
  };
}

/** Survives remounts so collapsing/expanding never replays the arrival highlight. */
let lastArrivalToken = 0;

const EXT_COLORS: Record<string, string> = {
  png: "#38bdf8",
  jpg: "#38bdf8",
  jpeg: "#38bdf8",
  webp: "#38bdf8",
  gif: "#a78bfa",
  pdf: "#f87171",
  md: "#a3e635",
  txt: "#94a3b8",
  json: "#fbbf24",
  ts: "#60a5fa",
  tsx: "#60a5fa",
  js: "#facc15",
  css: "#c084fc",
  html: "#fb923c",
  zip: "#f472b6",
};

function glyphColor(extension: string): string {
  return EXT_COLORS[extension] ?? "#8b8b95";
}

/** One pointer gesture that may become an internal drop or a native drag-out. */
interface DragSession {
  file: ShelfFile;
  ids: string[];
  startX: number;
  startY: number;
  moved: boolean;
  started: boolean;
}

export function ShelfWidget({ dropSession, gridColumns = 2 }: ShelfWidgetProps & { gridColumns?: 1 | 2 | 3 }) {
  const files = useSyncExternalStore(shelfStore.subscribe, shelfStore.getSnapshot, shelfStore.getSnapshot);
  const [selectedIds, setSelectedIds] = useState<string[]>([]);
  const [newlyArrivedIds, setNewlyArrivedIds] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number; file: ShelfFile } | null>(null);
  const [detailId, setDetailId] = useState<string | null>(null);
  const [historyOpen, setHistoryOpen] = useState(false);
  const [history, setHistory] = useState<ShelfHistoryEntry[]>([]);
  const [folderId, setFolderId] = useState<string | null>(null);
  const [dragOverId, setDragOverId] = useState<string | null>(null);
  const [renameTarget, setRenameTarget] = useState<{ id: string; value: string } | null>(null);
  // Preview files live in %TEMP% and can be cleaned by the OS, so a failed load
  // falls back to the plain glyph instead of an empty tile.
  const [brokenMedia, setBrokenMedia] = useState<string[]>([]);
  const markBroken = useCallback((key: string) => {
    setBrokenMedia((prev) => (prev.includes(key) ? prev : [...prev, key]));
  }, []);
  const selectionAnchor = useRef<string | null>(null);

  const filesRef = useRef(files);
  filesRef.current = files;
  const selectedRef = useRef(selectedIds);
  selectedRef.current = selectedIds;
  const dragSession = useRef<DragSession | null>(null);
  const dragOverRef = useRef<string | null>(null);
  const suppressClickUntil = useRef(0);
  const dragPrewarm = useRef<{ key: string; promise: Promise<ExportedFile[]> } | null>(null);

  const refresh = useCallback(async () => {
    try {
      await shelfStore.refresh();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    ensureShelfEvents();
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (!historyOpen) return;
    ipc
      .shelfHistory()
      .then(setHistory)
      .catch((e) => setError(String(e)));
  }, [historyOpen]);

  const rootFiles = useMemo(() => files.filter((file) => !file.parent_id), [files]);
  const visibleFiles = useMemo(
    () => (folderId ? files.filter((file) => file.parent_id === folderId) : rootFiles),
    [files, folderId, rootFiles],
  );
  const folder = folderId ? (files.find((file) => file.id === folderId) ?? null) : null;

  const childCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const file of files) {
      if (!file.parent_id) continue;
      counts.set(file.parent_id, (counts.get(file.parent_id) ?? 0) + 1);
    }
    return counts;
  }, [files]);

  // A folder can disappear (cleared, removed elsewhere); fall back to the root.
  useEffect(() => {
    if (folderId && !files.some((file) => file.id === folderId)) setFolderId(null);
  }, [files, folderId]);

  useEffect(() => {
    setSelectedIds((prev) => prev.filter((id) => visibleFiles.some((file) => file.id === id)));
  }, [visibleFiles]);

  useEffect(() => {
    const token = dropSession?.token;
    if (!token || !dropSession?.addedIds.length) return;
    if (token === lastArrivalToken) return;
    lastArrivalToken = token;
    setNewlyArrivedIds(dropSession.addedIds);
    const timer = window.setTimeout(() => setNewlyArrivedIds([]), 2400);
    return () => window.clearTimeout(timer);
  }, [dropSession?.token]);

  const toggleSelection = (id: string, event: MouseEvent<HTMLDivElement>) => {
    const list = visibleFiles;
    const anchor = selectionAnchor.current;
    if (event.shiftKey) {
      const anchorIndex = list.findIndex((file) => file.id === (anchor ?? id));
      const targetIndex = list.findIndex((file) => file.id === id);
      if (anchorIndex >= 0 && targetIndex >= 0) {
        const range = list
          .slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1)
          .map((file) => file.id);
        setSelectedIds((prev) => [...new Set([...prev, ...range])]);
        return;
      }
    }

    if (event.ctrlKey || event.metaKey) {
      selectionAnchor.current = id;
      setSelectedIds((prev) =>
        prev.includes(id) ? prev.filter((value) => value !== id) : [...prev, id],
      );
      return;
    }

    selectionAnchor.current = id;
    setSelectedIds((prev) => (prev.length === 1 && prev[0] === id ? [] : [id]));
  };

  const dragKeyFor = (file: ShelfFile) => {
    const ids = selectedIds.includes(file.id) ? [...selectedIds].sort() : [file.id];
    return ids.join(",");
  };

  /** Native drags can only start once the files exist, so build them early. */
  const prewarmDrag = (file: ShelfFile) => {
    if (file.kind === "folder") return;
    void prewarmDragIcon();
    const key = dragKeyFor(file);
    if (dragPrewarm.current?.key === key) return;
    const ids = selectedIds.includes(file.id) ? selectedIds : [file.id];
    dragPrewarm.current = {
      key,
      promise: ipc.exportShelfItems(ids).catch(() => [] as ExportedFile[]),
    };
  };

  const startNativeDrag = async (_file: ShelfFile, ids: string[]) => {
    const key = ids.slice().sort().join(",");
    const warmed = dragPrewarm.current;
    dragPrewarm.current = null;
    try {
      // Inline items have no file yet: generate real files first.
      const exported =
        warmed && warmed.key === key ? await warmed.promise : await ipc.exportShelfItems(ids);
      const paths = exported.map((item) => item.path).filter(Boolean);
      if (paths.length) await dragFilesOut(paths);
    } catch (e) {
      setError(String(e));
    } finally {
      requestIslandCollapse();
    }
  };

  const groupItems = async (targetId: string, ids: string[]) => {
    try {
      await ipc.createShelfFolder(targetId, ids.filter((id) => id !== targetId));
      setSelectedIds([]);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const moveItems = async (ids: string[], folderTarget: string) => {
    try {
      await ipc.createShelfFolder(folderTarget, ids);
      setSelectedIds([]);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  // One window-level gesture tracker: hovering another tile groups the items,
  // leaving the island hands the gesture over to a native OS drag.
  useEffect(() => {
    const onMove = (event: PointerEvent) => {
      const session = dragSession.current;
      if (!session || session.started) return;
      const dx = event.clientX - session.startX;
      const dy = event.clientY - session.startY;
      if (!session.moved) {
        if (Math.hypot(dx, dy) < 6) return;
        session.moved = true;
        suppressClickUntil.current = Date.now() + 700;
      }

      const shell = document.querySelector(".island-shell");
      const rect = shell?.getBoundingClientRect();
      // Small dead zone: a one-pixel overshoot at the island edge must not
      // turn an internal group gesture into an OS drag.
      const margin = 18;
      const outside =
        !!rect &&
        (event.clientX < rect.left - margin ||
          event.clientX > rect.right + margin ||
          event.clientY < rect.top - margin ||
          event.clientY > rect.bottom + margin);
      if (outside) {
        session.started = true;
        dragSession.current = null;
        dragOverRef.current = null;
        setDragOverId(null);
        void startNativeDrag(session.file, session.ids);
        return;
      }

      const element = document.elementFromPoint(event.clientX, event.clientY) as HTMLElement | null;
      const tile = element?.closest<HTMLElement>("[data-shelf-id]");
      const targetId = tile?.dataset.shelfId ?? null;
      const target = targetId ? filesRef.current.find((file) => file.id === targetId) : null;
      const accepts =
        !!target && !session.ids.includes(target.id) && session.file.kind !== "folder";
      const next = accepts ? target!.id : null;
      if (dragOverRef.current !== next) {
        dragOverRef.current = next;
        setDragOverId(next);
      }
    };

    const onUp = () => {
      const session = dragSession.current;
      dragSession.current = null;
      const targetId = dragOverRef.current;
      dragOverRef.current = null;
      setDragOverId(null);
      if (!session || !session.moved || session.started || !targetId) return;
      const target = filesRef.current.find((file) => file.id === targetId);
      if (!target) return;
      if (target.kind === "folder") {
        void moveItems(session.ids, target.id);
      } else {
        void groupItems(target.id, session.ids);
      }
    };

    const onCancel = () => {
      dragSession.current = null;
      dragOverRef.current = null;
      setDragOverId(null);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onCancel);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onCancel);
    };
  }, []);

  const moveOutOfFolder = async (file: ShelfFile) => {
    try {
      await ipc.moveShelfItem(file.id, "");
      setSelectedIds((prev) => prev.filter((id) => id !== file.id));
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const createEmptyFolder = async () => {
    try {
      await ipc.createShelfFolder("", []);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const applyRename = async () => {
    if (!renameTarget) return;
    try {
      await ipc.renameShelfItem(renameTarget.id, renameTarget.value);
      setRenameTarget(null);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const refreshLink = async (file: ShelfFile) => {
    try {
      const updated = await ipc.refreshLinkMetadata(file.id);
      shelfStore.upsert([updated]);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const remove = async (id: string) => {
    shelfStore.remove(id);
    setSelectedIds((prev) => prev.filter((value) => value !== id));
    try {
      await ipc.removeShelfItem(id);
    } catch (e) {
      setError(String(e));
    }
    await refresh();
  };

  const clear = async () => {
    // Pinned files survive a clear, so no optimistic wipe here.
    try {
      await ipc.clearShelf();
    } catch (e) {
      setError(String(e));
    }
    setSelectedIds([]);
    await refresh();
  };

  const open = async (file: ShelfFile) => {
    try {
      await ipc.openShelfItem(file.id);
    } catch (e) {
      setError(String(e));
    }
  };

  const togglePin = async (file: ShelfFile) => {
    try {
      await ipc.setShelfPinned(file.id, !file.pinned);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  };

  const copyFiles = async (file: ShelfFile) => {
    const targets =
      selectedIds.includes(file.id) && selectedIds.length > 1
        ? visibleFiles.filter((item) => selectedIds.includes(item.id))
        : [file];
    try {
      await ipc.copyShelfFiles(targets.map((item) => item.id));
    } catch (e) {
      setError(String(e));
    }
  };

  const saveAs = async (file: ShelfFile) => {
    try {
      const [exported] = await ipc.exportShelfItems([file.id]);
      if (!exported) return;
      const target = await saveDialog({ defaultPath: exported.name, title: "Save as" });
      if (!target) return;
      await ipc.saveFileAs(exported.path, target);
    } catch (e) {
      setError(String(e));
    }
  };

  const openMenu = (file: ShelfFile, event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (!selectedIds.includes(file.id)) {
      selectionAnchor.current = file.id;
      setSelectedIds([file.id]);
    }
    setMenu({ x: event.clientX, y: event.clientY, file });
  };

  const pinnedFiles = visibleFiles.filter((file) => file.pinned);
  const recentFiles = visibleFiles.filter((file) => !file.pinned);
  const detailFile = detailId ? (files.find((file) => file.id === detailId) ?? null) : null;

  const menuItems: ContextMenuItem[] = menu
    ? [
        ...(menu.file.kind === "url"
          ? [
              {
                label: menu.file.meta_fetched_at ? "Refresh link preview" : "Fetch link preview",
                onSelect: () => {
                  void refreshLink(menu.file);
                },
              },
            ]
          : []),
        {
          label: "Rename…",
          onSelect: () => {
            setRenameTarget({ id: menu.file.id, value: menu.file.custom_title || menu.file.name });
          },
        },
        ...(menu.file.parent_id
          ? [
              {
                label: "Remove from folder",
                onSelect: () => {
                  void moveOutOfFolder(menu.file);
                },
              },
            ]
          : []),
        {
          label: menu.file.pinned ? "Unpin" : "Pin to top",
          onSelect: () => {
            void togglePin(menu.file);
          },
        },
        { label: "Open", onSelect: () => { void open(menu.file); } },
        ...(menu.file.kind === "folder"
          ? []
          : [
              { label: "Copy", onSelect: () => { void copyFiles(menu.file); } },
              { label: "Save as…", onSelect: () => { void saveAs(menu.file); } },
            ]),
        { label: "Remove", danger: true, onSelect: () => { void remove(menu.file.id); } },
      ]
    : [];

  useSelectionKeys({
    ids: visibleFiles.map((file) => file.id),
    selected: selectedIds,
    setSelected: setSelectedIds,
    getAnchor: () => selectionAnchor.current,
    setAnchor: (id) => {
      selectionAnchor.current = id;
    },
    columns: gridColumns,
    itemAttribute: "data-shelf-id",
    onDelete: (ids) => {
      for (const id of ids) void remove(id);
    },
    onActivate: (id) => {
      const file = filesRef.current.find((item) => item.id === id);
      if (!file) return;
      if (file.kind === "folder") {
        setFolderId(file.id);
        setSelectedIds([]);
      } else {
        setDetailId(file.id);
      }
    },
    onEscape: () => {
      if (folderId) {
        setFolderId(null);
        setSelectedIds([]);
      }
    },
  });

  const renderTile = (file: ShelfFile) => {
    const isNewArrival = newlyArrivedIds.includes(file.id);    const isFolder = file.kind === "folder";
    const childCount = isFolder ? (childCounts.get(file.id) ?? 0) : 0;
    const isDropTarget = dragOverId === file.id;

    return (
      <div
        key={file.id}
        data-shelf-id={file.id}
        draggable={false}
        onDragStart={(event) => {
          // Chromium starts a native OLE drag for images and selected text.
          // That drag reaches our own drop target, shows the island's "drop
          // to add" overlay and re-ingests the item — so grouping a file onto
          // another file would drop it twice. Tile dragging is handled by the
          // pointer gesture below; the native export drag is started there.
          event.preventDefault();
        }}
        onPointerDown={(event) => {
          if (event.button !== 0) return;
          // Skip the drag pre-warm when pressing a control (pin/delete/expand).
          if ((event.target as HTMLElement).closest("button, input, select, textarea, a")) {
            return;
          }
          const ids = selectedRef.current.includes(file.id) ? selectedRef.current : [file.id];
          dragSession.current = {
            file,
            ids,
            startX: event.clientX,
            startY: event.clientY,
            moved: false,
            started: false,
          };
          prewarmDrag(file);
        }}
        onClick={(event) => {
          if (Date.now() < suppressClickUntil.current) {
            event.preventDefault();
            return;
          }
          // Folders open on click; files select.
          if (isFolder && !event.ctrlKey && !event.metaKey && !event.shiftKey) {
            setFolderId(file.id);
            setSelectedIds([]);
            return;
          }
          toggleSelection(file.id, event);
        }}
        onDoubleClick={() => {
          if (Date.now() < suppressClickUntil.current) return;
          void open(file);
        }}
        onContextMenu={(event) => openMenu(file, event)}
        title={
          isFolder
            ? `${file.name} — ${childCount} item${childCount === 1 ? "" : "s"} · click to open, drag out to copy`
            : `${file.name} — right-click for actions, double-click to open, drag onto another file to make a folder`
        }
        className={`group relative flex cursor-grab select-none flex-col overflow-hidden rounded-xl border bg-white/[0.04] transition active:cursor-grabbing ${
          selectedIds.includes(file.id)
            ? "border-cyan-300/55 ring-1 ring-cyan-300/45"
            : isFolder
              ? "border-amber-200/25 hover:border-amber-200/45 hover:bg-white/[0.07]"
              : file.pinned
                ? "border-amber-200/25 hover:border-amber-200/45 hover:bg-white/[0.07]"
                : "border-white/[0.07] hover:border-white/15 hover:bg-white/[0.07]"
        } ${isNewArrival ? "shelf-item-fresh" : ""} ${isDropTarget ? "border-cyan-300/70 ring-2 ring-cyan-300/60" : ""} ${gridColumns === 1 ? "h-[70px]" : "h-[96px]"}`}
      >
        {isDropTarget && (
          <div className="pointer-events-none absolute inset-0 z-20 grid place-items-center rounded-xl border border-cyan-300/60 bg-cyan-400/20 text-[9px] font-medium text-cyan-50">
            {isFolder ? "Move into folder" : "Create folder"}
          </div>
        )}

        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            void togglePin(file);
          }}
          aria-label={file.pinned ? `Unpin ${file.name}` : `Pin ${file.name}`}
          title={file.pinned ? "Unpin" : "Pin to top"}
          className={`absolute left-1 top-1 z-10 h-4 w-4 place-items-center rounded-full backdrop-blur transition ${
            file.pinned
              ? "grid bg-amber-300/85 text-black"
              : "hidden bg-black/60 text-white/80 group-hover:grid hover:text-amber-200"
          }`}
        >
          <PinIcon className="h-2.5 w-2.5" />
        </button>

        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            void remove(file.id);
          }}
          aria-label={`Remove ${file.name}`}
          className="absolute right-1 top-1 z-10 hidden rounded-full bg-black/60 p-1 text-[9px] leading-4 text-white/80 backdrop-blur group-hover:block hover:text-rose-300"
        >
          <ActionIcon name="delete" className="h-3 w-3" />
        </button>

        {file.parent_id && (
          <button
            type="button"
            onClick={(event) => {
              event.stopPropagation();
              void moveOutOfFolder(file);
            }}
            aria-label={`Remove ${file.name} from folder`}
            title="Remove from folder"
            className="absolute bottom-[26px] left-1 z-10 hidden h-4 w-4 place-items-center rounded bg-black/60 text-white/70 backdrop-blur transition group-hover:grid hover:text-cyan-200"
          >
            <ActionIcon name="close" className="h-2.5 w-2.5" />
          </button>
        )}

        <button
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            setDetailId(file.id);
          }}
          aria-label={`Expand ${file.name}`}
          title="Expand"
          className="absolute bottom-[26px] right-1 z-10 grid h-4 w-4 place-items-center rounded bg-black/60 text-[10px] leading-none text-white/70 opacity-0 backdrop-blur transition group-hover:opacity-100 hover:text-cyan-200"
        >
          ⤢
        </button>

        <div className="flex min-h-0 flex-1 items-center justify-center overflow-hidden">
          {isFolder ? (
            <div className="relative grid h-full w-full place-items-center">
              <span className="absolute left-[calc(50%-20px)] top-[18px] h-6 w-8 rounded-md border border-amber-200/25 bg-amber-200/10" />
              <span className="absolute left-[calc(50%-14px)] top-[22px] h-7 w-9 rounded-md border border-amber-200/25 bg-amber-200/15" />
              <span className="relative grid h-7 w-11 place-items-center rounded-md border border-amber-200/40 bg-amber-300/25 text-amber-100">
                <FolderIcon className="h-4 w-4" />
              </span>
            </div>
          ) : file.kind === "url" ? (
            file.meta_image && !brokenMedia.includes(`img:${file.id}`) ? (
              <img
                src={fileUrl(file.meta_image)}
                alt=""
                draggable={false}
                className="h-full w-full object-cover"
                onError={() => markBroken(`img:${file.id}`)}
              />
            ) : (
              <div className="flex w-full flex-col items-center gap-1 px-2 text-center">
                {file.meta_favicon && !brokenMedia.includes(`fav:${file.id}`) ? (
                  <img
                    src={fileUrl(file.meta_favicon)}
                    alt=""
                    draggable={false}
                    className="h-5 w-5 rounded"
                    onError={() => markBroken(`fav:${file.id}`)}
                  />
                ) : (
                  <LinkGlyph className="h-4 w-4 text-cyan-300" />
                )}
                <span className="line-clamp-2 break-words text-[8.5px] leading-tight text-white/55">
                  {file.meta_site || file.preview || file.content}
                </span>
              </div>
            )
          ) : file.kind === "text" || file.kind === "html" ? (
            <div className="h-full w-full overflow-hidden bg-black/25 px-1.5 py-1">
              <p className="line-clamp-3 break-words text-[8.5px] leading-snug text-white/60">
                {file.preview || file.content.slice(0, 140)}
              </p>
            </div>
          ) : file.thumb_path && !brokenMedia.includes(`thumb:${file.id}`) ? (
            <img
              src={fileUrl(file.thumb_path)}
              alt=""
              draggable={false}
              onError={() => markBroken(`thumb:${file.id}`)}
              className={
                file.thumb_kind === "cover"
                  ? "h-full w-full object-cover"
                  : "h-full w-full object-contain p-2 drop-shadow-[0_4px_10px_rgba(0,0,0,0.35)]"
              }
            />
          ) : (
            <div
              className="grid h-7 w-7 place-items-center rounded-md text-[8.5px] font-semibold uppercase tracking-wide"
              style={{
                background: `${glyphColor(file.extension)}22`,
                color: glyphColor(file.extension),
              }}
            >
              {(file.extension || "file").slice(0, 4)}
            </div>
          )}
        </div>

        <div className="shrink-0 bg-black/35 px-1.5 py-1">
          <div className="truncate text-[9.5px] leading-tight text-white/80">{file.name}</div>
          <div className="flex items-center justify-between gap-1 text-[8.5px] leading-tight text-white/35">
            <span className="min-w-0 truncate">
              {isFolder
                ? `${childCount} item${childCount === 1 ? "" : "s"}`
                : file.kind === "url"
                  ? file.meta_site || file.extension
                  : formatBytes(file.size_bytes)}
            </span>
            <span className="shrink-0 text-white/30">{relativeTime(file.created_at)}</span>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div
      className="widget-card relative flex h-full min-h-0 flex-col rounded-2xl p-3"
      onDragStart={(event) => event.preventDefault()}
    >
      <div className="mb-2 flex shrink-0 items-center justify-between gap-2">
        <div className="flex min-w-0 items-center gap-1">
          {folder && (
            <button
              type="button"
              onClick={() => {
                setFolderId(null);
                setSelectedIds([]);
              }}
              title="Back to all files"
              aria-label="Back to all files"
              className="grid h-5 w-5 shrink-0 place-items-center rounded-md text-[var(--text-muted)] transition hover:bg-white/10 hover:text-[var(--text-main)]"
            >
              <ActionIcon name="chevron" className="h-3 w-3 rotate-180" />
            </button>
          )}
          <span className="truncate text-[11px] text-[var(--text-muted)]">
            {historyOpen
              ? "Drop history"
              : folder
                ? `${folder.name} · ${visibleFiles.length}`
                : visibleFiles.length
                  ? `${visibleFiles.length} item${visibleFiles.length === 1 ? "" : "s"}${pinnedFiles.length ? ` · ${pinnedFiles.length} pinned` : ""}`
                  : "Dropzone shelf"}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          {!folder && !historyOpen && (
            <button
              type="button"
              onClick={() => void createEmptyFolder()}
              title="New folder"
              aria-label="New folder"
              className="rounded-md px-2 py-[2px] text-[10px] text-[var(--text-muted)] transition hover:bg-white/10 hover:text-[var(--text-main)]"
            >
              + Folder
            </button>
          )}
          <button
            type="button"
            onClick={() => setHistoryOpen((open) => !open)}
            title={historyOpen ? "Back to shelf" : "Everything dropped, and when"}
            className={`rounded-md px-2 py-[2px] text-[10px] transition hover:bg-white/10 ${historyOpen ? "bg-white/15 text-[var(--text-main)]" : "text-[var(--text-muted)]"}`}
          >
            {historyOpen ? "Shelf" : "History"}
          </button>
          <button
            type="button"
            onClick={clear}
            disabled={!recentFiles.length || historyOpen}
            title="Remove unpinned items"
            className="rounded-md px-2 py-[2px] text-[10px] text-[var(--text-muted)] transition hover:bg-white/10 hover:text-[var(--text-main)] disabled:opacity-30"
          >
            Clear
          </button>
        </div>
      </div>

      {error && <p className="mb-2 shrink-0 text-[11px] text-rose-300">{error}</p>}

      {historyOpen ? (
        <div className="min-h-0 flex-1 overflow-y-auto pr-0.5">
          {history.map((entry) => (
            <div
              key={entry.id}
              className="flex items-center gap-2 rounded-lg px-2 py-1.5 text-[10.5px] text-white/70 odd:bg-white/[0.03]"
              title={entry.path}
            >
              <span className="w-[54px] shrink-0 text-[9px] text-white/35">
                {relativeTime(entry.created_at)}
              </span>
              <span className="min-w-0 flex-1 truncate">{entry.name}</span>
              <span className="shrink-0 rounded bg-white/5 px-1 text-[8.5px] uppercase text-white/35">
                {entry.kind === "image" ? "IMG" : "FILE"}
              </span>
            </div>
          ))}
          {!history.length && (
            <div className="rounded-lg border border-dashed border-white/12 px-3 py-4 text-center text-[11px] text-white/35">
              Nothing dropped yet.
            </div>
          )}
        </div>
      ) : (
      <div className={`grid min-h-0 flex-1 content-start gap-2 overflow-y-auto pr-0.5 ${gridColumns === 1 ? "grid-cols-1" : gridColumns === 2 ? "grid-cols-2" : "grid-cols-3"}`}>
        {pinnedFiles.length > 0 && (
          <>
            <div className="col-span-full flex items-center gap-1 text-[8.5px] font-semibold uppercase tracking-[0.12em] text-amber-200/70">
              <PinIcon className="h-2.5 w-2.5" />
              Pinned
            </div>
            {pinnedFiles.map(renderTile)}
          </>
        )}

        {pinnedFiles.length > 0 && recentFiles.length > 0 && (
          <div className="col-span-full mt-0.5 text-[8.5px] font-semibold uppercase tracking-[0.12em] text-white/30">
            Recent
          </div>
        )}

        {recentFiles.map(renderTile)}

        {!visibleFiles.length && (
          <div className="col-span-full rounded-xl border border-dashed border-cyan-300/30 px-3 py-6 text-center text-[11px] text-white/35">
            {folder
              ? "This folder is empty. Drag a file onto it to add it."
              : "Drag files onto the island to stash them here. Drop one file onto another to make a folder."}
          </div>
        )}
      </div>
      )}

      <p className="mt-2 shrink-0 text-[10px] text-white/30">
        {folder
          ? "Drag out to copy · drop a file here to add it · Ctrl+A selects all"
          : "Drop a file on another to group · Ctrl+A selects all · right-click to rename"}
      </p>

      {renameTarget && (
        <div
          className="absolute inset-x-2 top-2 z-40 rounded-xl border border-white/12 bg-[#0d0d11]/97 p-2 shadow-[0_18px_40px_-18px_rgba(0,0,0,0.9)] backdrop-blur-xl"
          onClick={(event) => event.stopPropagation()}
        >
          <p className="mb-1 px-0.5 text-[9px] uppercase tracking-[0.1em] text-white/35">Rename</p>
          <div className="flex items-center gap-1">
            <input
              autoFocus
              value={renameTarget.value}
              onChange={(event) =>
                setRenameTarget((prev) => (prev ? { ...prev, value: event.target.value } : prev))
              }
              onKeyDown={(event) => {
                if (event.key === "Enter") void applyRename();
                if (event.key === "Escape") setRenameTarget(null);
              }}
              className="min-w-0 flex-1 rounded-md border border-white/10 bg-black/30 px-2 py-1 text-[11px] text-[var(--text-main)] outline-none focus:border-cyan-300/50"
            />
            <button
              type="button"
              onClick={() => void applyRename()}
              className="rounded-md bg-cyan-400/20 px-2 py-1 text-[10px] text-cyan-100 ring-1 ring-cyan-300/30 transition hover:bg-cyan-400/30"
            >
              Save
            </button>
            <button
              type="button"
              onClick={() => setRenameTarget(null)}
              aria-label="Cancel rename"
              className="rounded-md px-1.5 py-1 text-[10px] text-white/45 transition hover:bg-white/10 hover:text-white/80"
            >
              ✕
            </button>
          </div>
        </div>
      )}

      {detailFile && (
        <ShelfDetail
          file={detailFile}
          onClose={() => setDetailId(null)}
          onOpen={() => { void open(detailFile); }}
          onCopy={() => {
            void ipc.copyShelfFiles([detailFile.id]).catch((e) => setError(String(e)));
          }}
          onSaveAs={() => { void saveAs(detailFile); }}
          onTogglePin={() => { void togglePin(detailFile); }}
          onRemove={() => {
            setDetailId(null);
            void remove(detailFile.id);
          }}
          onRename={() =>
            setRenameTarget({ id: detailFile.id, value: detailFile.custom_title || detailFile.name })
          }
          onRefreshMeta={
            detailFile.kind === "url" ? () => { void refreshLink(detailFile); } : undefined
          }
          onRemoveFromFolder={
            detailFile.parent_id ? () => { void moveOutOfFolder(detailFile); } : undefined
          }
        />
      )}

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}

function PinIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M14.2 2 22 9.8l-1.8 1.8-1.2-.5-3.9 3.9.4 3.8-1.7 1.7-3.6-3.6-4.6 4.6-1.4-1.4 4.6-4.6L5.2 12l1.7-1.7 3.8.4L14.6 6.8l-.5-1.2L15.9 3.8 14.2 2Z" />
    </svg>
  );
}

function FolderIcon({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" fill="currentColor" className={className} aria-hidden="true">
      <path d="M3 6.5A2.5 2.5 0 0 1 5.5 4h3.2c.7 0 1.3.3 1.8.8l1 1.2h7A2.5 2.5 0 0 1 21 8.5v8A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5v-10Z" />
    </svg>
  );
}

function LinkGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      className={className}
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      aria-hidden="true"
    >
      <path d="M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1" />
      <path d="M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1" />
    </svg>
  );
}
