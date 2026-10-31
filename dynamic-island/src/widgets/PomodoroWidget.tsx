import { useEffect, useState } from "react";
import { useFocusTimer } from "../lib/FocusTimerContext";
import type { FocusTimerSettings, TimerMode } from "../lib/FocusTimerContext";
import { ActionIcon } from "../components/WidgetIcon";

const modeButtons: Array<{ mode: TimerMode; label: string }> = [
  { mode: "focus", label: "Focus" },
  { mode: "shortBreak", label: "Short" },
  { mode: "longBreak", label: "Long" },
  { mode: "timer", label: "Timer" },
  { mode: "stopwatch", label: "Watch" },
];

const modeTitle: Record<TimerMode, string> = {
  focus: "Focus",
  shortBreak: "Short break",
  longBreak: "Long break",
  timer: "Timer",
  stopwatch: "Stopwatch",
};

const RADIUS = 54;

function clock(ms: number): string {
  const seconds = Math.max(0, Math.ceil(ms / 1000));
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const remainder = seconds % 60;
  return hours > 0
    ? `${hours.toString().padStart(2, "0")}:${minutes.toString().padStart(2, "0")}:${remainder.toString().padStart(2, "0")}`
    : `${minutes.toString().padStart(2, "0")}:${remainder.toString().padStart(2, "0")}`;
}

