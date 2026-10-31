import { AnimatePresence, motion } from "framer-motion";
import { useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WidgetHeader } from "./WidgetHeader";
import { WidgetTabs } from "./WidgetTabs";
import { ActionIcon, WidgetIcon, widgetTheme } from "./WidgetIcon";
import { ClipboardWidget } from "../widgets/ClipboardWidget";
import { ShelfWidget } from "../widgets/ShelfWidget";
import { SearchWidget } from "../widgets/SearchWidget";
import { TodoWidget } from "../widgets/TodoWidget";
import { MediaWidget } from "../widgets/MediaWidget";
import { PomodoroWidget } from "../widgets/PomodoroWidget";
import { ipc } from "../lib/ipc";
import { applyIslandAppearance } from "../lib/appearance";
import { captureContent } from "../lib/dropCapture";
import { shelfStore } from "../lib/shelfStore";
import { isClipboardDragActive } from "../lib/clipboardDragState";
import { useFocusTimer } from "../lib/FocusTimerContext";
import type { AppSettings, DropPayload, ShelfFile, WidgetKey } from "../types";

const widgets: WidgetKey[] = ["clipboard", "shelf", "search", "todo", "media", "pomodoro"];

const DND_DEBUG = true;

function dndLog(event: string, details: Record<string, unknown> = {}) {
  if (!DND_DEBUG) return;
  console.info(`[island-dnd] ${event}`, details);
}

