import { createContext, useContext, useEffect, useMemo, useRef, useState } from "react";
import type { ReactNode } from "react";

export type TimerMode = "focus" | "shortBreak" | "longBreak" | "timer" | "stopwatch";

export interface FocusTimerSettings {
  focusMinutes: number;
  shortBreakMinutes: number;
  longBreakMinutes: number;
  longBreakEvery: number;
  timerMinutes: number;
  timerSeconds: number;
  alarmEnabled: boolean;
  stopwatchAlarmEnabled: boolean;
  stopwatchAlarmMinutes: number;
  stopwatchAlarmSeconds: number;
}

interface TimerSnapshot {
  mode: TimerMode;
  running: boolean;
  durationMs: number;
  remainingMs: number;
  elapsedMs: number;
  startedAt: number | null;
  completedFocusSessions: number;
  stopwatchAlarmFired: boolean;
}

export interface PomodoroInstance {
  id: string;
  mode: "focus" | "shortBreak" | "longBreak";
  running: boolean;
  remainingMs: number;
  durationMs: number;
  startedAt: number | null;
  completedSessions: number;
}

interface FocusTimerValue {
  timer: TimerSnapshot;
  pomodoros: PomodoroInstance[];
  settings: FocusTimerSettings;
  now: number;
  remainingMs: number;
  elapsedMs: number;
  recommendedBreak: "shortBreak" | "longBreak";
  updateSettings: (next: FocusTimerSettings) => void;
  start: (mode: TimerMode) => void;
  startCustomTimer: (durationMs: number) => void;
  resume: () => void;
  pause: () => void;
  reset: () => void;
  createPomodoro: () => void;
  togglePomodoro: (id: string) => void;
  resetPomodoro: (id: string) => void;
  removePomodoro: (id: string) => void;
}

const SETTINGS_KEY = "dynamic-island-focus-settings";
const defaultSettings: FocusTimerSettings = {
  focusMinutes: 25,
  shortBreakMinutes: 5,
  longBreakMinutes: 15,
  longBreakEvery: 4,
  timerMinutes: 10,
  timerSeconds: 0,
  alarmEnabled: true,
  stopwatchAlarmEnabled: false,
  stopwatchAlarmMinutes: 1,
  stopwatchAlarmSeconds: 0,
};

function loadSettings(): FocusTimerSettings {
  try {
    const saved = JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "null") as Partial<FocusTimerSettings> | null;
    if (!saved) return defaultSettings;
    const clamp = (value: unknown, fallback: number, min: number, max: number) =>
      typeof value === "number" && Number.isFinite(value)
        ? Math.min(max, Math.max(min, Math.round(value)))
        : fallback;
    return {
      focusMinutes: clamp(saved.focusMinutes, 25, 1, 240),
      shortBreakMinutes: clamp(saved.shortBreakMinutes, 5, 1, 120),
      longBreakMinutes: clamp(saved.longBreakMinutes, 15, 1, 180),
      longBreakEvery: clamp(saved.longBreakEvery, 4, 2, 12),
      timerMinutes: clamp(saved.timerMinutes, 10, 0, 999),
      timerSeconds: clamp(saved.timerSeconds, 0, 0, 59),
      alarmEnabled: saved.alarmEnabled ?? true,
      stopwatchAlarmEnabled: saved.stopwatchAlarmEnabled ?? false,
      stopwatchAlarmMinutes: clamp(saved.stopwatchAlarmMinutes, 1, 0, 999),
      stopwatchAlarmSeconds: clamp(saved.stopwatchAlarmSeconds, 0, 0, 59),
    };
  } catch {
    return defaultSettings;
  }
}

function durationFor(mode: TimerMode, settings: FocusTimerSettings): number {
  switch (mode) {
    case "focus": return settings.focusMinutes * 60_000;
    case "shortBreak": return settings.shortBreakMinutes * 60_000;
    case "longBreak": return settings.longBreakMinutes * 60_000;
    case "timer": return Math.max(1000, (settings.timerMinutes * 60 + settings.timerSeconds) * 1000);
    case "stopwatch": return 0;
  }
}

async function sendCompletionNotice(mode: TimerMode) {
  try {
    const { isPermissionGranted, requestPermission, sendNotification } = await import("@tauri-apps/plugin-notification");
    let granted = await isPermissionGranted();
    if (!granted) granted = (await requestPermission()) === "granted";
    if (!granted) return;
    const isBreak = mode === "shortBreak" || mode === "longBreak";
    sendNotification({
      title: mode === "stopwatch" ? "Stopwatch alarm" : isBreak ? "Break finished" : "Timer finished",
      body: mode === "focus"
        ? "Focus session complete. Time for a break."
        : isBreak
          ? "Break complete. Ready to focus again?"
          : mode === "stopwatch"
            ? "Your stopwatch alarm time has been reached."
            : "Your timer is done.",
    });
  } catch {
    // Notifications are optional outside the Tauri runtime.
  }
}

