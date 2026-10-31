import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

interface MediaState {
  title: string;
  artist: string;
  source: string;
  playing: boolean;
  progress_seconds: number;
  duration_seconds: number;
  artwork_url: string | null;
}

interface MediaTimeline {
  position_seconds: number;
  duration_seconds: number;
  playing: boolean;
}

const emptyMedia: MediaState = {
  title: "Nothing playing",
  artist: "Start playback in your browser or media app",
  source: "",
  playing: false,
  progress_seconds: 0,
  duration_seconds: 0,
  artwork_url: null,
};

function time(seconds: number) {
  const safe = Math.max(0, Math.floor(seconds));
  return `${Math.floor(safe / 60)}:${String(safe % 60).padStart(2, "0")}`;
}

function ControlIcon({ kind }: { kind: "previous" | "play" | "pause" | "next" }) {
  if (kind === "play") return <svg viewBox="0 0 24 24" className="h-5 w-5 fill-current"><path d="M8 5.7c0-.8.9-1.3 1.6-.9l9.1 6.3a1.1 1.1 0 0 1 0 1.8l-9.1 6.3a1.1 1.1 0 0 1-1.6-.9V5.7Z" /></svg>;
  if (kind === "pause") return <svg viewBox="0 0 24 24" className="h-5 w-5 fill-current"><path d="M7 5h3.5v14H7zM13.5 5H17v14h-3.5z" /></svg>;
  if (kind === "previous") return <svg viewBox="0 0 24 24" className="h-4 w-4 fill-current"><path d="M6 5h2v14H6zM19 5.8v12.4c0 .8-.9 1.2-1.5.8l-8.8-6.2a1 1 0 0 1 0-1.6l8.8-6.2c.6-.4 1.5 0 1.5.8Z" /></svg>;
  return <svg viewBox="0 0 24 24" className="h-4 w-4 fill-current"><path d="M16 5h2v14h-2zM5 5.8v12.4c0 .8.9 1.2 1.5.8l8.8-6.2a1 1 0 0 0 0-1.6L6.5 5c-.6-.4-1.5 0-1.5.8Z" /></svg>;
}