export function IslandShell() {
  const { timer, settings, remainingMs, elapsedMs } = useFocusTimer();
  const [expanded, setExpanded] = useState(false);
  const [expandingFromPill, setExpandingFromPill] = useState(false);
  const [minified, setMinified] = useState(false);
  const [modeReady, setModeReady] = useState(false);
  const [layout, setLayout] = useState<1 | 2 | 3>(2);
  const [active, setActive] = useState<WidgetKey>("clipboard");
  const [dropHover, setDropHover] = useState(false);
  const [dropProcessing, setDropProcessing] = useState(false);
  const [dropPinnedOpen, setDropPinnedOpen] = useState(false);
  const [dropSummary, setDropSummary] = useState<string | null>(null);
  const [dropSession, setDropSession] = useState<{ token: number; addedIds: string[] } | null>(null);
  const [now, setNow] = useState(() => new Date());
  const supportsGridLayout = active === "clipboard" || active === "shelf" || active === "todo";
  const expandedRef = useRef(expanded);
  const minifiedRef = useRef(minified);
  // The tiny dot only exists while nothing is running. A focus session brings
  // the full collapsed bar back, so the timer shares the island UI instead of
  // growing a second, timer-only pill.
  const tinyPill = minified && !timer.running;
  const timerRunningRef = useRef(timer.running);
  const expansionInFlight = useRef(false);
  const scheduleCollapseRef = useRef<(options?: { fromPointerLeave?: boolean }) => void>(() => undefined);
  const collapseTimer = useRef<number | undefined>(undefined);
  const pointerInsideIsland = useRef(false);
  /// Last real pointer movement over the island; the end-of-drag collapse uses
  /// it to tell "the cursor came back" apart from a stale hover flag.
  const lastPointerMoveAt = useRef(0);
  const dragLeaveTimer = useRef<number | undefined>(undefined);
  const externalFileDrag = useRef(false);
  const holdOpenAfterDrop = useRef(false);
  const pendingDropCount = useRef(0);
  const dropLockUntil = useRef(0);
  const dropSummaryTimer = useRef<number | undefined>(undefined);
  const dropPinnedTimer = useRef<number | undefined>(undefined);
  const dropPinnedOpenRef = useRef(dropPinnedOpen);
  const dropProcessingRef = useRef(dropProcessing);
  const dropSettledAt = useRef(0);
  const pointerMovedSinceDrop = useRef(false);
  const keepExpandedUntil = useRef(0);
  const regionPending = useRef<{ width: number; height: number; pill: boolean } | null>(null);
  const regionLastSent = useRef<{ width: number; height: number; pill: boolean } | null>(null);
  const regionBusy = useRef(false);
  const activeRef = useRef(active);
  // True while the user holds the click-through shortcut.
  const ghostRef = useRef(false);
  expandedRef.current = expanded;
  minifiedRef.current = minified;
  timerRunningRef.current = timer.running;
  dropPinnedOpenRef.current = dropPinnedOpen;
  dropProcessingRef.current = dropProcessing;
  activeRef.current = active;

  const pillTime = Math.ceil((timer.mode === "stopwatch" ? elapsedMs : remainingMs) / 1000);
  const pillHours = Math.floor(pillTime / 3600);
  const pillClock = pillHours > 0
    ? `${pillHours.toString().padStart(2, "0")}:${Math.floor((pillTime % 3600) / 60).toString().padStart(2, "0")}:${(pillTime % 60).toString().padStart(2, "0")}`
    : `${Math.floor(pillTime / 60).toString().padStart(2, "0")}:${(pillTime % 60).toString().padStart(2, "0")}`;
  const pillLabel = timer.mode === "focus" ? "FOCUS"
    : timer.mode === "longBreak" ? "LONG"
      : timer.mode === "shortBreak" ? "BREAK"
        : timer.mode === "stopwatch" ? "WATCH" : "TIMER";
  // Accent follows the mode so the pill reads at a glance.
  const pillAccent = timer.mode === "shortBreak" || timer.mode === "longBreak"
    ? "#34d399"
    : timer.mode === "stopwatch"
      ? "#a78bfa"
      : timer.mode === "timer"
        ? "#38bdf8"
        : "#fbbf24";
  const sessionDuration = timer.durationMs;
  const stopwatchTarget = (settings.stopwatchAlarmMinutes * 60 + settings.stopwatchAlarmSeconds) * 1000;
  const sessionProgress = timer.mode === "stopwatch"
    ? settings.stopwatchAlarmEnabled && stopwatchTarget > 0
      ? Math.min(100, elapsedMs / stopwatchTarget * 100)
      : (elapsedMs % 60_000) / 60_000 * 100
    : sessionDuration > 0
      ? Math.min(100, Math.max(0, (sessionDuration - remainingMs) / sessionDuration * 100))
      : 0;

  const updateNativeRegion = (widthValue: number | string, heightValue: number | string, force = false) => {
    const width = Number.parseFloat(String(widthValue));
    const height = Number.parseFloat(String(heightValue));
    if (!Number.isFinite(width) || !Number.isFinite(height)) return;
    // While the island is still pill-sized, keep the pill shape. Flipping to
    // the panel radius on the first expansion frame draws a tiny rectangle.
    const pill = (tinyPill && !expanded) || (expandingFromPill && height < 22);
    // The spring fires every frame; skip frames that would not change the
    // native region so the UI thread is not flooded with window IPC.
    const last = regionLastSent.current;
    if (!force && last && last.pill === pill && Math.abs(last.width - width) < 1 && Math.abs(last.height - height) < 1) {
      return;
    }
    regionLastSent.current = { width, height, pill };
    regionPending.current = { width, height, pill };
    if (regionBusy.current) return;

    regionBusy.current = true;
    void (async () => {
      while (regionPending.current) {
        const frame = regionPending.current;
        regionPending.current = null;
        try { await ipc.setIslandRegion(frame.width, frame.height, frame.pill); }
        catch { /* Native window regions are only available in Tauri. */ }
      }
      regionBusy.current = false;
    })();
  };

  /**
   * `set_island_window_bounds` always resets the native region to the
   * collapsed pill shape. Because `SetWindowRgn` clips both painting and
   * hit-testing, an expanded 540x430 panel gets cut down to a 246x36 strip
   * until the next animation frame re-sends the region — the island looks like
   * it vanished and stops responding to hover. Anything that changes native
   * bounds must re-assert the last animated region afterwards.
   */
  const reassertNativeRegion = () => {
    const last = regionLastSent.current;
    if (!last) return;
    updateNativeRegion(last.width, last.height, true);
  };

  const expand = async () => {
    window.clearTimeout(collapseTimer.current);
    if (ghostRef.current || expanded || expansionInFlight.current) return;
    expansionInFlight.current = true;

    const fromPill = minifiedRef.current && !timerRunningRef.current;
    if (fromPill) setExpandingFromPill(true);

    // Grow and re-centre the native window *before* animating. The shell is
    // centred inside the window, so animating first lays a 540px shell out in
    // the old 92px window: you see a strip of the panel on the left, then it
    // snaps right when the resize lands.
    await ipc
      .setIslandBounds(true, minifiedRef.current, timerRunningRef.current)
      .catch(() => undefined);
    reassertNativeRegion();

    setExpanded(true);
    expansionInFlight.current = false;

    window.setTimeout(() => {
      if (pointerInsideIsland.current && !externalFileDrag.current && expandedRef.current) {
        void getCurrentWindow().setFocus().catch(() => undefined);
      }
    }, 120);
  };

  const scheduleCollapse = (options?: { fromPointerLeave?: boolean }) => {
    const fromPointerLeave = options?.fromPointerLeave ?? false;
    // Read live refs: a mouseleave sets `setDropPinnedOpen(false)` in the same
    // tick, and the state variable captured by this render would still be true
    // and swallow the request entirely.
    const pinned = dropPinnedOpenRef.current;
    const processing = dropProcessingRef.current;
    dndLog("schedule-collapse", {
      externalFileDrag: externalFileDrag.current,
      processing,
      pinned,
      holdOpenAfterDrop: holdOpenAfterDrop.current,
      pointerInside: pointerInsideIsland.current,
      fromPointerLeave,
    });
    if (externalFileDrag.current || processing) return;
    if (pinned) return;
    // Never collapse out from under the cursor: collapsing while hovered
    // shrinks the native window, which fires mouseleave/mouseenter and makes
    // the island appear to vanish and pop back a moment later.
    if (pointerInsideIsland.current) return;
    window.clearTimeout(collapseTimer.current);

    // Moving the cursor away is an explicit "close it" signal, so it must not
    // wait out the post-drop hold. A stray mouseleave right after a native drop
    // is ignored unless the pointer actually moved over the island afterwards.
    const quickClose = fromPointerLeave
      && (pointerMovedSinceDrop.current || Date.now() - dropSettledAt.current > 600);
    const delay = quickClose
      ? 200
      : Math.max(
          minifiedRef.current ? 260 : 180,
          keepExpandedUntil.current - Date.now(),
          dropLockUntil.current - Date.now(),
          dropSummary ? 1800 : 0,
        );
    collapseTimer.current = window.setTimeout(() => {
      dndLog("collapse-timer-fired", {
        externalFileDrag: externalFileDrag.current,
        processing: dropProcessingRef.current,
        pinned: dropPinnedOpenRef.current,
        quickClose,
      });
      if (dropPinnedOpenRef.current) return;
      if (pointerInsideIsland.current) return;
      if (externalFileDrag.current || dropProcessingRef.current) return;
      if (quickClose || Date.now() >= dropLockUntil.current) {
        dndLog("collapse-now");
        setExpanded(false);
      }
    }, delay);
  };

  scheduleCollapseRef.current = scheduleCollapse;

  useEffect(() => {
    window.clearTimeout(dropPinnedTimer.current);
    if (!dropPinnedOpen) return;
    // Failsafe unlock so drop lock can never trap the UI. While the pointer is
    // still on the island we re-arm instead of collapsing underneath it, but
    // only a bounded number of times so a stale hover flag cannot pin it open.
    const arm = (attempt: number) => {
      dropPinnedTimer.current = window.setTimeout(() => {
        if (pointerInsideIsland.current && attempt < 6) {
          dndLog("drop-pin-rearm", { attempt });
          arm(attempt + 1);
          return;
        }
        dndLog("drop-pin-release", { attempt });
        pointerInsideIsland.current = false;
        setDropPinnedOpen(false);
        holdOpenAfterDrop.current = false;
        scheduleCollapseRef.current();
      }, 8000);
    };
    arm(0);
    return () => window.clearTimeout(dropPinnedTimer.current);
  }, [dropPinnedOpen]);

  const showDropSummary = (message: string, timeout = 2400) => {
    dndLog("drop-summary", { message, timeout });
    window.clearTimeout(dropSummaryTimer.current);
    setDropSummary(message);
    dropSummaryTimer.current = window.setTimeout(() => setDropSummary(null), timeout);
  };

  /** Text / HTML / image drops and pastes become real files in Downloads. */
  const capture = async (data: DataTransfer | null) => {
    if (!data) return;
    try {
      const saved = await captureContent(data);
      if (!saved.length) return;
      setActive("shelf");
      void expand();
      showDropSummary(
        `${saved.length} item${saved.length === 1 ? "" : "s"} saved to Downloads`,
      );
    } catch (e) {
      showDropSummary(String(e), 3200);
    }
  };

  useEffect(() => {
    const onPaste = (event: ClipboardEvent) => {
      const target = event.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)
      ) {
        return;
      }
      if (!event.clipboardData) return;
      event.preventDefault();
      void capture(event.clipboardData);
    };

    window.addEventListener("paste", onPaste);
    return () => window.removeEventListener("paste", onPaste);
  }, []);

  /** Native drop target payload (text/HTML/URL/image/file paths). */
  const ingestPayload = async (payload: DropPayload) => {
    try {
      const saved = await ipc.ingestDrop([payload]);
      if (!saved.length) return;
      setActive("shelf");
      void expand();
      showDropSummary(
        `${saved.length} item${saved.length === 1 ? "" : "s"} captured`,
      );
    } catch (e) {
      showDropSummary(String(e), 3200);
    }
  };

  const ingestRef = useRef(ingestPayload);
  const expandRef = useRef(expand);
  ingestRef.current = ingestPayload;
  expandRef.current = expand;

  useEffect(() => {
    let disposed = false;
    const disposers: Array<() => void> = [];

    const register = <T,>(name: string, handler: (payload: T) => void) => {
      listen<T>(name, (event) => handler(event.payload))
        .then((unlisten) => {
          if (disposed) unlisten();
          else disposers.push(unlisten);
        })
        .catch(() => undefined);
    };

    register<boolean>("drop-hover", (hover) => {
      setDropHover(hover);
      if (hover) expandRef.current();
    });
    register<DropPayload>("drop-payload", (payload) => {
      void ingestRef.current(payload);
    });

    // Holding the click-through shortcut drops the island out of the way so
    // clicks reach whatever is behind it.
    register<boolean>("click-through", (active) => {
      ghostRef.current = active;
      if (!active) return;
      pointerInsideIsland.current = false;
      externalFileDrag.current = false;
      holdOpenAfterDrop.current = false;
      setDropHover(false);
      setDropPinnedOpen(false);
      window.clearTimeout(collapseTimer.current);
      setExpanded(false);
    });

    return () => {
      disposed = true;
      disposers.forEach((dispose) => dispose());
    };
  }, []);

  const finishCollapse = async () => {
    while (regionBusy.current) await new Promise((resolve) => window.setTimeout(resolve, 4));
    await ipc.setIslandBounds(false, minified, timer.running).catch(() => undefined);
    reassertNativeRegion();
  };

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    ipc.getSettings().then((settings) => {
      if (!disposed) {
        setMinified(settings.minified_island);
        setModeReady(true);
      }
    }).catch(() => { if (!disposed) setModeReady(true); });
    listen<boolean>("minified-island-changed", async (event) => {
      if (!event.payload && !expandedRef.current) {
        // Grow the native frame first while retaining the small pill region;
        // the motion updates the region as the regular collapsed bar appears.
        await ipc.setIslandBounds(true, true, timerRunningRef.current).catch(() => undefined);
      }
      setMinified(event.payload);
    })
      .then((dispose) => { if (disposed) dispose(); else unlisten = dispose; })
      .catch(() => undefined);
    return () => { disposed = true; unlisten?.(); };
  }, []);

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 15_000);
    return () => window.clearInterval(id);
  }, []);

  // A native drag-out swallows the mouse events the island relies on, so a
  // widget asks for the collapse explicitly once `dragFilesOut` resolves. A
  // stale "pointer is inside" flag must not keep the panel open, but a cursor
  // that actually came back during the short grace period keeps it expanded.
  useEffect(() => {
    const onCollapseRequest = () => {
      // A drop committed on the island itself owns the open panel.
      if (dropPinnedOpenRef.current || dropProcessingRef.current) return;
      const requestedAt = Date.now();
      externalFileDrag.current = false;
      holdOpenAfterDrop.current = false;
      setDropHover(false);
      window.clearTimeout(collapseTimer.current);
      collapseTimer.current = window.setTimeout(() => {
        if (dropPinnedOpenRef.current || dropProcessingRef.current) return;
        if (externalFileDrag.current) return;
        // The pointer is genuinely back on the island.
        if (lastPointerMoveAt.current > requestedAt) return;
        pointerInsideIsland.current = false;
        setExpanded(false);
      }, 180);
    };
    window.addEventListener("island:collapse", onCollapseRequest);
    return () => window.removeEventListener("island:collapse", onCollapseRequest);
  }, []);

  // Island surface: glass (default), pure-black AMOLED, or a custom
  // opacity/blur pair. The settings window emits `settings-changed` app-wide,
  // so editing the preset recolours the island as soon as it is saved.
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;

    ipc
      .getSettings()
      .then((loaded) => {
        if (!disposed) applyIslandAppearance(loaded);
      })
      .catch(() => undefined);

    listen<AppSettings>("settings-changed", (event) => {
      applyIslandAppearance(event.payload);
    })
      .then((dispose) => {
        if (disposed) dispose();
        else unlisten = dispose;
      })
      .catch(() => undefined);

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // Auto-open the shelf when files are dragged onto the island.
  useEffect(() => {
    let dispose: (() => void) | undefined;

    (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        dispose = await getCurrentWebview().onDragDropEvent(async (event) => {
          if (isClipboardDragActive()) return;

          dndLog("native-drag-event", {
            type: event.payload.type,
            pathCount: event.payload.type === "drop" ? (event.payload.paths ?? []).length : undefined,
            expanded: expandedRef.current,
            dropPinnedOpen: dropPinnedOpenRef.current,
            dropProcessing: dropProcessingRef.current,
          });

          if (event.payload.type === "enter" || event.payload.type === "over") {
            externalFileDrag.current = true;
            dropLockUntil.current = Date.now() + 10_000;
            setDropHover(true);
            window.clearTimeout(dragLeaveTimer.current);
            window.clearTimeout(collapseTimer.current);
            if (activeRef.current !== "shelf") setActive("shelf");
            if (!expandedRef.current && !expansionInFlight.current) {
              expansionInFlight.current = true;
              if (minifiedRef.current && !timerRunningRef.current) setExpandingFromPill(true);
              await ipc.setIslandBounds(true, minifiedRef.current, timerRunningRef.current).catch(() => undefined);
              reassertNativeRegion();
              setExpanded(true);
              expansionInFlight.current = false;
            }
            return;
          }

          if (event.payload.type === "leave") {
            if (dropPinnedOpenRef.current) {
              dndLog("leave-ignored-pinned-open");
              return;
            }
            window.clearTimeout(dragLeaveTimer.current);
            dragLeaveTimer.current = window.setTimeout(() => {
              setDropHover(false);
              externalFileDrag.current = false;
              dndLog("leave-timer-fired");
              // The drag left the island without dropping: close promptly.
              scheduleCollapseRef.current({ fromPointerLeave: true });
            }, 300);
            return;
          }

          if (event.payload.type === "drop") {
            dndLog("drop-start", { paths: event.payload.paths ?? [] });
            window.clearTimeout(dragLeaveTimer.current);
            setDropHover(false);
            setDropProcessing(true);
            window.clearTimeout(collapseTimer.current);
            // Only grow the native frame when it is not already expanded: this
            // call resets the window region to the collapsed strip, which
            // would clip the open panel away the instant files are dropped.
            if (!expandedRef.current) {
              await ipc.setIslandBounds(true, minifiedRef.current, timerRunningRef.current).catch(() => undefined);
              reassertNativeRegion();
            }
            const paths = event.payload.paths ?? [];

            pendingDropCount.current = paths.length;
            const added: ShelfFile[] = [];
            const rejected: string[] = [];

            for (const path of paths) {
              try {
                const [item] = await ipc.ingestDrop([{ files: [path] }]);
                if (item) {
                  added.push(item);
                  dndLog("drop-accepted", { path, id: item.id });
                }
              } catch {
                rejected.push(path);
                dndLog("drop-rejected", { path });
              }
            }

            const acceptedCount = added.length;
            externalFileDrag.current = false;
            pointerInsideIsland.current = true;
            holdOpenAfterDrop.current = true;
            setDropPinnedOpen(true);
            keepExpandedUntil.current = Date.now() + 5200;
            dropLockUntil.current = Date.now() + 6000;
            setExpanded(true);
            setActive("shelf");
            // Store data and the arrival token are published together so the
            // tiles render once with their highlight instead of painting plain,
            // then re-rendering into the animation (the visible blink).
            shelfStore.upsert(added);
            setDropSession({ token: Date.now(), addedIds: added.map((item) => item.id) });

            if (acceptedCount > 0 && rejected.length === 0) {
              showDropSummary(`${acceptedCount} file${acceptedCount === 1 ? "" : "s"} added`);
            } else if (acceptedCount > 0 && rejected.length > 0) {
              const failedName = rejected[0]?.split(/[/\\]/).pop();
              showDropSummary(`${acceptedCount} added, ${rejected.length} failed${failedName ? ` (${failedName})` : ""}`, 3200);
            } else {
              const failedName = rejected[0]?.split(/[/\\]/).pop();
              showDropSummary(`Drop failed. No files added${failedName ? ` (${failedName})` : ""}.`, 3200);
            }

            setDropProcessing(false);
            dropSettledAt.current = Date.now();
            pointerMovedSinceDrop.current = false;
            dndLog("drop-finished", {
              acceptedCount,
              rejectedCount: rejected.length,
              dropPinnedOpen: true,
              holdOpenAfterDrop: holdOpenAfterDrop.current,
            });
            if (!dropPinnedOpenRef.current) scheduleCollapseRef.current();
          }
        });
      } catch {
        // Drag-drop events are only available inside the Tauri runtime.
      }
    })();

    return () => {
      window.clearTimeout(dropSummaryTimer.current);
      window.clearTimeout(dropPinnedTimer.current);
      dispose?.();
    };
  }, []);

  const content = useMemo(() => {
    switch (active) {
      case "clipboard":
        return <ClipboardWidget gridColumns={layout} />;
      case "shelf":
        return <ShelfWidget dropSession={dropSession ?? undefined} gridColumns={layout} />;
      case "search":
        return <SearchWidget />;
      case "todo":
        return <TodoWidget gridColumns={layout} />;
      case "media":
        return <MediaWidget />;
      case "pomodoro":
        return <PomodoroWidget />;
      default:
        return null;
    }
  }, [active, layout, dropSession]);

  if (!modeReady) return <div className="h-screen w-screen" />;

  return (
    <div
      className="relative flex h-screen w-screen justify-center"
      onDragOver={(event) => {
        // Only fires when HTML5 drag-and-drop is available; native file drops
        // still go through the Tauri event above.
        event.preventDefault();
      }}
      onDrop={(event) => {
        event.preventDefault();
        void capture(event.dataTransfer);
      }}
    >
      <motion.section
        initial={false}
        animate={{
          width: expanded ? 540 : tinyPill ? 92 : timer.running ? 288 : 246,
          height: expanded ? 430 : tinyPill ? 10 : 36,
          borderRadius: expanded ? "0px 0px 30px 30px" : tinyPill ? "9999px" : "0px 0px 20px 20px",
        }}
        transition={{ type: "spring", stiffness: 320, damping: 28 }}
        onUpdate={(latest) => updateNativeRegion(latest.width, latest.height)}
        onAnimationComplete={() => {
          if (!expanded) void finishCollapse();
          if (expanded && expandingFromPill) setExpandingFromPill(false);
        }}
        onMouseEnter={() => { pointerInsideIsland.current = true; lastPointerMoveAt.current = Date.now(); void expand(); }}
        onMouseMove={() => {
          // Real pointer presence: proves a later mouseleave is genuine.
          pointerInsideIsland.current = true;
          lastPointerMoveAt.current = Date.now();
          pointerMovedSinceDrop.current = true;
        }}
        onMouseLeave={() => {
          pointerInsideIsland.current = false;
          // A native file drag suppresses pointer events, so mouseleave fires
          // spuriously while files are being dragged in or committed.
          if (externalFileDrag.current || dropProcessingRef.current) return;
          // Clear the pin by ref so the collapse scheduled below is not
          // swallowed by the render-time state value.
          dropPinnedOpenRef.current = false;
          holdOpenAfterDrop.current = false;
          setDropPinnedOpen(false);
          scheduleCollapse({ fromPointerLeave: true });
        }}
        className={`island-shell relative overflow-hidden ${(tinyPill && !expanded) || expandingFromPill ? "island-pill" : ""} ${((tinyPill && !expanded) || expandingFromPill) && timer.running ? "island-pill-active" : ""} ${dropHover ? "island-drop-hover" : ""} ${dropProcessing ? "island-drop-processing" : ""}`}
      >
        <AnimatePresence>
          {dropHover && (
            <motion.div
              key="drop-overlay"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12 }}
              className="pointer-events-none absolute inset-0 z-20 rounded-[inherit] border border-cyan-300/55 bg-cyan-300/10 shadow-[inset_0_0_24px_rgba(56,189,248,0.18)]"
            >
              <div className="absolute inset-0 grid place-items-center text-[11px] font-medium tracking-wide text-cyan-100/90">Drop files to add to shelf</div>
            </motion.div>
          )}
        </AnimatePresence>

        <AnimatePresence>
          {dropSummary && expanded && (
            <motion.div
              key="drop-summary"
              initial={{ opacity: 0, y: -6, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, y: -6, scale: 0.98 }}
              transition={{ duration: 0.15 }}
              className="pointer-events-none absolute left-1/2 top-2 z-30 -translate-x-1/2 rounded-full border border-cyan-300/35 bg-black/45 px-3 py-1 text-[10px] font-medium text-cyan-100 backdrop-blur-md"
            >
              {dropSummary}
            </motion.div>
          )}
        </AnimatePresence>

        <AnimatePresence>
          {expanded ? (
            <motion.div
              key="expanded"
              initial={{ opacity: 0, y: -6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -4 }}
              transition={{ duration: 0.16 }}
              className="flex h-full flex-col"
            >
              <WidgetHeader active={active} />
              <WidgetTabs active={active} onSelect={setActive} />
              <div className="flex items-center justify-between px-3 pb-1.5">
                <div className="flex items-center gap-1.5 rounded-full border border-white/10 bg-white/[0.05] px-2 py-1 text-[9px] font-medium uppercase tracking-[0.09em] text-[var(--text-muted)]">
                  <ActionIcon name="spark" className="h-3 w-3" />
                  <span>{widgetTheme[active].hint}</span>
                </div>

                <div role="group" className="flex items-center gap-1 rounded-xl border border-white/10 bg-white/[0.04] p-1" aria-label="Item grid layout">
                  <span className="flex items-center gap-1 px-1 text-[9px] font-medium text-[var(--text-muted)]"><ActionIcon name="grid" className="h-3 w-3" />View</span>
                  {([1, 2, 3] as const).map((columns) => (
                    <button
                      key={columns}
                      type="button"
                      onClick={() => setLayout(columns)}
                      aria-label={`${columns} column layout`}
                      aria-pressed={layout === columns}
                      title={supportsGridLayout ? `${columns} column${columns === 1 ? "" : "s"}` : "Layout is fixed for this widget"}
                      disabled={!supportsGridLayout}
                      className={`flex h-7 min-w-8 items-center justify-center gap-1.5 rounded-lg px-1.5 text-[10px] transition ${
                        layout === columns
                          ? "bg-white/15 text-[var(--text-main)] shadow-sm"
                          : "text-[var(--text-muted)] hover:bg-white/10"
                      } ${supportsGridLayout ? "" : "cursor-not-allowed opacity-45"}`}
                    >
                      <GridLayoutIcon columns={columns} />
                      {columns}
                    </button>
                  ))}
                </div>
              </div>
              <div className="min-h-0 flex-1 overflow-hidden px-3 pb-3">{content}</div>
            </motion.div>
          ) : tinyPill ? (
            <motion.div
              key="minified"
              initial={{ opacity: 0, scale: 0.92 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0, scale: 0.94 }}
              transition={{ duration: 0.14 }}
              className="flex h-full w-full items-center justify-center rounded-full px-3 pb-[5px]"
              aria-label="Minified Pill"
            />
          ) : (
            <motion.div
              key="collapsed"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12 }}
              className="flex h-full items-center justify-center gap-[3px] px-2"
            >
              {widgets.map((key) => {
                const isActive = key === active;
                return (
                  <button
                    key={key}
                    type="button"
                    aria-label={widgetTheme[key].label}
                    onClick={() => {
                      setActive(key);
                      void expand();
                    }}
                    className="grid h-[22px] w-[22px] place-items-center rounded-full transition-transform duration-150 hover:scale-110"
                    style={{
                      color: isActive ? widgetTheme[key].accent : "rgba(255,255,255,0.34)",
                      background: isActive ? `${widgetTheme[key].accent}1f` : "transparent",
                      boxShadow: isActive ? `0 0 12px -2px ${widgetTheme[key].accent}80` : "none",
                    }}
                  >
                    <WidgetIcon widget={key} className="h-[13px] w-[13px]" />
                  </button>
                );
              })}

              {!timer.running && <span className="mx-1 h-3 w-px bg-white/12" />}

              {timer.running ? (
                // A running session owns the right side of the bar: the local
                // clock is replaced by the live chip so the session is visible
                // without minifying the island.
                <button
                  type="button"
                  onClick={() => {
                    setActive("pomodoro");
                    void expand();
                  }}
                  aria-label={`${pillLabel} session, ${pillClock} remaining. Open focus.`}
                  title="Open focus session"
                  className="relative flex items-center gap-1.5 rounded-full px-2 py-[3px] transition hover:brightness-110"
                  style={{
                    background: `${pillAccent}1c`,
                    boxShadow: `inset 0 0 0 1px ${pillAccent}55`,
                  }}
                >
                  <span className="relative grid h-3.5 w-3.5 shrink-0 place-items-center">
                    <span
                      className="island-ping pointer-events-none absolute inset-0 rounded-full"
                      style={{ background: `${pillAccent}44` }}
                    />
                    <span
                      className="island-breathe relative grid h-3.5 w-3.5 place-items-center rounded-full"
                      style={{ background: `${pillAccent}30` }}
                    >
                      <PillModeIcon mode={timer.mode} color={pillAccent} />
                    </span>
                  </span>
                  <span
                    className="text-[8px] font-semibold uppercase leading-none tracking-[0.14em]"
                    style={{ color: `${pillAccent}dd` }}
                  >
                    {pillLabel}
                  </span>
                  <span
                    className="text-[11px] font-semibold leading-none tabular-nums text-white"
                    style={{ textShadow: `0 0 10px ${pillAccent}55` }}
                  >
                    {pillClock}
                  </span>
                  <span className="absolute inset-x-1.5 bottom-[2px] h-[2px] overflow-hidden rounded-full bg-white/10">
                    <span
                      className="relative block h-full overflow-hidden rounded-full transition-[width] duration-500 ease-out"
                      style={{
                        width: `${Math.max(2, sessionProgress)}%`,
                        background: `linear-gradient(90deg, ${pillAccent}99, ${pillAccent})`,
                      }}
                    >
                      <span className="island-shimmer absolute inset-y-0 w-6 bg-gradient-to-r from-transparent via-white/60 to-transparent" />
                    </span>
                  </span>
                </button>
              ) : (
                <span className="text-[10px] font-medium tabular-nums tracking-wide text-white/45">
                  {now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
                </span>
              )}
            </motion.div>
          )}
        </AnimatePresence>
      </motion.section>
    </div>
  );
}

