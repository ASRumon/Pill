import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { fileUrl, ipc } from "../lib/ipc";
import { WindowControls } from "./WindowChrome";
import type { ClipboardEntry } from "../types";

async function closeWindow() {
  try {
    await getCurrentWindow().close();
  } catch {
    window.close();
  }
}

export function EditorWindow({ id }: { id: number }) {
  const [entry, setEntry] = useState<ClipboardEntry | null>(null);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    ipc
      .getClip(id)
      .then((clip) => {
        setEntry(clip);
        setDraft(clip.content);
      })
      .catch((e) => setError(String(e)));
  }, [id]);

  const isImage = entry?.kind === "image";
  const dirty = Boolean(entry) && draft !== entry?.content;

  const save = async () => {
    if (!entry || isImage) return;
    setSaving(true);
    try {
      await ipc.updateClip(id, draft);
      await closeWindow();
    } catch (e) {
      setError(String(e));
      setSaving(false);
    }
  };

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden rounded-xl border border-white/10 bg-[#0d0d10] text-[13px] text-white">
      <header
        data-tauri-drag-region
        className="flex items-center justify-between gap-3 border-b border-white/10 px-4 py-3"
      >
        <div className="min-w-0" data-tauri-drag-region>
          <h1 className="text-sm font-semibold" data-tauri-drag-region>
            Edit clip
          </h1>
          <p className="truncate text-[11px] text-white/40" data-tauri-drag-region>
            {entry ? `${entry.kind} · ${entry.created_at}` : "Loading..."}
          </p>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className="rounded-full bg-cyan-400/10 px-2 py-1 text-[10px] text-cyan-300">
            saving creates a new clip
          </span>
          <WindowControls />
        </div>
      </header>

      <main className="min-h-0 flex-1 p-4">
        {error && (
          <div className="rounded-lg border border-rose-400/30 bg-rose-400/10 px-3 py-2 text-xs text-rose-200">
            {error}
          </div>
        )}

        {!error && !entry && (
          <div className="text-xs text-white/40">Loading clip...</div>
        )}

        {entry && isImage && (
          <div className="flex h-full flex-col items-center justify-center gap-3">
            <img
              src={fileUrl(entry.image_path)}
              alt={entry.preview}
              className="max-h-[70%] max-w-full rounded-lg border border-white/10 object-contain"
            />
            <p className="text-[11px] text-white/40">
              Image clips are read-only. Convert them by dragging with an image format selected.
            </p>
          </div>
        )}

        {entry && !isImage && (
          <textarea
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            spellCheck={false}
            className="h-full w-full resize-none rounded-xl border border-white/10 bg-black/40 p-3 font-mono text-[12.5px] leading-relaxed text-white/90 outline-none focus:border-cyan-400/40"
          />
        )}
      </main>

      <footer className="flex items-center justify-between border-t border-white/10 px-4 py-3">
        <span className="text-[11px] text-white/35">
          {dirty ? "Unsaved changes" : "No changes"}
        </span>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void closeWindow()}
            className="rounded-lg bg-white/8 px-3 py-1.5 text-xs text-white/70 transition hover:bg-white/12"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={!dirty || saving || isImage}
            onClick={save}
            className="rounded-lg bg-cyan-400/90 px-3 py-1.5 text-xs font-medium text-black transition hover:bg-cyan-300 disabled:cursor-default disabled:opacity-35"
          >
            {saving ? "Saving..." : "Save as new clip"}
          </button>
        </div>
      </footer>
    </div>
  );
}
