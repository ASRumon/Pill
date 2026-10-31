import { motion } from "framer-motion";
import { useEffect } from "react";
import { WidgetIcon, widgetTheme } from "./WidgetIcon";
import { isListNavigationActive } from "../lib/listNavigation";
import type { WidgetKey } from "../types";

const tabs: WidgetKey[] = [
  "clipboard",
  "shelf",
  "search",
  "todo",
  "media",
  "pomodoro",
];

interface WidgetTabsProps {
  active: WidgetKey;
  onSelect: (widget: WidgetKey) => void;
}

export function WidgetTabs({ active, onSelect }: WidgetTabsProps) {
  // No focus effect here on purpose: moving focus to the selected tab ran a
  // frame *after* a newly mounted widget focused its own field, so the Search
  // box lost focus right after opening.
  //
  // Instead, arrow keys are handled at the window level. Nothing on the island
  // holds focus after expanding, so a per-button handler alone only worked
  // after clicking a tab. The tablist unmounts when the island collapses, so
  // this listener is naturally scoped to an open island.
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      // A list with a selection owns the arrows (grid navigation).
      if (isListNavigationActive()) return;
      // The tab buttons handle their own arrows (and move focus with them).
      if (event.defaultPrevented) return;

      const target = event.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA" || target?.isContentEditable) {
        // Typed text keeps the arrows for caret movement.
        const value = (target as HTMLInputElement).value ?? "";
        if (value.length > 0) return;
      }

      event.preventDefault();
      const offset = event.key === "ArrowRight" ? 1 : -1;
      const currentIndex = tabs.indexOf(active);
      onSelect(tabs[(currentIndex + offset + tabs.length) % tabs.length]);
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [active, onSelect]);

  return (
    <div
      role="tablist"
      aria-label="Island widgets"
      className="mx-3 mb-2 flex items-center gap-1 rounded-2xl border border-white/10 bg-white/[0.04] p-1 shadow-sm backdrop-blur-xl"
    >
      {tabs.map((key) => {
        const theme = widgetTheme[key];
        const isActive = key === active;

        return (
          <button
            key={key}
            type="button"
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            onClick={() => onSelect(key)}
            onKeyDown={(event) => {
              if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
              if (isListNavigationActive()) return;
              event.preventDefault();
              const currentIndex = tabs.indexOf(key);
              const offset = event.key === "ArrowRight" ? 1 : -1;
              const nextIndex = (currentIndex + offset + tabs.length) % tabs.length;
              const nextTab = tabs[nextIndex];
              onSelect(nextTab);
              event.currentTarget.parentElement
                ?.querySelectorAll<HTMLButtonElement>('[role="tab"]')[nextIndex]
                ?.focus();
            }}
            aria-label={theme.label}
            title={theme.label}
            className="relative flex min-w-0 flex-1 items-center justify-center gap-1.5 rounded-xl px-1.5 py-2 text-[9px] font-medium transition-colors duration-150"
            style={{ color: isActive ? "var(--text-main)" : "var(--text-muted)" }}
          >
            {isActive && (
              <motion.span
                layoutId="tab-pill"
                transition={{ type: "spring", stiffness: 420, damping: 34 }}
                className="absolute inset-0 rounded-xl"
                style={{
                  background: `linear-gradient(135deg, ${theme.from}38, ${theme.to}1f)`,
                  boxShadow: `inset 0 0 0 1px ${theme.accent}55, 0 2px 8px ${theme.accent}12`,
                }}
              />
            )}
            <WidgetIcon widget={key} className="relative h-4 w-4" />
            <span className="relative hidden truncate min-[410px]:inline">{theme.label === "Now Playing" ? "Media" : theme.label === "File Shelf" ? "Files" : theme.label}</span>
          </button>
        );
      })}
    </div>
  );
}
