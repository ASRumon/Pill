import { getCurrentWindow } from "@tauri-apps/api/window";

/**
 * Window buttons for the runtime-created windows (settings, clip editor).
 *
 * Those windows are built with `decorations: false` + `transparent: true`, so
 * Windows no longer paints the native title bar (caption, close/minimize, the
 * painted frame). Dragging is handled by `data-tauri-drag-region` on the
 * surrounding header instead.
 */
async function control(action: "minimize" | "close") {
  try {
    const win = getCurrentWindow();
    if (action === "close") await win.close();
    else await win.minimize();
  } catch {
    // Plain-browser preview: only closing has a fallback.
    if (action === "close") window.close();
  }
}

export function WindowControls({ className }: { className?: string }) {
  return (
    <div className={`flex shrink-0 items-center gap-1 ${className ?? ""}`}>
      <button
        type="button"
        aria-label="Minimize window"
        title="Minimize"
        onClick={() => void control("minimize")}
        className="grid h-6 w-6 place-items-center rounded-md text-white/40 transition hover:bg-white/10 hover:text-white/85"
      >
        <svg viewBox="0 0 12 12" className="h-3 w-3" aria-hidden="true">
          <path d="M2.5 6h7" stroke="currentColor" strokeWidth="1.3" strokeLinecap="round" />
        </svg>
      </button>
      <button
        type="button"
        aria-label="Close window"
        title="Close"
        onClick={() => void control("close")}
        className="grid h-6 w-6 place-items-center rounded-md text-white/40 transition hover:bg-rose-500/85 hover:text-white"
      >
        <svg viewBox="0 0 12 12" className="h-3 w-3" aria-hidden="true">
          <path
            d="M3 3l6 6M9 3l-6 6"
            stroke="currentColor"
            strokeWidth="1.3"
            strokeLinecap="round"
          />
        </svg>
      </button>
    </div>
  );
}
