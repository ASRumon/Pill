import { useEffect, useState } from "react";
import { WidgetIcon, widgetTheme } from "./WidgetIcon";
import { ipc } from "../lib/ipc";
import { useUpdateNotice } from "../lib/updates";
import type { WidgetKey } from "../types";

interface WidgetHeaderProps {
  active: WidgetKey;
}

export function WidgetHeader({ active }: WidgetHeaderProps) {
  const theme = widgetTheme[active];
  const [now, setNow] = useState(() => new Date());
  const { update, dismiss } = useUpdateNotice();

  useEffect(() => {
    const id = window.setInterval(() => setNow(new Date()), 1_000);
    return () => window.clearInterval(id);
  }, []);

  const time = now.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  const date = now.toLocaleDateString([], {
    weekday: "short",
    day: "numeric",
    month: "short",
    year: "numeric",
  });

  return (
    <div className="px-3.5 pt-3">
      <div className="flex items-start gap-3">
        <div
          className="grid h-8 w-8 shrink-0 place-items-center rounded-[11px] text-white shadow-md ring-1 ring-white/15"
          style={{
            background: `linear-gradient(135deg, ${theme.from}, ${theme.to})`,
            boxShadow: `0 8px 20px -8px ${theme.accent}`,
          }}
        >
          <WidgetIcon widget={active} className="h-4 w-4" />
        </div>

        <div className="min-w-0 flex-1 pt-0.5">
          <div className="truncate text-[12.5px] font-semibold tracking-tight text-[var(--text-main)]">
            {theme.label}
          </div>
          <div className="truncate text-[9.5px] text-[var(--text-muted)]">{theme.hint}</div>
        </div>

        {update && (
          <div className="flex shrink-0 items-start gap-0.5 pt-0.5">
            <button
              type="button"
              onClick={() => void ipc.openExternal(update.action_url, null)}
              title={
                `Pill ${update.latest_version} is available — ${update.action_label.toLowerCase()}` +
                (update.download_name ? `\n${update.download_name}` : "")
              }
              className="flex items-center gap-1 rounded-full border border-emerald-300/30 bg-emerald-400/15 px-2 py-[3px] text-[9px] font-medium text-emerald-100 transition hover:bg-emerald-400/25"
            >
              <span className="relative grid h-1.5 w-1.5 place-items-center">
                <span className="island-ping absolute inset-0 rounded-full bg-emerald-300/60" />
                <span className="h-1.5 w-1.5 rounded-full bg-emerald-300" />
              </span>
              {update.source === "store" ? "Update in Store" : `Update ${update.latest_version}`}
            </button>
            <button
              type="button"
              onClick={dismiss}
              aria-label="Dismiss update notice"
              title="Dismiss"
              className="grid h-4 w-4 place-items-center rounded-full text-[9px] leading-none text-white/30 transition hover:bg-white/10 hover:text-white/70"
            >
              ✕
            </button>
          </div>
        )}

        {/* Clock, date and weekday dots all stack under the time. */}
        <div className="flex shrink-0 flex-col items-end gap-[3px]">
          <div
            className="text-[21px] font-semibold leading-none tracking-tight tabular-nums text-[var(--text-main)]"
            title={now.toLocaleTimeString()}
          >
            {time}
          </div>
          <div className="text-[8.5px] leading-none tabular-nums text-[var(--text-muted)]">
            {date}
          </div>
        </div>
      </div>
    </div>
  );
}