export function MediaWidget() {
  const [media, setMedia] = useState<MediaState>(emptyMedia);
  // Position/duration/playing, sampled once a second. The bar itself is drawn
  // from this sample plus the elapsed time, so it moves smoothly without
  // hammering the media session.
  const [timeline, setTimeline] = useState<MediaTimeline>({
    position_seconds: 0,
    duration_seconds: 0,
    playing: false,
  });
  const sampledAt = useRef(0);
  const [clock, setClock] = useState(0);
  const [busy, setBusy] = useState(false);

  const refreshInfo = useCallback(async () => {
    try {
      const next = await invoke<MediaState>("get_media_state");
      setMedia(next);
      setTimeline({
        position_seconds: next.progress_seconds,
        duration_seconds: next.duration_seconds,
        playing: next.playing,
      });
      sampledAt.current = performance.now();
    } catch { /* Browser preview or unsupported platform. */ }
  }, []);

  const refreshTimeline = useCallback(async () => {
    try {
      const next = await invoke<MediaTimeline>("get_media_timeline");
      sampledAt.current = performance.now();
      setTimeline((prev) =>
        prev.position_seconds === next.position_seconds &&
        prev.duration_seconds === next.duration_seconds &&
        prev.playing === next.playing
          ? prev
          : next,
      );
    } catch { /* No active session right now. */ }
  }, []);

  useEffect(() => {
    void refreshInfo();
    void refreshTimeline();
    const info = window.setInterval(() => void refreshInfo(), 4000);
    const position = window.setInterval(() => void refreshTimeline(), 1000);
    return () => {
      window.clearInterval(info);
      window.clearInterval(position);
    };
  }, [refreshInfo, refreshTimeline]);

  // Between samples, advance the bar locally while playback is running.
  useEffect(() => {
    if (!timeline.playing) return;
    const id = window.setInterval(() => setClock((value) => value + 1), 250);
    return () => window.clearInterval(id);
  }, [timeline.playing]);

  const act = async (action: "previous" | "play_pause" | "next") => {
    if (busy) return;
    setBusy(true);
    try {
      await invoke("media_action", { action });
      window.setTimeout(() => {
        void refreshInfo();
        void refreshTimeline();
      }, 250);
    } catch { /* Playback controls are available in the Tauri app. */ }
    finally { setBusy(false); }
  };

  const duration = Math.max(timeline.duration_seconds, media.duration_seconds);
  const elapsed = timeline.playing
    ? timeline.position_seconds + Math.max(0, (performance.now() - sampledAt.current) / 1000)
    : timeline.position_seconds;
  const position = duration > 0 ? Math.min(duration, Math.max(0, elapsed)) : Math.max(0, elapsed);
  const progress = duration > 0 ? Math.min(100, Math.max(0, position / duration * 100)) : 0;
  // Keep the clock ref read so the timer-driven re-render is not optimised away.
  void clock;

  return (
    <div className="widget-card relative isolate overflow-hidden rounded-[24px] border border-white/[0.09] shadow-[0_18px_50px_-28px_rgba(0,0,0,0.9)]">
      {media.artwork_url && <div className="absolute inset-0 -z-10 scale-110 bg-cover bg-center opacity-25 blur-2xl" style={{ backgroundImage: `url("${media.artwork_url}")` }} />}
      <div className="absolute inset-0 -z-10 bg-gradient-to-br from-white/[0.1] via-[color:var(--bg-card)] to-[color:var(--bg-main)]" />

      <div className="flex items-center gap-2 px-4 pt-3">
        <span className={`h-1.5 w-1.5 rounded-full ${media.playing ? "animate-pulse bg-emerald-300 shadow-[0_0_9px_rgba(110,231,183,0.8)]" : "bg-white/25"}`} />
        <span className="text-[9px] font-semibold uppercase tracking-[0.2em] text-[var(--text-muted)]">Now playing</span>
        {media.source && <span className="ml-auto max-w-32 truncate rounded-full border border-white/10 bg-black/20 px-2 py-0.5 text-[9px] text-white/45">{media.source}</span>}
      </div>

      <div className="flex gap-4 px-4 pb-3 pt-3">
        <div className="relative h-[118px] w-[118px] shrink-0 overflow-hidden rounded-[18px] bg-gradient-to-br from-violet-500/35 via-fuchsia-400/15 to-cyan-300/20 shadow-[0_10px_30px_-12px_rgba(0,0,0,0.8)] ring-1 ring-white/10">
          {media.artwork_url ? (
            <img src={media.artwork_url} alt={`${media.title} album artwork`} className="h-full w-full object-cover" />
          ) : (
            <div className="grid h-full w-full place-items-center text-white/70">
              <svg viewBox="0 0 64 64" className="h-12 w-12 fill-current opacity-75"><path d="M42 8v35.5a10.5 10.5 0 1 1-4-8.2V19.7l-20 4.8v23a10.5 10.5 0 1 1-4-8.2V17.2L42 11V8Z" /></svg>
            </div>
          )}
          {media.playing && <div className="absolute bottom-2 right-2 flex h-6 items-end gap-[3px] rounded-full bg-black/35 px-2 py-1 backdrop-blur-sm" aria-label="Playing"><i className="h-2 w-[3px] animate-pulse rounded-full bg-white/90" /><i className="h-3 w-[3px] animate-pulse rounded-full bg-white/80 [animation-delay:120ms]" /><i className="h-1.5 w-[3px] animate-pulse rounded-full bg-white/70 [animation-delay:240ms]" /></div>}
        </div>

        <div className="flex min-w-0 flex-1 flex-col justify-center">
          <div className="line-clamp-2 text-[17px] font-semibold leading-tight tracking-tight text-[var(--text-main)]">{media.title}</div>
          <div className="mt-1.5 truncate text-[11px] text-[var(--text-muted)]">{media.artist || "Unknown artist"}</div>
          <div className="mt-auto pt-4">
            <div className="h-[3px] overflow-hidden rounded-full bg-white/15">
              <div className="h-full rounded-full bg-gradient-to-r from-fuchsia-300 to-cyan-200 transition-[width] duration-300 ease-linear" style={{ width: `${progress}%` }} />
            </div>
            <div className="mt-1 flex justify-between font-mono text-[9px] tabular-nums text-[var(--text-muted)]">
              <span>{time(position)}</span><span>{time(duration)}</span>
            </div>
          </div>
        </div>
      </div>

      <div className="flex items-center justify-center gap-7 border-t border-white/[0.07] bg-black/15 px-4 py-2.5">
        <button type="button" onClick={() => void act("previous")} disabled={busy || !media.source} aria-label="Previous track" className="grid h-9 w-9 place-items-center rounded-full text-white/65 transition hover:bg-white/10 hover:text-white disabled:opacity-30"><ControlIcon kind="previous" /></button>
        <button type="button" onClick={() => void act("play_pause")} disabled={busy || !media.source} aria-label={media.playing ? "Pause" : "Play"} className="grid h-11 w-11 place-items-center rounded-full bg-white text-[#141419] shadow-[0_4px_18px_-5px_rgba(255,255,255,0.45)] transition hover:scale-105 hover:bg-white/90 disabled:opacity-40"><ControlIcon kind={media.playing ? "pause" : "play"} /></button>
        <button type="button" onClick={() => void act("next")} disabled={busy || !media.source} aria-label="Next track" className="grid h-9 w-9 place-items-center rounded-full text-white/65 transition hover:bg-white/10 hover:text-white disabled:opacity-30"><ControlIcon kind="next" /></button>
      </div>
      {!media.source && <div className="px-4 pb-3 text-center text-[9px] text-[var(--text-muted)]">Supports apps and browser tabs that publish to Windows media controls.</div>}
    </div>
  );
}
