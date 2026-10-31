import { useEffect, useState } from "react";
import { fileUrl, formatBytes } from "../lib/ipc";
import type { ShelfFile } from "../types";

/** Short "when" label shared by shelf tiles and the history list. */
export function relativeTime(timestamp: number): string {
  const diff = Date.now() - timestamp;
  if (!Number.isFinite(diff) || diff < 0) return "just now";
  const seconds = Math.round(diff / 1000);
  if (seconds < 45) return "just now";
  const minutes = Math.round(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.round(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  return `${Math.round(hours / 24)}d ago`;
}

interface ShelfDetailProps {
  file: ShelfFile;
  onClose: () => void;
  onOpen: () => void;
  onCopy: () => void;
  onSaveAs: () => void;
  onTogglePin: () => void;
  onRemove: () => void;
  onRename: () => void;
  onRefreshMeta?: () => void;
  onRemoveFromFolder?: () => void;
}

const actionClass =
  "rounded-lg border border-white/10 bg-white/[0.04] px-2 py-[5px] text-[10px] text-white/75 transition hover:border-white/20 hover:bg-white/10 hover:text-white";

/** Expanded view for one dropped item. */
export function ShelfDetail({
  file,
  onClose,
  onOpen,
  onCopy,
  onSaveAs,
  onTogglePin,
  onRemove,
  onRename,
  onRefreshMeta,
  onRemoveFromFolder,
}: ShelfDetailProps) {
  const isFolder = file.kind === "folder";
  const isLink = file.kind === "url";
  // Cached previews can be swept by the OS; a broken image falls back to the
  // text/glyph presentation instead of an empty box.
  const [broken, setBroken] = useState({ hero: false, favicon: false, thumb: false });
  useEffect(() => setBroken({ hero: false, favicon: false, thumb: false }), [file.id]);

  return (
    <div
      className="absolute inset-0 z-30 flex flex-col rounded-2xl border border-white/10 bg-[#0d0d11]/97 p-3 shadow-[0_20px_50px_-20px_rgba(0,0,0,0.9)] backdrop-blur-xl"
      onDragStart={(event) => event.preventDefault()}
    >
      <div className="mb-2 flex items-start justify-between gap-2">
        <div className="min-w-0">
          <button
            type="button"
            onClick={onRename}
            title="Rename"
            className="block max-w-full truncate text-left text-[12px] font-medium text-white/90 transition hover:text-cyan-200"
          >
            {file.name}
          </button>
          <p className="truncate text-[9.5px] text-white/40">
            {isFolder
              ? "Folder"
              : isLink
                ? file.meta_site || "link"
                : `${formatBytes(file.size_bytes)} · ${file.extension || "file"}`}{" "}
            · dropped {relativeTime(file.created_at)}
          </p>
        </div>
        <button
          type="button"
          onClick={onClose}
          aria-label="Close details"
          className="shrink-0 rounded-md px-1.5 py-0.5 text-[11px] text-white/50 transition hover:bg-white/10 hover:text-white"
        >
          ✕
        </button>
      </div>

      <div className="grid min-h-0 flex-1 place-items-center overflow-hidden rounded-xl border border-white/10 bg-black/30">
        {isFolder ? (
          <div className="flex flex-col items-center gap-2 px-4 text-center">
            <span className="grid h-11 w-11 place-items-center rounded-xl bg-amber-300/15 text-amber-200">
              <FolderGlyph />
            </span>
            <p className="text-[12px] font-medium text-white/85">{file.name}</p>
            <p className="text-[9.5px] text-white/40">
              Organisational folder. Click the tile to open it, drag it out to copy every file.
            </p>
          </div>
        ) : isLink ? (
          <div className="flex h-full w-full flex-col">
            {(file.meta_image || file.meta_favicon) && !broken.hero && (
              <div className="relative h-1/2 w-full shrink-0 overflow-hidden border-b border-white/10">
                <img
                  src={fileUrl(file.meta_image || file.meta_favicon)}
                  alt=""
                  className="h-full w-full object-cover"
                  onError={() => setBroken((prev) => ({ ...prev, hero: true }))}
                />
              </div>
            )}
            <div className="flex min-h-0 flex-1 flex-col items-center justify-center gap-1.5 px-3 py-2 text-center">
              <span className="flex items-center gap-1.5 text-[9px] uppercase tracking-[0.08em] text-cyan-200/80">
                {file.meta_favicon && !file.meta_image && !broken.favicon && (
                  <img
                    src={fileUrl(file.meta_favicon)}
                    alt=""
                    className="h-3 w-3 rounded"
                    onError={() => setBroken((prev) => ({ ...prev, favicon: true }))}
                  />
                )}
                {file.meta_site || "Link"}
              </span>
              <button
                type="button"
                onClick={onRename}
                className="line-clamp-2 text-[12.5px] font-medium leading-snug text-white/90 transition hover:text-cyan-200"
                title="Rename this link"
              >
                {file.meta_title || file.name}
              </button>
              {file.meta_description && (
                <p className="line-clamp-3 text-[9.5px] leading-relaxed text-white/45">
                  {file.meta_description}
                </p>
              )}
              <p className="w-full truncate text-[9px] text-white/30" title={file.content}>
                {file.content}
              </p>
            </div>
          </div>
        ) : file.kind === "text" || file.kind === "html" ? (
          <div className="h-full w-full overflow-y-auto p-2.5">
            <p className="whitespace-pre-wrap break-words text-[10px] leading-relaxed text-white/70">
              {file.content || file.preview}
            </p>
          </div>
        ) : file.thumb_path && !broken.thumb ? (
          <img
            src={fileUrl(file.thumb_path)}
            alt=""
            onError={() => setBroken((prev) => ({ ...prev, thumb: true }))}
            className={
              file.thumb_kind === "cover"
                ? "h-full w-full object-contain"
                : "max-h-full max-w-full object-contain p-3 drop-shadow-[0_8px_18px_rgba(0,0,0,0.45)]"
            }
          />
        ) : (
          <span className="rounded-lg bg-white/5 px-3 py-2 font-mono text-[11px] uppercase text-white/45">
            {file.extension || "file"}
          </span>
        )}
      </div>

      <p className="mt-2 truncate text-[9px] text-white/25" title={file.path || file.content}>
        {file.path || file.content || `${file.kind} · ${file.mime || "inline"}`}
      </p>

      <div className="mt-2 grid grid-cols-3 gap-1.5">
        <button type="button" onClick={onOpen} className={actionClass}>
          {isLink ? "Open link" : "Open"}
        </button>
        {isFolder ? (
          <button type="button" onClick={onRename} className={actionClass}>
            Rename
          </button>
        ) : (
          <button type="button" onClick={onCopy} className={actionClass}>
            Copy
          </button>
        )}
        {isFolder ? (
          <button type="button" onClick={onTogglePin} className={actionClass}>
            {file.pinned ? "Unpin" : "Pin"}
          </button>
        ) : (
          <button type="button" onClick={onSaveAs} className={actionClass}>
            Save as…
          </button>
        )}

        {!isFolder && (
          <button type="button" onClick={onTogglePin} className={actionClass}>
            {file.pinned ? "Unpin" : "Pin"}
          </button>
        )}
        {isLink && onRefreshMeta && (
          <button type="button" onClick={onRefreshMeta} className={actionClass}>
            Refresh preview
          </button>
        )}
        {onRemoveFromFolder && (
          <button type="button" onClick={onRemoveFromFolder} className={actionClass}>
            Remove from folder
          </button>
        )}
        <button
          type="button"
          onClick={onRemove}
          className="col-span-3 rounded-lg border border-rose-400/20 bg-rose-400/10 px-2 py-[5px] text-[10px] text-rose-200/90 transition hover:border-rose-400/40 hover:bg-rose-400/15"
        >
          {isFolder ? "Delete folder" : "Remove from shelf"}
        </button>
      </div>
    </div>
  );
}

function FolderGlyph() {
  return (
    <svg viewBox="0 0 24 24" className="h-5 w-5 fill-current" aria-hidden="true">
      <path d="M3 6.5A2.5 2.5 0 0 1 5.5 4h3.2c.7 0 1.3.3 1.8.8l1 1.2h7A2.5 2.5 0 0 1 21 8.5v8A2.5 2.5 0 0 1 18.5 19h-13A2.5 2.5 0 0 1 3 16.5v-10Z" />
    </svg>
  );
}
