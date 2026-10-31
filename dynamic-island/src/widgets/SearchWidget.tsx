import { useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent } from "react";
import { listen } from "@tauri-apps/api/event";
import { dragFilesOut, formatBytes, ipc, prewarmDragIcon } from "../lib/ipc";
import { requestIslandCollapse } from "../lib/islandControl";
import { BUILTIN_ENGINES, allEngines, engineUrl } from "../lib/searchEngines";
import { ContextMenu } from "../components/ContextMenu";
import { ActionIcon } from "../components/WidgetIcon";
import type { AppSettings, SearchEngine, SearchResult, SearchStatus } from "../types";

const GREETING = "Everything is not reachable from the app.";

/** The island unmounts inactive widgets, so keep the last search around. */
const cache: {
  query: string;
  results: SearchResult[];
  status: SearchStatus | null;
} = { query: "", results: [], status: null };

function parentFolder(path: string): string {
  const cut = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  return cut > 0 ? path.slice(0, cut) : "";
}

function extensionBadge(extension: string): string {
  if (!extension) return "FILE";
  return extension.slice(0, 4).toUpperCase();
}

export function SearchWidget() {
  const [query, setQuery] = useState(cache.query);
  const [results, setResults] = useState<SearchResult[]>(cache.results);
  const [status, setStatus] = useState<SearchStatus | null>(cache.status);
  const [engineId, setEngineId] = useState("google");
  const [customEngines, setCustomEngines] = useState<SearchEngine[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [enginePicker, setEnginePicker] = useState<{ x: number; y: number } | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const webButtonRef = useRef<HTMLButtonElement | null>(null);
  const longPressTimer = useRef<number | undefined>(undefined);
  const longPressed = useRef(false);
  const settingsRef = useRef<AppSettings | null>(null);
  // A cancelled HTML5 drag still emits a click; that click must not open the
  // file the user was trying to drop somewhere else.
  const suppressClickUntil = useRef(0);

  const engines = useMemo(() => allEngines(customEngines), [customEngines]);
  const activeEngine =
    engines.find((engine) => engine.id === engineId) ?? BUILTIN_ENGINES[0];
  const everythingReady = status?.everything ?? false;

  useEffect(() => {
    let disposed = false;

    ipc
      .getSettings()
      .then((loaded) => {
        if (disposed) return;
        settingsRef.current = loaded;
        setEngineId(loaded.search_engine || "google");
        setCustomEngines(loaded.search_engines ?? []);
      })
      .catch((e) => !disposed && setError(String(e)));

    ipc
      .searchStatus()
      .then((next) => {
        cache.status = next;
        if (!disposed) setStatus(next);
      })
      .catch(() => !disposed && setStatus(null));

    listen<AppSettings>("settings-changed", (event) => {
      settingsRef.current = event.payload;
      setEngineId(event.payload.search_engine || "google");
      setCustomEngines(event.payload.search_engines ?? []);
    })
      .then((unlisten) => {
        if (disposed) unlisten();
      })
      .catch(() => undefined);

    listen<string>("search-engine-changed", (event) => {
      setEngineId(event.payload);
    })
      .then((unlisten) => {
        if (disposed) unlisten();
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
    };
  }, []);

  useEffect(() => {
    cache.query = query;
  }, [query]);

  useEffect(() => {
    cache.results = results;
  }, [results]);

  // The Search tab is meant to be typed into immediately. The island remounts
  // widgets on every expand, so grabbing focus here also covers re-opening the
  // panel while Search is the active tab.
  useEffect(() => {
    const input = inputRef.current;
    if (!input) return;
    input.focus();
    const end = input.value.length;
    input.setSelectionRange(end, end);
  }, []);

  // Live file lookup. Everything answers in a few milliseconds, so a short
  // debounce is enough to stay ahead of typing.
  useEffect(() => {
    const term = query.trim();
    if (!everythingReady || term.length < 2) {
      setResults([]);
      setLoading(false);
      return;
    }

    let cancelled = false;
    setLoading(true);
    const timer = window.setTimeout(async () => {
      try {
        const found = await ipc.everythingSearch(term);
        if (cancelled) return;
        setResults(found);
        setError(null);
      } catch (e) {
        if (cancelled) return;
        setResults([]);
        setError(String(e));
      } finally {
        if (!cancelled) setLoading(false);
      }
    }, 180);

    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [query, everythingReady]);

  const persistEngine = (id: string) => {
    setEngineId(id);
    void ipc.setSearchEngine(id).catch(() => undefined);
  };

  // The search button carries the engine: wheel cycles, hold/right-click picks.
  // Refs keep the native wheel listener stable across renders.
  const enginesRef = useRef(engines);
  enginesRef.current = engines;
  const activeIdRef = useRef(activeEngine.id);
  activeIdRef.current = activeEngine.id;
  const persistEngineRef = useRef(persistEngine);
  persistEngineRef.current = persistEngine;

  const cycleEngine = (direction: 1 | -1) => {
    const list = enginesRef.current;
    if (list.length < 2) return;
    const index = list.findIndex((engine) => engine.id === activeIdRef.current);
    const next = list[(index + direction + list.length) % list.length];
    if (next) persistEngineRef.current(next.id);
  };

  useEffect(() => {
    const node = webButtonRef.current;
    if (!node) return;
    // React's synthetic wheel listener is passive, so preventDefault there
    // would be ignored and the panel would scroll instead of switching.
    const onWheel = (event: WheelEvent) => {
      event.preventDefault();
      cycleEngine(event.deltaY >= 0 ? 1 : -1);
    };
    node.addEventListener("wheel", onWheel, { passive: false });
    return () => node.removeEventListener("wheel", onWheel);
  }, []);

  useEffect(() => () => window.clearTimeout(longPressTimer.current), []);

  const openEnginePicker = () => {
    const rect = webButtonRef.current?.getBoundingClientRect();
    setEnginePicker({ x: rect ? rect.left : 12, y: rect ? rect.bottom + 6 : 12 });
  };

  const beginLongPress = () => {
    longPressed.current = false;
    window.clearTimeout(longPressTimer.current);
    longPressTimer.current = window.setTimeout(() => {
      longPressed.current = true;
      openEnginePicker();
    }, 420);
  };

  const cancelLongPress = () => window.clearTimeout(longPressTimer.current);

  const openWeb = async () => {
    const term = query.trim();
    if (!term) {
      inputRef.current?.focus();
      return;
    }
    try {
      await ipc.openExternal(engineUrl(activeEngine, term), settingsRef.current?.search_browser);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const startDrag = async (row: SearchResult, event: DragEvent<HTMLDivElement>) => {
    // Hand the drag to the OS so it can land in Explorer, Slack, VS Code, ...
    event.preventDefault();
    suppressClickUntil.current = Date.now() + 750;
    try {
      await dragFilesOut([row.path]);
    } catch (e) {
      setError(String(e));
    } finally {
      requestIslandCollapse();
    }
  };

  const openResult = async (row: SearchResult) => {
    if (Date.now() < suppressClickUntil.current) return;
    try {
      await ipc.openSearchResult(row.path);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="widget-card flex h-full min-h-0 flex-col rounded-2xl p-3">
      <div className="flex shrink-0 items-center gap-2">
        <div className="relative flex min-w-0 flex-1 items-center">
          <SearchGlyph className="pointer-events-none absolute left-2.5 h-3.5 w-3.5 text-white/35" />
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void openWeb();
              }
              if (event.key === "Escape") setQuery("");
            }}
            placeholder={everythingReady ? "Search files and the web" : "Search the web"}
            className="w-full rounded-xl border border-white/10 bg-black/25 py-2 pl-8 pr-2 text-[12px] text-[var(--text-main)] outline-none transition placeholder:text-white/35 focus:border-cyan-300/50 focus:ring-2 focus:ring-cyan-300/10"
          />
        </div>
        <button
          ref={webButtonRef}
          type="button"
          onPointerDown={(event) => {
            if (event.button === 0) beginLongPress();
          }}
          onPointerUp={cancelLongPress}
          onPointerLeave={cancelLongPress}
          onPointerCancel={cancelLongPress}
          onClick={() => {
            // A long press already opened the engine picker.
            if (longPressed.current) {
              longPressed.current = false;
              return;
            }
            void openWeb();
          }}
          onContextMenu={(event) => {
            event.preventDefault();
            openEnginePicker();
          }}
          aria-label={`Search with ${activeEngine.name}`}
          title={`Search with ${activeEngine.name}\nScroll to switch engine · hold or right-click to pick`}
          className="flex shrink-0 items-center gap-1 rounded-xl bg-cyan-300/25 px-2.5 py-2 text-[11px] font-medium text-cyan-50 transition hover:bg-cyan-300/35 active:scale-[0.97]"
        >
          <ActionIcon name="spark" className="h-3.5 w-3.5" />
          <span className="max-w-[76px] truncate">{activeEngine.name}</span>
        </button>
      </div>

      <div className="mt-1.5 flex shrink-0 items-center gap-1.5 text-[9px]">
        <span
          className={`shrink-0 rounded-full px-2 py-0.5 font-medium uppercase tracking-[0.08em] ${
            everythingReady
              ? "bg-emerald-400/15 text-emerald-200/90"
              : "bg-white/6 text-white/35"
          }`}
          title={status?.everything_detail || "Everything was not detected"}
        >
          {everythingReady ? "Everything" : "Web only"}
        </span>
        <span className="min-w-0 truncate text-white/25">
          Scroll or hold the button to switch engine
        </span>
      </div>

      {error && <p className="mt-2 text-[11px] text-rose-300">{error}</p>}

      <div className="mt-2 min-h-[74px] flex-1 overflow-y-auto">
        {results.map((row) => (
          <div
            key={row.path}
            role="button"
            tabIndex={0}
            draggable
            onDragStart={(event) => {
              void startDrag(row, event);
            }}
            onPointerDown={() => {
              void prewarmDragIcon();
            }}
            onClick={() => void openResult(row)}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                void openResult(row);
              }
            }}
            title={`${row.path}\nClick to open · drag to move into any app`}
            className="group mb-1 flex cursor-grab items-center gap-2 rounded-xl border border-white/[0.06] bg-white/[0.04] px-2 py-1.5 text-left transition hover:border-white/15 hover:bg-white/[0.08] active:cursor-grabbing"
          >
            <span className="grid h-6 w-6 shrink-0 place-items-center rounded-md bg-black/30 text-[8px] font-semibold uppercase text-white/55">
              {extensionBadge(row.extension)}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[11.5px] text-[var(--text-main)]">{row.name}</span>
              <span className="block truncate text-[9.5px] text-[var(--text-muted)]">
                {parentFolder(row.path)}
              </span>
            </span>
            <span className="shrink-0 text-[9px] tabular-nums text-white/30">
              {row.size > 0 ? formatBytes(row.size) : ""}
            </span>
            <span
              className="shrink-0 text-white/25 opacity-0 transition group-hover:opacity-100"
              aria-hidden="true"
            >
              <GripGlyph className="h-3.5 w-3.5" />
            </span>
          </div>
        ))}

        {!results.length && (
          <div className="rounded-xl border border-white/8 px-3 py-3 text-[10.5px] leading-relaxed text-[var(--text-muted)]">
            {!everythingReady ? (
              <>
                <span className="text-white/60">{GREETING}</span> Enable the HTTP server in
                Everything (Tools → Options → HTTP Server) or pick{" "}
                <span className="text-white/60">es.exe</span> in Settings. Web search still
                works.
              </>
            ) : query.trim().length < 2 ? (
              <>Type at least two characters. Enter opens {activeEngine.name}; drag any result into another app.</>
            ) : loading ? (
              <>Searching Everything…</>
            ) : (
              <>No files matched “{query.trim()}”.</>
            )}
          </div>
        )}
      </div>

      {enginePicker && (
        <ContextMenu
          x={enginePicker.x}
          y={enginePicker.y}
          items={engines.map((engine) => ({
            label: `${engine.id === activeEngine.id ? "✓ " : ""}${engine.name}`,
            onSelect: () => persistEngine(engine.id),
          }))}
          onClose={() => setEnginePicker(null)}
        />
      )}
    </div>
  );
}

function SearchGlyph({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      className={className}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.9}
      strokeLinecap="round"
    >
      <circle cx="11" cy="11" r="6.4" />
      <path d="m15.8 15.8 4 4" />
    </svg>
  );
}

function GripGlyph({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 24 24" className={className} fill="currentColor">
      <circle cx="9" cy="6" r="1.4" />
      <circle cx="15" cy="6" r="1.4" />
      <circle cx="9" cy="12" r="1.4" />
      <circle cx="15" cy="12" r="1.4" />
      <circle cx="9" cy="18" r="1.4" />
      <circle cx="15" cy="18" r="1.4" />
    </svg>
  );
}