function GridLayoutIcon({ columns }: { columns: 1 | 2 | 3 }) {
  const gap = 2;
  const size = (18 - gap * (columns - 1)) / columns;
  return (
    <svg viewBox="0 0 22 22" className="h-3.5 w-3.5" fill="currentColor" aria-hidden="true">
      {Array.from({ length: columns * columns }, (_, index) => {
        const row = Math.floor(index / columns);
        const column = index % columns;
        return <rect key={index} x={2 + column * (size + gap)} y={2 + row * (size + gap)} width={size} height={size} rx="1.5" />;
      })}
    </svg>
  );
}

/** Mode glyph for the active-timer pill. */
function PillModeIcon({ mode, color }: { mode: string; color: string }) {
  const props = {
    viewBox: "0 0 24 24",
    className: "h-2.5 w-2.5",
    fill: "none",
    stroke: color,
    strokeWidth: 2.2,
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    "aria-hidden": true,
  };

  if (mode === "shortBreak" || mode === "longBreak") {
    return (
      <svg {...props}>
        <path d="M5.5 8h10v4.5a5 5 0 0 1-5 5 5 5 0 0 1-5-5Z" />
        <path d="M15.5 9.4h1.4a2.1 2.1 0 0 1 0 4.2h-1.4" />
      </svg>
    );
  }

  if (mode === "timer") {
    return (
      <svg {...props}>
        <path d="M7.5 4h9v2c0 2-2 2.9-2 6s2 4 2 6v2h-9v-2c0-2 2-2.9 2-6s-2-4-2-6Z" />
      </svg>
    );
  }

  if (mode === "stopwatch") {
    return (
      <svg {...props}>
        <circle cx="12" cy="13.5" r="6.2" />
        <path d="M12 10.2v3.4l2.2 1.3M9.7 3.6h4.6M12 3.6v2.6" />
      </svg>
    );
  }

  return (
    <svg {...props}>
      <circle cx="12" cy="12" r="6.4" />
      <circle cx="12" cy="12" r="2" />
    </svg>
  );
}