export function PomodoroWidget() {
  const {
    timer,
    settings,
    remainingMs,
    elapsedMs,
    recommendedBreak,
    updateSettings,
    start,
    startCustomTimer,
    resume,
    pause,
    reset,
  } = useFocusTimer();

  const [showOptions, setShowOptions] = useState(false);
  const [showNewTimer, setShowNewTimer] = useState(false);
  const [customMinutes, setCustomMinutes] = useState("10");
  const [customSeconds, setCustomSeconds] = useState("0");

  const mode = timer.mode;
  const isStopwatch = mode === "stopwatch";
  const valueMs = isStopwatch ? elapsedMs : remainingMs;
  const total = isStopwatch ? 60_000 : timer.durationMs;
  const progress = isStopwatch
    ? ((elapsedMs % 60_000) / 60_000) * 100
    : total > 0
      ? Math.min(100, Math.max(0, ((total - remainingMs) / total) * 100))
      : 0;

  const circumference = 2 * Math.PI * RADIUS;
  const cycle = Math.max(1, settings.longBreakEvery);
  const completedInSet = timer.completedFocusSessions % cycle;
  const sessionNumber = completedInSet + 1;
  const untouched = !isStopwatch && remainingMs === total;

  const setNumber = (key: keyof FocusTimerSettings, value: string, min: number, max: number) => {
    const parsed = Number.parseInt(value, 10);
    if (Number.isFinite(parsed)) {
      updateSettings({ ...settings, [key]: Math.min(max, Math.max(min, parsed)) });
    }
  };

  return (
    <div className="widget-card flex h-full min-h-0 flex-col rounded-2xl p-3">
      <div className="mb-1 flex shrink-0 items-center gap-1">
        {modeButtons.map(({ mode: choice, label }) => (
          <button
            key={choice}
            type="button"
            onClick={() => start(choice)}
            aria-pressed={mode === choice}
            className={`rounded-full px-2 py-[3px] text-[9.5px] transition ${
              mode === choice
                ? "bg-amber-300/20 text-amber-100 ring-1 ring-amber-200/35"
                : "bg-white/[0.06] text-white/50 hover:bg-white/10"
            }`}
          >
            {label}
          </button>
        ))}
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            onClick={() => setShowNewTimer((value) => !value)}
            aria-label="Create a timer with a custom duration"
            aria-expanded={showNewTimer}
            title="New custom timer"
            className="grid h-5 w-5 place-items-center rounded-full bg-amber-300/15 text-[11px] leading-none text-amber-100 hover:bg-amber-300/25"
          >
            +
          </button>
          <button
            type="button"
            onClick={() => setShowOptions((value) => !value)}
            aria-expanded={showOptions}
            title="Timer settings"
            className={`rounded-full px-2 py-[3px] text-[9.5px] transition ${
              showOptions ? "bg-white/15 text-white/80" : "bg-white/[0.06] text-white/50 hover:bg-white/10"
            }`}
          >
            Options
          </button>
        </div>
      </div>

      {/* The ring, the custom-timer form and the options tray share one scroll
          area so opening Options can never push the widget past the panel. */}
      <div className="flex min-h-0 flex-1 flex-col overflow-y-auto pr-0.5">
      {showNewTimer && (
        <form
          className="mb-2 flex items-end gap-2 rounded-xl bg-white/[0.04] p-2"
          onSubmit={(event) => {
            event.preventDefault();
            const minutes = Math.max(0, Math.min(999, Number.parseInt(customMinutes, 10) || 0));
            const seconds = Math.max(0, Math.min(59, Number.parseInt(customSeconds, 10) || 0));
            if (minutes * 60 + seconds === 0) return;
            startCustomTimer((minutes * 60 + seconds) * 1000);
            setShowNewTimer(false);
          }}
        >
          <span className="mr-auto pb-1 text-[10px] text-white/60">New timer</span>
          <label className="text-[8.5px] text-white/40">
            Min
            <input
              type="number"
              min="0"
              max="999"
              value={customMinutes}
              onChange={(event) => setCustomMinutes(event.target.value)}
              className="mt-0.5 block w-12 rounded bg-black/25 px-2 py-1 text-center text-[11px] text-white outline-none focus:ring-1 focus:ring-amber-200/50"
            />
          </label>
          <label className="text-[8.5px] text-white/40">
            Sec
            <input
              type="number"
              min="0"
              max="59"
              value={customSeconds}
              onChange={(event) => setCustomSeconds(event.target.value)}
              className="mt-0.5 block w-12 rounded bg-black/25 px-2 py-1 text-center text-[11px] text-white outline-none focus:ring-1 focus:ring-amber-200/50"
            />
          </label>
          <button
            type="submit"
            disabled={(Number(customMinutes) || 0) * 60 + (Number(customSeconds) || 0) <= 0}
            className="rounded-lg bg-amber-300/15 px-2.5 py-1.5 text-[10px] text-amber-100 hover:bg-amber-300/25 disabled:cursor-not-allowed disabled:opacity-40"
          >
            Start
          </button>
        </form>
      )}

      <div className="flex flex-col items-center pt-1">
        <div className="relative h-[142px] w-[142px]">
          <svg viewBox="0 0 120 120" className="h-full w-full -rotate-90">
            <circle cx="60" cy="60" r={RADIUS} fill="none" stroke="rgba(255,255,255,0.07)" strokeWidth="7" />
            <circle
              cx="60"
              cy="60"
              r={RADIUS}
              fill="none"
              stroke="url(#focusRing)"
              strokeWidth="7"
              strokeLinecap="round"
              strokeDasharray={circumference}
              strokeDashoffset={circumference - (progress / 100) * circumference}
              style={{ transition: "stroke-dashoffset 300ms linear" }}
            />
            <defs>
              <linearGradient id="focusRing" x1="0" y1="0" x2="1" y2="1">
                <stop offset="0%" stopColor="#fcd34d" />
                <stop offset="100%" stopColor="#f97316" />
              </linearGradient>
            </defs>
          </svg>

          <div className="absolute inset-0 flex flex-col items-center justify-center">
            <span className="text-[31px] font-light leading-none tracking-tight tabular-nums text-white/95">
              {clock(valueMs)}
            </span>
            <span className="mt-1.5 text-[8.5px] uppercase tracking-[0.2em] text-white/40">
              {modeTitle[mode]}
            </span>
            {mode === "focus" && (
              <span className="mt-1 text-[9px] text-amber-200/70">
                Session {sessionNumber} of {cycle}
              </span>
            )}
          </div>
        </div>

        {!isStopwatch && (
          <div
            className="mt-2 flex items-center gap-1.5"
            title={`${completedInSet} of ${cycle} focus sessions done in this set`}
          >
            {Array.from({ length: cycle }).map((_, index) => (
              <span
                key={index}
                className={`h-1.5 w-1.5 rounded-full transition ${
                  index < completedInSet ? "bg-amber-300" : "bg-white/15"
                }`}
              />
            ))}
          </div>
        )}

        <div className="mt-3 flex w-full max-w-[250px] items-center gap-2">
          <button
            type="button"
            onClick={timer.running ? pause : resume}
            className="flex-1 rounded-xl bg-amber-300/20 px-3 py-2 text-[12px] font-semibold text-amber-100 ring-1 ring-amber-200/30 transition hover:bg-amber-300/30"
          >
            {timer.running ? "Pause" : untouched ? "Start" : "Resume"}
          </button>
          <button
            type="button"
            onClick={reset}
            aria-label="Reset timer"
            title="Reset"
            className="grid h-8 w-8 place-items-center rounded-xl bg-white/[0.06] text-white/60 transition hover:bg-white/10 hover:text-white"
          >
            <ActionIcon name="reset" className="h-3.5 w-3.5" />
          </button>
          {mode === "focus" && (
            <button
              type="button"
              onClick={() => start(recommendedBreak)}
              title={`Skip to ${recommendedBreak === "longBreak" ? "long" : "short"} break`}
              className="rounded-xl bg-white/[0.06] px-2.5 py-2 text-[10px] text-white/60 transition hover:bg-white/10 hover:text-white"
            >
              Break
            </button>
          )}
        </div>
      </div>

      {showOptions && (
        <div className="mt-3 grid grid-cols-2 gap-2 border-t border-white/10 pt-3">
          <NumberOption label="Focus · min" value={settings.focusMinutes} min={1} max={240} onChange={(value) => setNumber("focusMinutes", value, 1, 240)} />
          <NumberOption label="Short break · min" value={settings.shortBreakMinutes} min={1} max={120} onChange={(value) => setNumber("shortBreakMinutes", value, 1, 120)} />
          <NumberOption label="Long break · min" value={settings.longBreakMinutes} min={1} max={180} onChange={(value) => setNumber("longBreakMinutes", value, 1, 180)} />
          <NumberOption label="Long break every" value={settings.longBreakEvery} min={2} max={12} onChange={(value) => setNumber("longBreakEvery", value, 2, 12)} suffix="sessions" />
          <NumberOption label="Timer · min" value={settings.timerMinutes} min={0} max={999} onChange={(value) => setNumber("timerMinutes", value, 0, 999)} />
          <NumberOption label="Timer · sec" value={settings.timerSeconds} min={0} max={59} onChange={(value) => setNumber("timerSeconds", value, 0, 59)} />
          <NumberOption label="Stopwatch alarm minutes" value={settings.stopwatchAlarmMinutes} min={0} max={999} onChange={(value) => setNumber("stopwatchAlarmMinutes", value, 0, 999)} />
          <NumberOption label="Stopwatch alarm seconds" value={settings.stopwatchAlarmSeconds} min={0} max={59} onChange={(value) => setNumber("stopwatchAlarmSeconds", value, 0, 59)} />
          <label className="col-span-2 flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-2 text-[10px] text-white/65">
            <input
              type="checkbox"
              checked={settings.stopwatchAlarmEnabled}
              onChange={(event) => updateSettings({ ...settings, stopwatchAlarmEnabled: event.target.checked })}
              className="accent-amber-300"
            />
            Alarm once when the stopwatch reaches this time
          </label>
          <label className="col-span-2 flex items-center gap-2 rounded-lg bg-white/[0.04] px-2.5 py-2 text-[10px] text-white/65">
            <input
              type="checkbox"
              checked={settings.alarmEnabled}
              onChange={(event) => updateSettings({ ...settings, alarmEnabled: event.target.checked })}
              className="accent-amber-300"
            />
            Play alarm and show notification when finished
          </label>
        </div>
      )}
      </div>

      <p className="mt-2 shrink-0 text-center text-[9px] text-white/30">
        {timer.completedFocusSessions} focus session{timer.completedFocusSessions === 1 ? "" : "s"} completed · preferences save automatically
      </p>
    </div>
  );
}

function NumberOption({
  label,
  value,
  min,
  max,
  onChange,
  suffix,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  onChange: (value: string) => void;
  suffix?: string;
}) {
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);

  return (
    <label className="flex items-center justify-between gap-2 rounded-lg bg-white/[0.04] px-2.5 py-2 text-[10px] text-white/55">
      <span>{label}</span>
      <span className="flex items-center gap-1">
        <input
          type="number"
          min={min}
          max={max}
          value={draft}
          onChange={(event) => {
            const next = event.target.value;
            setDraft(next);
            if (/^\d+$/.test(next)) onChange(next);
          }}
          onBlur={() => {
            const parsed = Number.parseInt(draft, 10);
            const normalized = Number.isFinite(parsed) ? Math.min(max, Math.max(min, parsed)) : min;
            setDraft(String(normalized));
            onChange(String(normalized));
          }}
          className="w-12 rounded bg-black/25 px-1.5 py-1 text-right text-white outline-none focus:ring-1 focus:ring-amber-200/50"
        />
        {suffix && <span className="text-[8px]">{suffix}</span>}
      </span>
    </label>
  );
}
