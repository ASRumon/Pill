import { useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { ipc, formatBytes } from "../lib/ipc";
import { BACKGROUND_OPTIONS } from "../lib/appearance";
import { allEngines } from "../lib/searchEngines";
import { WindowControls } from "./WindowChrome";
import type { AppSettings, SearchStatus, StorageUsage, UpdateChannel } from "../types";

const TEXT_FORMATS = [
  "txt",
  "md",
  "html",
  "json",
  "csv",
  "xml",
  "yml",
  "log",
  "ini",
  "css",
  "js",
  "ts",
  "py",
  "sql",
  "bat",
  "ps1",
  "sh",
];
const IMAGE_FORMATS = ["png", "jpg", "webp"];
const RETENTION_OPTIONS = [
  { value: "1", label: "1 hour" },
  { value: "24", label: "24 hours" },
  { value: "168", label: "7 days" },
  { value: "0", label: "Never" },
];
const SHORTCUT_OPTIONS = [
  { value: "ctrl+alt", label: "Ctrl + Alt" },
  { value: "ctrl+shift", label: "Ctrl + Shift" },
  { value: "alt+shift", label: "Alt + Shift" },
  { value: "ctrl+alt+shift", label: "Ctrl + Alt + Shift" },
  { value: "off", label: "Off" },
];

async function closeWindow() {
  try {
    await getCurrentWindow().close();
  } catch {
    window.close();
  }
}

function PillRow({
  value,
  options,
  onChange,
}: {
  value: string;
  options: string[];
  onChange: (next: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {options.map((option) => (
        <button
          key={option}
          type="button"
          onClick={() => onChange(option)}
          className={`rounded-full px-3 py-1 text-xs capitalize transition ${
            value === option
              ? "bg-cyan-400/25 text-cyan-100 ring-1 ring-cyan-400/40"
              : "bg-white/6 text-white/50 hover:bg-white/10 hover:text-white/80"
          }`}
        >
          .{option}
        </button>
      ))}
    </div>
  );
}

function ChoiceRow({
  value,
  options,
  onChange,
}: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (next: string) => void;
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          onClick={() => onChange(option.value)}
          className={`rounded-full px-3 py-1 text-xs transition ${
            value === option.value
              ? "bg-cyan-400/25 text-cyan-100 ring-1 ring-cyan-400/40"
              : "bg-white/6 text-white/50 hover:bg-white/10 hover:text-white/80"
          }`}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

function SliderRow({
  label,
  value,
  min,
  max,
  suffix,
  onChange,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  suffix: string;
  onChange: (next: number) => void;
}) {
  return (
    <label className="block">
      <span className="mb-1 flex items-center justify-between text-[11px] text-white/50">
        <span>{label}</span>
        <span className="font-medium tabular-nums text-white/70">
          {value}
          {suffix}
        </span>
      </span>
      <input
        type="range"
        min={min}
        max={max}
        step={1}
        value={value}
        onChange={(event) => onChange(Number(event.target.value))}
        aria-label={label}
        className="h-1.5 w-full cursor-pointer appearance-none rounded-full bg-white/10 accent-cyan-400"
      />
    </label>
  );
}

export function SettingsWindow() {
  const [settings, setSettings] = useState<AppSettings>({
    default_text_format: "txt",
    default_image_format: "png",
    minified_island: false,
    shelf_retention_hours: 24,
    click_through_shortcut: "ctrl+alt",
    search_engine: "google",
    search_engines: [],
    search_browser: "default",
    search_everything_port: 80,
    search_es_path: "",
    island_background: "amoled",
    island_opacity: 92,
    island_blur: 24,
  });
  const [capturing, setCapturing] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [status, setStatus] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [searchStatus, setSearchStatus] = useState<SearchStatus | null>(null);
  const [storage, setStorage] = useState<StorageUsage | null>(null);
  const [storageBusy, setStorageBusy] = useState(false);
  const [confirmClear, setConfirmClear] = useState(false);
  const [channel, setChannel] = useState<UpdateChannel | null>(null);
  const [updating, setUpdating] = useState(false);
  const [draftEngine, setDraftEngine] = useState({ name: "", url: "" });

  useEffect(() => {
    ipc
      .getSettings()
      .then(setSettings)
      .catch((e) => setError(String(e)));

    ipc
      .clipboardWatchEnabled()
      .then(setCapturing)
      .catch(() => undefined);

    ipc
      .getAutostart()
      .then(setAutostart)
      .catch(() => undefined);

    ipc
      .updateChannel()
      .then(setChannel)
      .catch(() => undefined);

    void refreshSearchStatus();
    void refreshStorage();
  }, []);

  async function refreshSearchStatus() {
    try {
      setSearchStatus(await ipc.searchStatus());
    } catch {
      setSearchStatus(null);
    }
  }

  async function refreshStorage() {
    try {
      setStorage(await ipc.storageUsage());
    } catch {
      setStorage(null);
    }
  }

  async function clearStorage() {
    setConfirmClear(false);
    setStorageBusy(true);
    try {
      setStorage(await ipc.clearStorage());
      setStatus("Storage cleared");
      window.setTimeout(() => setStatus(""), 1800);
    } catch (e) {
      setError(String(e));
    } finally {
      setStorageBusy(false);
    }
  }

  /**
   * Store copies are updated by Windows, so the only useful action there is the
   * product page. Direct copies run the usual GitHub check and open whatever
   * link the backend attached to the notice.
   */
  async function checkUpdates() {
    setUpdating(true);
    try {
      if (channel?.store_managed) {
        await ipc.openExternal(channel.channel_url, null);
        return;
      }
      const info = await ipc.checkForUpdate();
      if (!info) {
        setStatus("Pill is up to date");
        window.setTimeout(() => setStatus(""), 1800);
        return;
      }
      await ipc.openExternal(info.action_url, null);
      setStatus(`Pill ${info.latest_version} is ready`);
      window.setTimeout(() => setStatus(""), 1800);
    } catch (e) {
      setError(String(e));
    } finally {
      setUpdating(false);
    }
  }

  const engines = allEngines(settings.search_engines);

  const addEngine = () => {
    const name = draftEngine.name.trim();
    const url = draftEngine.url.trim();
    if (!name || !url.startsWith("http") || !url.includes("{query}")) {
      setError("Custom engines need a name and an http(s) URL containing {query}.");
      return;
    }
    const id = `custom-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}`;
    if (engines.some((engine) => engine.id === id)) {
      setError("An engine with that name already exists.");
      return;
    }
    setSettings((prev) => ({
      ...prev,
      search_engines: [...prev.search_engines, { id, name, url }],
    }));
    setDraftEngine({ name: "", url: "" });
    setError(null);
  };

  const removeEngine = (id: string) => {
    setSettings((prev) => ({
      ...prev,
      search_engines: prev.search_engines.filter((engine) => engine.id !== id),
      search_engine: prev.search_engine === id ? "google" : prev.search_engine,
    }));
  };

  const pickEsExecutable = async () => {
    try {
      const picked = await openDialog({
        multiple: false,
        directory: false,
        title: "Select es.exe (Everything command line tool)",
        filters: [{ name: "Executable", extensions: ["exe"] }],
      });
      if (typeof picked === "string") {
        setSettings((prev) => ({ ...prev, search_es_path: picked }));
      }
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleCapture = async () => {
    try {
      setCapturing(await ipc.setClipboardWatch(!capturing));
    } catch (e) {
      setError(String(e));
    }
  };

  const toggleAutostart = async () => {
    try {
      setAutostart(await ipc.setAutostart(!autostart));
    } catch (e) {
      setError(String(e));
    }
  };

  const save = async () => {
    try {
      setSettings(await ipc.setSettings(settings));
      setStatus("Saved");
      void refreshSearchStatus();
      window.setTimeout(() => setStatus(""), 1600);
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="flex h-screen w-screen flex-col overflow-hidden rounded-xl border border-white/10 bg-[#0d0d10] text-[13px] text-white">
      <header
        data-tauri-drag-region
        className="flex items-start justify-between gap-3 border-b border-white/10 px-4 py-3"
      >
        <div className="min-w-0" data-tauri-drag-region>
          <h1 className="text-sm font-semibold" data-tauri-drag-region>
            Pill Settings
          </h1>
          <p className="text-[11px] text-white/40" data-tauri-drag-region>
            Applies to every drag-out from the clipboard widget.
          </p>
        </div>
        <WindowControls />
      </header>

      <main className="min-h-0 flex-1 space-y-4 overflow-y-auto p-4">
        {error && (
          <div className="rounded-lg border border-rose-400/30 bg-rose-400/10 px-3 py-2 text-xs text-rose-200">
            {error}
          </div>
        )}

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Island background</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Glass keeps the frosted look. AMOLED paints the island pure black for OLED screens.
            Custom lets you set the transparency and blur yourself.
          </p>
          <ChoiceRow
            value={settings.island_background}
            options={BACKGROUND_OPTIONS}
            onChange={(next) =>
              setSettings((prev) => ({ ...prev, island_background: next }))
            }
          />

          {settings.island_background === "custom" && (
            <div className="mt-3 space-y-2.5 border-t border-white/10 pt-3">
              <SliderRow
                label="Opacity"
                value={settings.island_opacity}
                min={10}
                max={100}
                suffix="%"
                onChange={(next) =>
                  setSettings((prev) => ({ ...prev, island_opacity: next }))
                }
              />
              <SliderRow
                label="Blur"
                value={settings.island_blur}
                min={0}
                max={40}
                suffix="px"
                onChange={(next) =>
                  setSettings((prev) => ({ ...prev, island_blur: next }))
                }
              />
            </div>
          )}

          <p className="mt-2 text-[10px] text-white/25">Applies to the island when you save.</p>
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Default text format</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Used when you drag a single clip without picking a format.
          </p>
          <PillRow
            value={settings.default_text_format}
            options={TEXT_FORMATS}
            onChange={(next) =>
              setSettings((prev) => ({ ...prev, default_text_format: next }))
            }
          />
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Default image format</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Used when you drag a copied image clip.
          </p>
          <PillRow
            value={settings.default_image_format}
            options={IMAGE_FORMATS}
            onChange={(next) =>
              setSettings((prev) => ({ ...prev, default_image_format: next }))
            }
          />
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Shelf auto-clear</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Unpinned items leave the shelf after this. Pinned items stay, and the drop history is
            always kept.
          </p>
          <ChoiceRow
            value={String(settings.shelf_retention_hours)}
            options={RETENTION_OPTIONS}
            onChange={(next) =>
              setSettings((prev) => ({ ...prev, shelf_retention_hours: Number(next) }))
            }
          />
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Click-through shortcut</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Hold these keys and the island ignores the mouse, so you can click whatever sits
            behind it. Pick a combination your other apps do not use.
          </p>
          <ChoiceRow
            value={settings.click_through_shortcut}
            options={SHORTCUT_OPTIONS}
            onChange={(next) =>
              setSettings((prev) => ({ ...prev, click_through_shortcut: next }))
            }
          />
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Search</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Pressing Enter in the Search widget opens the term in your browser. Files come from
            Everything when it is reachable.
          </p>

          <div className="mb-2 flex flex-wrap gap-1.5">
            {engines.map((engine) => (
              <button
                key={engine.id}
                type="button"
                onClick={() =>
                  setSettings((prev) => ({ ...prev, search_engine: engine.id }))
                }
                className={`rounded-full px-3 py-1 text-xs transition ${
                  settings.search_engine === engine.id
                    ? "bg-cyan-400/25 text-cyan-100 ring-1 ring-cyan-400/40"
                    : "bg-white/6 text-white/50 hover:bg-white/10 hover:text-white/80"
                }`}
              >
                {engine.name}
              </button>
            ))}
          </div>

          <div className="mb-2 space-y-1">
            {settings.search_engines.map((engine) => (
              <div
                key={engine.id}
                className="flex items-center gap-2 rounded-lg bg-black/20 px-2 py-1.5 text-[11px]"
              >
                <span className="shrink-0 font-medium text-white/75">{engine.name}</span>
                <span className="min-w-0 flex-1 truncate text-white/30">{engine.url}</span>
                <button
                  type="button"
                  onClick={() => removeEngine(engine.id)}
                  className="shrink-0 rounded px-1.5 text-[11px] text-rose-300/80 transition hover:bg-rose-400/10"
                >
                  Remove
                </button>
              </div>
            ))}
          </div>

          <div className="flex flex-wrap items-center gap-1.5">
            <input
              value={draftEngine.name}
              onChange={(event) =>
                setDraftEngine((prev) => ({ ...prev, name: event.target.value }))
              }
              placeholder="Name (e.g. GitHub)"
              className="w-28 rounded-lg border border-white/10 bg-black/25 px-2 py-1.5 text-[11px] text-white outline-none focus:border-cyan-300/50"
            />
            <input
              value={draftEngine.url}
              onChange={(event) =>
                setDraftEngine((prev) => ({ ...prev, url: event.target.value }))
              }
              onKeyDown={(event) => {
                if (event.key === "Enter") addEngine();
              }}
              placeholder="https://example.com/?q={query}"
              className="min-w-0 flex-1 rounded-lg border border-white/10 bg-black/25 px-2 py-1.5 text-[11px] text-white outline-none focus:border-cyan-300/50"
            />
            <button
              type="button"
              onClick={addEngine}
              className="rounded-lg bg-white/10 px-2.5 py-1.5 text-[11px] text-white/80 transition hover:bg-white/15"
            >
              Add engine
            </button>
          </div>
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <h2 className="mb-1 text-xs font-semibold text-white/80">Browser</h2>
          <p className="mb-2 text-[11px] text-white/35">
            Search links are handed to this browser. They never load inside the island.
          </p>
          <ChoiceRow
            value={settings.search_browser}
            options={(searchStatus?.browsers ?? [{ id: "default", name: "System default" }]).map(
              (browser) => ({ value: browser.id, label: browser.name }),
            )}
            onChange={(next) =>
              setSettings((prev) => ({ ...prev, search_browser: next }))
            }
          />
          <div className="mt-2 text-[10px] text-white/30">
            {settings.search_browser !== "default" && (
              <span className="block truncate">{settings.search_browser}</span>
            )}
          </div>
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <div className="mb-1 flex items-center justify-between gap-2">
            <h2 className="text-xs font-semibold text-white/80">Everything (file search)</h2>
            <span
              className={`shrink-0 rounded-full px-2 py-0.5 text-[9px] font-medium uppercase tracking-[0.08em] ${
                searchStatus?.everything
                  ? "bg-emerald-400/15 text-emerald-200/90"
                  : "bg-white/6 text-white/40"
              }`}
            >
              {searchStatus?.everything ? "Detected" : "Not found"}
            </span>
          </div>
          <p className="mb-2 text-[11px] text-white/35">
            {searchStatus?.everything
              ? searchStatus.everything_detail
              : searchStatus?.everything_detail ||
                "Enable Tools → Options → HTTP Server in Everything, or select es.exe below."}
          </p>
          <div className="flex flex-wrap items-center gap-1.5">
            <label className="flex items-center gap-1.5 rounded-lg bg-black/20 px-2 py-1.5 text-[11px] text-white/60">
              HTTP port
              <input
                type="number"
                min={1}
                max={65535}
                value={settings.search_everything_port}
                onChange={(event) =>
                  setSettings((prev) => ({
                    ...prev,
                    search_everything_port: Number(event.target.value) || 80,
                  }))
                }
                className="w-16 rounded-md border border-white/10 bg-black/30 px-1.5 py-0.5 text-[11px] text-white outline-none focus:border-cyan-300/50"
              />
            </label>
            <button
              type="button"
              onClick={() => void pickEsExecutable()}
              className="rounded-lg bg-white/10 px-2.5 py-1.5 text-[11px] text-white/80 transition hover:bg-white/15"
            >
              Select es.exe
            </button>
            <button
              type="button"
              onClick={() => void refreshSearchStatus()}
              className="rounded-lg bg-white/10 px-2.5 py-1.5 text-[11px] text-white/80 transition hover:bg-white/15"
            >
              Recheck
            </button>
            <span className="min-w-0 flex-1 truncate text-[10px] text-white/30">
              {settings.search_es_path || "es.exe: auto-detect"}
            </span>
          </div>
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <div className="mb-1 flex items-center justify-between gap-2">
            <h2 className="text-xs font-semibold text-white/80">Storage</h2>
            <button
              type="button"
              onClick={() => void refreshStorage()}
              title="Recalculate"
              className="rounded-md px-2 py-[2px] text-[10px] text-white/45 transition hover:bg-white/10 hover:text-white/80"
            >
              Refresh
            </button>
          </div>
          <p className="mb-2 text-[11px] text-white/35">
            Copied images are cached as PNG copies so they can be dragged out later, and shelf
            attachments are written to disk. Clearing removes history, unpinned items and the
            files behind them; pinned items, folders, tasks and settings stay.
          </p>

          {storage ? (
            <>
              <div className="flex items-baseline gap-1.5">
                <span className="text-[19px] font-semibold leading-none tabular-nums text-white/90">
                  {formatBytes(storage.total_bytes)}
                </span>
                <span className="text-[10px] text-white/35">in use</span>
              </div>

              <div className="mt-2 grid grid-cols-2 gap-1.5">
                {[
                  { label: "Database", value: storage.database_bytes },
                  { label: "Clipboard images", value: storage.clipboard_images_bytes },
                  { label: "Shelf files", value: storage.shelf_files_bytes },
                  { label: "Previews", value: storage.previews_bytes },
                ].map((row) => (
                  <div key={row.label} className="rounded-lg bg-black/20 px-2 py-1.5">
                    <div className="text-[9px] uppercase tracking-[0.08em] text-white/30">
                      {row.label}
                    </div>
                    <div className="text-[11px] tabular-nums text-white/75">
                      {formatBytes(row.value)}
                    </div>
                  </div>
                ))}
              </div>

              <p className="mt-2 text-[10px] text-white/30">
                {storage.clipboard_items} clip{storage.clipboard_items === 1 ? "" : "s"} ·{" "}
                {storage.shelf_items} shelf item{storage.shelf_items === 1 ? "" : "s"} ·{" "}
                {storage.pinned_items} pinned
              </p>

              {storage.database_note && (
                <p className="mt-2 rounded-lg border border-amber-300/25 bg-amber-300/10 px-2 py-1.5 text-[10px] leading-relaxed text-amber-100/80">
                  {storage.database_note}
                </p>
              )}

              <div className="mt-2 flex flex-wrap items-center gap-2">
                {confirmClear ? (
                  <>
                    <button
                      type="button"
                      onClick={() => void clearStorage()}
                      disabled={storageBusy}
                      className="rounded-lg border border-rose-400/40 bg-rose-400/20 px-2.5 py-1.5 text-[11px] text-rose-100 transition hover:bg-rose-400/30 disabled:opacity-40"
                    >
                      {storageBusy ? "Clearing…" : "Yes, clear"}
                    </button>
                    <button
                      type="button"
                      onClick={() => setConfirmClear(false)}
                      className="rounded-lg bg-white/10 px-2.5 py-1.5 text-[11px] text-white/70 transition hover:bg-white/15"
                    >
                      Cancel
                    </button>
                  </>
                ) : (
                  <>
                    <button
                      type="button"
                      onClick={() => setConfirmClear(true)}
                      disabled={storageBusy}
                      className="rounded-lg border border-rose-400/25 bg-rose-400/10 px-2.5 py-1.5 text-[11px] text-rose-200/90 transition hover:border-rose-400/40 hover:bg-rose-400/15 disabled:opacity-40"
                    >
                      Clear history…
                    </button>
                    <span className="text-[10px] text-white/30">Pinned items are kept.</span>
                  </>
                )}
              </div>
            </>
          ) : (
            <span className="text-[10px] text-white/30">Calculating…</span>
          )}
        </section>

        <section className="flex items-center justify-between rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <div>
            <h2 className="text-xs font-semibold text-white/80">Clipboard capture</h2>
            <p className="text-[11px] text-white/35">
              {capturing ? "Watching the clipboard" : "Capture paused"}
            </p>
          </div>
          <button
            type="button"
            onClick={toggleCapture}
            role="switch"
            aria-checked={capturing}
            className={`relative h-6 w-11 shrink-0 rounded-full transition ${
              capturing ? "bg-cyan-400/80" : "bg-white/15"
            }`}
          >
            <span
              className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-all ${
                capturing ? "left-[22px]" : "left-0.5"
              }`}
            />
          </button>
        </section>

        <section className="flex items-center justify-between rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <div className="min-w-0 pr-3">
            <h2 className="text-xs font-semibold text-white/80">Start with Windows</h2>
            <p className="text-[11px] text-white/35">
              {autostart
                ? "Launches automatically when you sign in"
                : "Launch this app myself"}
            </p>
            <p className="mt-1 text-[10px] text-white/25">
              Registers Pill under Task Manager → Startup apps for your account. Runs the
              installed Pill.exe, and repairs the path after an update. No admin needed.
            </p>
          </div>
          <button
            type="button"
            onClick={toggleAutostart}
            role="switch"
            aria-checked={autostart}
            aria-label="Start with Windows"
            className={`relative h-6 w-11 shrink-0 rounded-full transition ${
              autostart ? "bg-cyan-400/80" : "bg-white/15"
            }`}
          >
            <span
              className={`absolute top-0.5 h-5 w-5 rounded-full bg-white transition-all ${
                autostart ? "left-[22px]" : "left-0.5"
              }`}
            />
          </button>
        </section>

        <section className="rounded-xl border border-white/10 bg-white/[0.03] p-3">
          <div className="mb-1 flex items-center justify-between gap-2">
            <h2 className="text-xs font-semibold text-white/80">Updates</h2>
            <span className="text-[10px] tabular-nums text-white/30">
              v{channel?.current_version ?? "…"}
            </span>
          </div>
          <p className="mb-2 text-[11px] leading-relaxed text-white/35">
            {channel?.store_managed
              ? `Installed from the ${channel.label}. Windows installs new versions in the background, so Pill only links to the product page.`
              : "Installed directly. Pill checks GitHub Releases and hands you the new installer; nothing installs on its own."}
          </p>
          <button
            type="button"
            onClick={() => void checkUpdates()}
            disabled={updating}
            className="rounded-lg bg-white/10 px-2.5 py-1.5 text-[11px] text-white/80 transition hover:bg-white/15 disabled:opacity-40"
          >
            {updating
              ? "Checking…"
              : channel?.store_managed
                ? "Open Microsoft Store"
                : "Check for updates"}
          </button>
        </section>
      </main>

      <footer className="flex items-center justify-between border-t border-white/10 px-4 py-3">
        <span className="text-[11px] text-cyan-300/80">{status}</span>
        <div className="flex gap-2">
          <button
            type="button"
            onClick={() => void closeWindow()}
            className="rounded-lg bg-white/8 px-3 py-1.5 text-xs text-white/70 transition hover:bg-white/12"
          >
            Close
          </button>
          <button
            type="button"
            onClick={save}
            className="rounded-lg bg-cyan-400/90 px-3 py-1.5 text-xs font-medium text-black transition hover:bg-cyan-300"
          >
            Save
          </button>
        </div>
      </footer>
    </div>
  );
}
