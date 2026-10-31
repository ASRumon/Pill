import { useEffect, useRef, useState } from "react";
import type { DragEvent, MouseEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { dragFilesOut, fileUrl, ipc, prewarmDragIcon } from "../lib/ipc";
import { ContextMenu } from "../components/ContextMenu";
import type { ContextMenuItem } from "../components/ContextMenu";
import { setClipboardDragActive } from "../lib/clipboardDragState";
import { requestIslandCollapse } from "../lib/islandControl";
import { useSelectionKeys } from "../lib/selectionKeys";
import type { AppSettings, ClipboardEntry } from "../types";

const TEXT_FORMATS = [
  "txt",
  "md",
  "html",
  "json",
  "csv",
  "xml",
  "yml",
  "log",
  "ini",
  "css",
  "js",
  "ts",
  "py",
  "sql",
  "bat",
  "ps1",
  "sh",
];
const IMAGE_FORMATS = ["png", "jpg", "webp"];

async function renderTextImage(content: string, format: string): Promise<string> {
  if (content.length > 30_000) {
    throw new Error("This clip is too long to convert into one image.");
  }
  const canvas = document.createElement("canvas");
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Could not create a text-to-image renderer.");

  const padding = 48;
  const maxTextWidth = 1120;
  const lineHeight = 36;
  context.font = '24px "Segoe UI", Arial, sans-serif';

  const lines: string[] = [];
  for (const paragraph of content.replace(/\r/g, "").split("\n")) {
    if (!paragraph) {
      lines.push("");
      continue;
    }
    let line = "";
    for (const word of paragraph.split(/\s+/)) {
      const candidate = line ? `${line} ${word}` : word;
      if (context.measureText(candidate).width <= maxTextWidth) {
        line = candidate;
        continue;
      }
      if (line) lines.push(line);
      line = "";
      for (const character of word) {
        if (context.measureText(line + character).width > maxTextWidth && line) {
          lines.push(line);
          line = "";
        }
        line += character;
      }
    }
    lines.push(line);
  }

  if (lines.length > 420) {
    throw new Error("This clip is too long to convert into one image.");
  }

  canvas.width = maxTextWidth + padding * 2;
  canvas.height = Math.max(120, lines.length * lineHeight + padding * 2);
  context.fillStyle = "#ffffff";
  context.fillRect(0, 0, canvas.width, canvas.height);
  context.font = '24px "Segoe UI", Arial, sans-serif';
  context.textBaseline = "top";
  context.fillStyle = "#171717";
  lines.forEach((line, index) => context.fillText(line, padding, padding + index * lineHeight));

  const mime = format === "jpg" ? "image/jpeg" : `image/${format}`;
  return canvas.toDataURL(mime, 0.92);
}

export function ClipboardWidget({ gridColumns = 2 }: { gridColumns?: 1 | 2 | 3 }) {
  const [entries, setEntries] = useState<ClipboardEntry[]>([]);
  const [selected, setSelected] = useState<number[]>([]);
  const [format, setFormat] = useState("txt");
  const [multiSelect, setMultiSelect] = useState(false);
  const [draggingFormat, setDraggingFormat] = useState<string | null>(null);
  const [settings, setSettings] = useState<AppSettings>({
    default_text_format: "txt",
    default_image_format: "png",
    minified_island: false,
    shelf_retention_hours: 24,
    click_through_shortcut: "ctrl+alt",
    search_engine: "google",
    search_engines: [],
    search_browser: "default",
    search_everything_port: 80,
    search_es_path: "",
    island_background: "amoled",
    island_opacity: 92,
    island_blur: 24,
  });
  const [watching, setWatching] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [failedImages, setFailedImages] = useState<number[]>([]);
  const [menu, setMenu] = useState<{ x: number; y: number; entry: ClipboardEntry } | null>(null);
  const selectionAnchor = useRef<number | null>(null);
  const prewarm = useRef<{ key: string; promise: Promise<{ path: string }[]> } | null>(null);
  // A cancelled HTML5 drag still emits a click; two attempts then look like a
  // double-click and would pop the (decorated) editor window mid-drag.
  const suppressClickUntil = useRef(0);

  const refresh = async () => {
    try {
      setEntries(await ipc.listClips());
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const copyClip = async (entry: ClipboardEntry) => {
    try {
      await ipc.copyClip(entry.id);
    } catch (e) {
      setError(String(e));
    }
  };

  const saveClipAs = async (entry: ClipboardEntry) => {
    try {
      const format =
        entry.kind === "image" ? settings.default_image_format : settings.default_text_format;
      const [file] = await ipc.exportClips([entry.id], format);
      if (!file) return;
      const target = await saveDialog({
        defaultPath: `clip-${entry.id}.${file.extension}`,
        title: "Save clip as",
      });
      if (!target) return;
      await ipc.saveFileAs(file.path, target);
    } catch (e) {
      setError(String(e));
    }
  };

  const openMenu = (entry: ClipboardEntry, event: MouseEvent<HTMLDivElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (!selected.includes(entry.id)) {
      selectionAnchor.current = entry.id;
      setSelected([entry.id]);
    }
    setMenu({ x: event.clientX, y: event.clientY, entry });
  };

  const menuItems: ContextMenuItem[] = menu
    ? [
        { label: "Copy", onSelect: () => { void copyClip(menu.entry); } },
        { label: "Save as…", onSelect: () => { void saveClipAs(menu.entry); } },
        { label: "Edit", onSelect: () => { void ipc.openClipEditor(menu.entry.id); } },
        { label: "Delete", danger: true, onSelect: () => { void removeEntry(menu.entry.id); } },
      ]
    : [];

  useEffect(() => {
    void refresh();

    ipc
      .getSettings()
      .then((loaded) => {
        setSettings(loaded);
        setFormat(loaded.default_text_format);
      })
      .catch(() => undefined);

    ipc
      .clipboardWatchEnabled()
      .then(setWatching)
      .catch(() => undefined);

    let disposed = false;
    const disposers: Array<() => void> = [];

    const register = <T,>(eventName: string, handler: (payload: T) => void) => {
      listen<T>(eventName, (event) => handler(event.payload))
        .then((unlisten) => {
          if (disposed) unlisten();
          else disposers.push(unlisten);
        })
        .catch(() => {
          // Events are unavailable outside the Tauri runtime (browser preview).
        });
    };

    register<ClipboardEntry>("clipboard-changed", () => void refresh());
    register<boolean>("clipboard-watch-changed", setWatching);
    register<AppSettings>("settings-changed", (payload) => {
      setSettings(payload);
      setFormat(payload.default_text_format);
    });

    return () => {
      disposed = true;
      disposers.forEach((unlisten) => unlisten());
    };
  }, []);

  const toggleWatch = async () => {
    try {
      setWatching(await ipc.setClipboardWatch(!watching));
    } catch (e) {
      setError(String(e));
    }
  };

  /** Single-select replaces the selection; multi-select adds to it. */
  const toggleSelect = (id: number, event: MouseEvent<HTMLDivElement>) => {
    const anchor = selectionAnchor.current;
    if (event.shiftKey) {
      const anchorIndex = entries.findIndex((entry) => entry.id === (anchor ?? id));
      const targetIndex = entries.findIndex((entry) => entry.id === id);
      if (anchorIndex >= 0 && targetIndex >= 0) {
        const range = entries
          .slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1)
          .map((entry) => entry.id);
        setSelected((prev) => [...new Set([...prev, ...range])]);
        return;
      }
    }

    if (multiSelect || event.ctrlKey || event.metaKey) {
      selectionAnchor.current = id;
      setSelected((prev) =>
        prev.includes(id) ? prev.filter((value) => value !== id) : [...prev, id],
      );
      return;
    }

    selectionAnchor.current = id;
    setSelected((prev) => {
      return prev.length === 1 && prev[0] === id ? [] : [id];
    });
  };

  const toggleMultiSelect = () => {
    const next = !multiSelect;
    setMultiSelect(next);
    // Leaving multi-select keeps only the most recent pick.
    if (!next) setSelected((prev) => (prev.length > 1 ? [prev[prev.length - 1]] : prev));
  };

  const defaultFormatFor = (kind: string) =>
    kind === "image" ? settings.default_image_format : settings.default_text_format;

  /** Applies the selected output format to every clip, including cross-type conversion. */
  const exportSelection = async (ids: number[], outputFormat: string) => {
    const selectedEntries = ids.map((id) => entries.find((entry) => entry.id === id));
    if (selectedEntries.some((entry) => !entry)) {
      throw new Error("A selected clip is no longer available. Refresh and try again.");
    }

    const files = [];
    for (const entry of selectedEntries) {
      if (!entry) continue;
      if (entry.kind === "text" && IMAGE_FORMATS.includes(outputFormat)) {
        const dataUrl = await renderTextImage(entry.content, outputFormat);
        files.push(await ipc.saveConvertedTextImage(dataUrl, outputFormat));
      } else {
        const [file] = await ipc.exportClips([entry.id], outputFormat);
        if (file) files.push(file);
      }
    }
    return files;
  };

  /** A mixed selection keeps each clip's configured default output type. */
  const exportSelectionDefaults = async (ids: number[]) => {
    const batches = await Promise.all(
      ids.map(async (id) => {
        const entry = entries.find((item) => item.id === id);
        if (!entry) {
          throw new Error("A selected clip is no longer available. Refresh and try again.");
        }
        return exportSelection([id], defaultFormatFor(entry.kind));
      }),
    );
    return batches.flat();
  };

  const dragKey = (entry: ClipboardEntry) =>
    selected.length ? `selection:${selected.join(",")}` : `clip:${entry.id}`;

  /** The native drag can only start once the files exist, so build them early. */
  const prewarmDrag = (entry: ClipboardEntry) => {
    void prewarmDragIcon();
    const key = dragKey(entry);
    if (prewarm.current?.key === key) return;
    const promise = (
      selected.length
        ? exportSelectionDefaults(selected)
        : exportSelection([entry.id], defaultFormatFor(entry.kind))
    ).catch(() => [] as { path: string }[]);
    prewarm.current = { key, promise };
  };

  /** Drags the selection (or just this clip) out as real files. */
  const startDrag = async (entry: ClipboardEntry, event: DragEvent) => {
    event.preventDefault();
    suppressClickUntil.current = Date.now() + 750;

    const hasSelection = selected.length > 0;
    const ids = hasSelection ? selected : [entry.id];
    const key = dragKey(entry);
    const warmed = prewarm.current;
    prewarm.current = null;

    setClipboardDragActive(true);
    try {
      const files =
        warmed && warmed.key === key
          ? await warmed.promise
          : hasSelection
            ? await exportSelectionDefaults(ids)
            : await exportSelection(ids, defaultFormatFor(entry.kind));
      if (!files.length) return;
      await dragFilesOut(files.map((file) => file.path));
    } catch (e) {
      setError(String(e));
    } finally {
      setClipboardDragActive(false);
      requestIslandCollapse();
    }
  };

  /**
   * Dragging a format chip converts the whole selection into that format and
   * starts the native OS drag, so it can be dropped into Explorer or any app
   * that accepts files.
   */
  const startFormatDrag = async (exportFormat: string, event: DragEvent) => {
    // The native drag replaces the webview drag entirely.
    event.preventDefault();
    setFormat(exportFormat);

    if (!selected.length) {
      setError("Select one or more clips first, then drag a format out.");
      return;
    }

    setError(null);
    setDraggingFormat(exportFormat);

    setClipboardDragActive(true);
    try {
      const files = await exportSelection(selected, exportFormat);
      await dragFilesOut(files.map((file) => file.path));
    } catch (e) {
      setError(String(e));
    } finally {
      setClipboardDragActive(false);
      setDraggingFormat(null);
      requestIslandCollapse();
    }
  };

  const removeEntry = async (id: number) => {
    // Optimistic: the row leaves immediately, the backend catches up.
    setEntries((prev) => prev.filter((entry) => entry.id !== id));
    setSelected((prev) => prev.filter((value) => value !== id));
    setFailedImages((prev) => prev.filter((value) => value !== id));
    try {
      await ipc.deleteClip(id);
    } catch (e) {
      setError(String(e));
      await refresh();
    }
  };

  const clearAll = async () => {
    setEntries([]);
    setSelected([]);
    try {
      await ipc.clearClips();
    } catch (e) {
      setError(String(e));
      await refresh();
    }
  };

  useSelectionKeys({
    ids: entries.map((entry) => String(entry.id)),
    selected: selected.map(String),
    setSelected: (next) => setSelected(next.map(Number)),
    getAnchor: () => (selectionAnchor.current === null ? null : String(selectionAnchor.current)),
    setAnchor: (id) => {
      selectionAnchor.current = id === null ? null : Number(id);
    },
    columns: gridColumns,
    onDelete: (ids) => {
      for (const id of ids) void removeEntry(Number(id));
    },
    onEscape: () => setSelected([]),
  });

  /** Draggable output-format chip: click selects it, drag exports as files. */
  const renderFormatChip = (option: string, tone: "cyan" | "fuchsia") => {
    const active = format === option;
    const dragging = draggingFormat === option;
    const empty = selected.length === 0;
    const glow = tone === "cyan" ? "rgba(56,225,255,0.55)" : "rgba(232,121,249,0.55)";

    const toneClass =
      tone === "cyan"
        ? "bg-cyan-400/25 text-cyan-50"
        : "bg-fuchsia-400/25 text-fuchsia-50";

    return (
      <span
        key={option}
        role="button"
        tabIndex={0}
        draggable
        onDragStart={(event) => void startFormatDrag(option, event)}
        onDragEnd={() => setDraggingFormat(null)}
        onClick={() => setFormat(option)}
        onKeyDown={(event) => {
          if (event.key === "Enter" || event.key === " ") {
            event.preventDefault();
            setFormat(option);
          }
        }}
        title={
          empty
            ? "Select a clip first, then drag this format out"
            : `Drag to convert and export ${selected.length} clip${selected.length === 1 ? "" : "s"} as .${option}`
        }
        className={`shrink-0 select-none rounded-full px-2 py-[2px] text-[10px] font-medium tracking-wide transition outline-none focus-visible:ring-2 focus-visible:ring-white/50 ${
          dragging
            ? "scale-105 cursor-grabbing"
            : empty
              ? "cursor-not-allowed"
              : "cursor-grab active:cursor-grabbing"
        } ${
          active || dragging
            ? toneClass
            : "bg-white/[0.06] text-white/45 hover:bg-white/10 hover:text-white/75"
        } ${empty && !dragging ? "opacity-40" : ""}`}
        style={
          dragging
            ? { boxShadow: `0 0 0 1px ${glow}, 0 0 14px ${glow}` }
            : active
              ? { boxShadow: "inset 0 0 0 1px rgba(255,255,255,0.18)" }
              : undefined
        }
      >
        {option.toUpperCase()}
      </span>
    );
  };

  return (
    <div className="widget-card flex h-full min-h-0 flex-col rounded-2xl p-3">
      <div className="mb-2 flex shrink-0 items-center justify-between">
        <button
          type="button"
          onClick={toggleWatch}
          title={watching ? "Pause clipboard capture" : "Resume clipboard capture"}
          className="flex items-center gap-1.5 rounded-full px-2 py-[3px] text-[10px] transition hover:brightness-110"
          style={{
            background: watching ? "rgba(56,225,255,0.14)" : "rgba(255,255,255,0.06)",
            color: watching ? "#7dd3fc" : "rgba(255,255,255,0.45)",
          }}
        >
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{
              background: watching ? "#38e1ff" : "#6b7280",
              boxShadow: watching ? "0 0 8px #38e1ff" : "none",
            }}
          />
          {watching ? "Capturing" : "Paused"}
        </button>

        <div className="flex items-center gap-2 text-[10px] text-white/35">
          <span>{selected.length ? `${selected.length} selected` : `${entries.length} clips`}</span>
          <button
            type="button"
            onClick={toggleMultiSelect}
            title={multiSelect ? "Multiple selection on" : "Enable multiple selection"}
            className={`rounded px-1.5 py-[2px] transition ${
              multiSelect
                ? "bg-cyan-400/20 text-cyan-200 ring-1 ring-cyan-400/40"
                : "hover:bg-white/10 hover:text-white/70"
            }`}
          >
            Multi
          </button>
          <button
            type="button"
            onClick={() => {
              setMultiSelect(true);
              setSelected(entries.map((entry) => entry.id));
            }}
            disabled={!entries.length}
            className="rounded px-1.5 py-[2px] hover:bg-white/10 hover:text-white/70 disabled:opacity-30"
          >
            All
          </button>
          <button
            type="button"
            onClick={clearAll}
            disabled={!entries.length}
            className="rounded px-1.5 py-[2px] hover:bg-white/10 hover:text-rose-300 disabled:opacity-30"
          >
            Clear
          </button>
        </div>
      </div>

      <div className="mb-1.5 flex items-center gap-2">
        <span className="shrink-0 text-[9px] uppercase tracking-wider text-white/25">
          {selected.length ? `Drag ${selected.length}` : "Format"}
        </span>
        <div className="no-scrollbar flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
          {TEXT_FORMATS.map((option) => renderFormatChip(option, "cyan"))}
          <span className="mx-0.5 h-3.5 w-px shrink-0 bg-white/15" />
          {IMAGE_FORMATS.map((option) => renderFormatChip(option, "fuchsia"))}
        </div>
      </div>

      {error && <p className="mt-2 text-[11px] text-rose-300">{error}</p>}

      <div className={`mt-2 grid min-h-0 flex-1 content-start gap-1.5 overflow-y-auto pr-0.5 ${gridColumns === 1 ? "grid-cols-1" : gridColumns === 2 ? "grid-cols-2" : "grid-cols-3"}`}>
        {entries.map((entry) => {
          const isSelected = selected.includes(entry.id);
          return (
            <div
              key={entry.id}
              data-select-id={entry.id}
              draggable
              onDragStart={(event) => void startDrag(entry, event)}
              onPointerDown={(event) => {
                // Never pre-warm a drag when the press is on a control
                // (delete, checkbox, …): an export can take a while.
                if ((event.target as HTMLElement).closest("button, input, select, textarea, a")) {
                  return;
                }
                prewarmDrag(entry);
              }}
              onClick={(event) => {
                if (Date.now() < suppressClickUntil.current) {
                  event.preventDefault();
                  return;
                }
                toggleSelect(entry.id, event);
              }}
              onDoubleClick={() => {
                if (Date.now() < suppressClickUntil.current) return;
                void ipc.openClipEditor(entry.id);
              }}
              onContextMenu={(event) => openMenu(entry, event)}
              title="Click to select · right-click for copy/save · double-click to edit"              className={`group relative flex cursor-pointer ${gridColumns === 1 ? "flex-row items-center gap-2 px-2 py-1.5" : "min-h-[78px] flex-col items-start gap-1.5 p-2"} rounded-lg border transition ${
                isSelected
                  ? "border-cyan-400/40 bg-cyan-400/10"
                  : "border-white/[0.05] bg-white/[0.03] hover:bg-white/[0.06]"
              }`}
            >
              <span className="grid h-4 w-4 shrink-0 place-items-center">
                <input
                  type="checkbox"
                  checked={isSelected}
                  onClick={(event) => event.stopPropagation()}
                  onChange={() => {
                    selectionAnchor.current = entry.id;
                    setMultiSelect(true);
                    setSelected((prev) =>
                      prev.includes(entry.id)
                        ? prev.filter((value) => value !== entry.id)
                        : [...prev, entry.id],
                    );
                  }}
                  aria-label={`Select ${entry.kind} clip`}
                  className="h-3.5 w-3.5 cursor-pointer accent-cyan-300"
                />
              </span>

              <button
                type="button"
                onClick={(event) => event.stopPropagation()}
                title={selected.length > 1 ? `Drag all ${selected.length} selected clips` : "Drag this clip or the current selection"}
                aria-label={selected.length > 1 ? `Drag ${selected.length} selected clips` : `Drag ${entry.kind} clip`}
                className="absolute right-1 top-1 z-10 grid h-4 w-4 cursor-grab place-items-center rounded text-[10px] text-white/35 hover:bg-white/10 hover:text-white active:cursor-grabbing"
              >
                ⠿
              </button>

              {entry.kind === "image" && entry.image_path && !failedImages.includes(entry.id) ? (
                <img
                  src={fileUrl(entry.image_path)}
                  alt=""
                  className={`${gridColumns === 1 ? "h-6 w-6" : "h-12 w-full"} shrink-0 rounded object-cover ring-1 ring-white/10`}
                  onError={() => setFailedImages((prev) => [...prev, entry.id])}
                />
              ) : entry.kind === "image" ? (
                <span className={`grid ${gridColumns === 1 ? "h-6 w-6" : "h-12 w-full"} shrink-0 place-items-center rounded bg-fuchsia-400/10 text-[8px] text-fuchsia-200`} title="Image preview unavailable">IMG</span>
              ) : (
                <span className="shrink-0 rounded bg-cyan-400/10 px-1 py-[1px] font-mono text-[9px] text-cyan-300/80">
                  TXT
                </span>
              )}

              <span className={`min-w-0 ${gridColumns === 1 ? "flex-1 truncate" : "w-full line-clamp-2 break-words"} text-[11.5px] text-white/80`}>
                {entry.preview || "(empty clip)"}
              </span>

              <button
                type="button"
                onClick={(event) => {
                  event.stopPropagation();
                  void removeEntry(entry.id);
                }}
                aria-label="Delete clip"
                className="absolute bottom-1 right-1 hidden shrink-0 rounded bg-black/50 px-1 text-[10px] text-white/55 hover:bg-white/10 hover:text-rose-300 group-hover:block"
              >
                ✕
              </button>
            </div>
          );
        })}

        {!entries.length && (
          <div className="rounded-lg border border-dashed border-white/12 px-3 py-4 text-center text-[11px] text-white/35">
            Copy text or an image to start capturing.
          </div>
        )}
      </div>

      <p className="mt-1.5 truncate text-[9.5px] leading-relaxed text-white/30">
        {selected.length
          ? `${selected.length} selected · drag a format chip to convert them all`
          : "Click to select · Ctrl/Shift-click for more · Ctrl+A selects all"}
      </p>

      {menu && (
        <ContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} />
      )}
    </div>
  );
}