function playAlarm(context: AudioContext | null) {
  if (!context) return;
  void context.resume().then(() => {
    const startAt = context.currentTime;
    [880, 660, 880].forEach((frequency, index) => {
      const oscillator = context.createOscillator();
      const gain = context.createGain();
      oscillator.frequency.value = frequency;
      gain.gain.setValueAtTime(0.0001, startAt + index * 0.32);
      gain.gain.exponentialRampToValueAtTime(0.12, startAt + index * 0.32 + 0.025);
      gain.gain.exponentialRampToValueAtTime(0.0001, startAt + index * 0.32 + 0.25);
      oscillator.connect(gain);
      gain.connect(context.destination);
      oscillator.start(startAt + index * 0.32);
      oscillator.stop(startAt + index * 0.32 + 0.26);
    });
  }).catch(() => undefined);
}

const FocusTimerContext = createContext<FocusTimerValue | null>(null);

export function FocusTimerProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState(loadSettings);
  const [timer, setTimer] = useState<TimerSnapshot>(() => ({
    mode: "focus",
    running: false,
    durationMs: loadSettings().focusMinutes * 60_000,
    remainingMs: loadSettings().focusMinutes * 60_000,
    elapsedMs: 0,
    startedAt: null,
    completedFocusSessions: 0,
    stopwatchAlarmFired: false,
  }));
  const [pomodoros, setPomodoros] = useState<PomodoroInstance[]>([]);
  const [now, setNow] = useState(Date.now);
  const audioContext = useRef<AudioContext | null>(null);

  useEffect(() => {
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(settings));
    } catch {
      // The timer remains usable when browser storage is unavailable.
    }
  }, [settings]);

  useEffect(() => {
    if (!timer.running && !pomodoros.some((item) => item.running)) return;
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, [timer.running, pomodoros]);

  useEffect(() => {
    const at = Date.now();
    setPomodoros((items) => items.map((item) => {
      if (!item.running || !item.startedAt || at - item.startedAt < item.remainingMs) return item;
      const completedSessions = item.completedSessions + (item.mode === "focus" ? 1 : 0);
      const nextMode = item.mode === "focus"
        ? (completedSessions % settings.longBreakEvery === 0 ? "longBreak" : "shortBreak")
        : "focus";
      const durationMs = durationFor(nextMode, settings);
      return { ...item, mode: nextMode, completedSessions, durationMs, remainingMs: durationMs, startedAt: at };
    }));
  }, [now, settings]);

  const remainingMs = timer.mode === "stopwatch"
    ? 0
    : Math.max(0, timer.remainingMs - (timer.running && timer.startedAt ? now - timer.startedAt : 0));
  const elapsedMs = timer.mode === "stopwatch"
    ? timer.elapsedMs + (timer.running && timer.startedAt ? now - timer.startedAt : 0)
    : 0;

  useEffect(() => {
    if (!timer.running || timer.mode === "stopwatch" || remainingMs > 0) return;
    const finishedMode = timer.mode;
    setTimer((current) => current.running
      ? {
          ...current,
          running: false,
          remainingMs: 0,
          startedAt: null,
          completedFocusSessions: current.completedFocusSessions + (finishedMode === "focus" ? 1 : 0),
        }
      : current);

    if (settings.alarmEnabled) {
      playAlarm(audioContext.current);
      void sendCompletionNotice(finishedMode);
    }
  }, [remainingMs, timer.mode, timer.running, settings.alarmEnabled]);

  const stopwatchAlarmMs = (settings.stopwatchAlarmMinutes * 60 + settings.stopwatchAlarmSeconds) * 1000;
  useEffect(() => {
    if (
      timer.mode !== "stopwatch"
      || !timer.running
      || timer.stopwatchAlarmFired
      || !settings.stopwatchAlarmEnabled
      || !settings.alarmEnabled
      || stopwatchAlarmMs <= 0
      || elapsedMs < stopwatchAlarmMs
    ) return;

    setTimer((current) => current.running && !current.stopwatchAlarmFired
      ? { ...current, stopwatchAlarmFired: true }
      : current);
    playAlarm(audioContext.current);
    void sendCompletionNotice("stopwatch");
  }, [timer.mode, timer.running, timer.stopwatchAlarmFired, settings.stopwatchAlarmEnabled, settings.alarmEnabled, stopwatchAlarmMs, elapsedMs]);

  const updateSettings = (next: FocusTimerSettings) => {
    setSettings(next);
  };

  const start = (mode: TimerMode) => {
    if (typeof AudioContext !== "undefined") {
      audioContext.current ??= new AudioContext();
      void audioContext.current.resume().catch(() => undefined);
    }
    const duration = durationFor(mode, settings);
    setTimer((current) => ({
      ...current,
      mode,
      running: true,
      durationMs: duration,
      remainingMs: duration,
      elapsedMs: 0,
      startedAt: Date.now(),
      stopwatchAlarmFired: false,
    }));
    setNow(Date.now());
  };

  const startCustomTimer = (durationMs: number) => {
    if (typeof AudioContext !== "undefined") {
      audioContext.current ??= new AudioContext();
      void audioContext.current.resume().catch(() => undefined);
    }
    const duration = Math.max(1000, durationMs);
    setTimer((current) => ({
      ...current,
      mode: "timer",
      running: true,
      durationMs: duration,
      remainingMs: duration,
      elapsedMs: 0,
      startedAt: Date.now(),
      stopwatchAlarmFired: false,
    }));
    setNow(Date.now());
  };

  const resume = () => {
    if (typeof AudioContext !== "undefined") {
      audioContext.current ??= new AudioContext();
      void audioContext.current.resume().catch(() => undefined);
    }
    const startedAt = Date.now();
    setTimer((current) => {
      const restart = current.mode !== "stopwatch" && current.remainingMs <= 0;
      const duration = restart
        ? current.mode === "timer" ? current.durationMs : durationFor(current.mode, settings)
        : current.durationMs;
      return {
        ...current,
        running: true,
        durationMs: duration,
        remainingMs: restart ? duration : current.remainingMs,
        elapsedMs: current.mode === "stopwatch" ? current.elapsedMs : 0,
        startedAt,
      };
    });
    setNow(startedAt);
  };

  const pause = () => {
    setTimer((current) => {
      if (!current.running || !current.startedAt) return current;
      const elapsed = Date.now() - current.startedAt;
      return current.mode === "stopwatch"
        ? { ...current, running: false, elapsedMs: current.elapsedMs + elapsed, startedAt: null }
        : { ...current, running: false, remainingMs: Math.max(0, current.remainingMs - elapsed), startedAt: null };
    });
    setNow(Date.now());
  };

  const reset = () => {
    setTimer((current) => {
      const duration = current.mode === "timer" ? current.durationMs : durationFor(current.mode, settings);
      return {
        ...current,
        running: false,
        durationMs: duration,
        remainingMs: duration,
        elapsedMs: 0,
        startedAt: null,
        stopwatchAlarmFired: false,
      };
    });
    setNow(Date.now());
  };

  const createPomodoro = () => {
    const durationMs = durationFor("focus", settings);
    setPomodoros((items) => [...items, {
      id: crypto.randomUUID(), mode: "focus", running: true, durationMs,
      remainingMs: durationMs, startedAt: Date.now(), completedSessions: 0,
    }]);
    setNow(Date.now());
  };
  const togglePomodoro = (id: string) => setPomodoros((items) => items.map((item) => {
    if (item.id !== id) return item;
    if (item.running) {
      const remainingMs = Math.max(0, item.remainingMs - (item.startedAt ? Date.now() - item.startedAt : 0));
      return { ...item, running: false, remainingMs, startedAt: null };
    }
    return { ...item, running: true, startedAt: Date.now() };
  }));
  const resetPomodoro = (id: string) => setPomodoros((items) => items.map((item) => {
    if (item.id !== id) return item;
    const durationMs = durationFor("focus", settings);
    return { ...item, mode: "focus", running: false, durationMs, remainingMs: durationMs, startedAt: null, completedSessions: 0 };
  }));
  const removePomodoro = (id: string) => setPomodoros((items) => items.filter((item) => item.id !== id));

  const recommendedBreak = timer.completedFocusSessions > 0
    && timer.completedFocusSessions % settings.longBreakEvery === 0
    ? "longBreak"
    : "shortBreak";

  const value = useMemo<FocusTimerValue>(() => ({
    timer, pomodoros, settings, now, remainingMs, elapsedMs, recommendedBreak,
    updateSettings, start, startCustomTimer, resume, pause, reset,
    createPomodoro, togglePomodoro, resetPomodoro, removePomodoro,
  }), [timer, pomodoros, settings, now, remainingMs, elapsedMs, recommendedBreak]);

  return <FocusTimerContext.Provider value={value}>{children}</FocusTimerContext.Provider>;
}

export function useFocusTimer(): FocusTimerValue {
  const value = useContext(FocusTimerContext);
  if (!value) throw new Error("useFocusTimer must be used inside FocusTimerProvider");
  return value;
}
