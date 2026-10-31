# Pill — developer notes

> **User documentation, full feature list, shortcuts and license: [../README.md](../README.md).**
> This file keeps the app-workspace architecture notes.

Pill (formerly "Dynamic Island") is a Windows desktop island built with Tauri v2, Rust, React 19, TypeScript, Tailwind and Framer Motion. It docks a morphing pill to the top-center of the primary monitor and expands into a Fluent-style glass panel with six utility widgets.

## Widgets

| Widget | Behaviour |
| --- | --- |
| Clipboard | Background watcher captures text and images into SQLite with full content. Pick a text or image format at the top, select one or many clips, then drag them out — each clip becomes its own file. Double-click a clip to edit it in a separate window; saving creates a new clip and leaves the original intact. Re-copying the same payload bumps the existing row instead of duplicating it. |
| File Shelf | Universal drop inbox: files, folders, images, text, URLs and HTML (embedded images extracted to assets). Real Explorer thumbnails/icons, pinning, retention-based auto-clear, drag-out, save-as, open. |
| Tasks | SQLite-backed tasks with today's progress counter, quick add, toggle, and delete. |
| Media | Windows SMTC session: artwork, title/artist, previous / play-pause / next, progress. Refreshes every 3 s. |
| Focus | Pomodoro, stopwatch and custom timers with a session dial, session dots, break recommendations, alarms, plus a live chip in the collapsed bar and a minified pill. |
| Search | Everything lookups (`es.exe` or its HTTP server) with live results that can be dragged into any app, plus web search engines (built-in and custom) opened in your chosen browser. |

## Clipboard workflow

1. Copy text or an image anywhere in Windows — it appears in the clip list.
2. Choose a format in the selector at the top: `txt / md / html`, separator, then `png / jpg / webp`.
3. Click clips to select them (or `All`), then **drag any selected clip** into Explorer, Slack, VS Code, or a browser. One file is written per selected clip.
4. Dragging a clip that is *not* part of a selection exports only that clip using your **default** format from Settings.
5. **Double-click** a clip to open the editor window. `Save as new clip` writes a new entry and leaves the original untouched.

## Tray menu

`Show Pill` · `Settings...` · `Toggle Clipboard Capture` · `Minified Pill` · `Restart Application` · `Quit`

`Settings...` opens a dedicated window for the default text/image export formats and the clipboard capture switch.

## Architecture

```
src-tauri/
├─ src/
│  ├─ main.rs            # binary entry
│  ├─ lib.rs             # builder, plugins, IPC registration
│  ├─ state.rs           # AppState (SQLite + shelf), schema init
│  ├─ settings.rs        # default export formats (SQLite-backed)
│  ├─ clipboard/mod.rs   # watcher, capture, multi-file export
│  ├─ shelf/mod.rs       # shelf, thumbnails, open-with-default-app
│  ├─ tray.rs            # tray icon + menu actions
│  ├─ todo/mod.rs        # task CRUD
│  ├─ media/mod.rs       # SMTC wrapper (placeholder)
│  ├─ timer/mod.rs       # pomodoro state helpers
│  └─ utils/mod.rs       # top-center positioning, drag preview icon
```

```
src/
├─ components/           # IslandShell, header, tab carousel
├─ widgets/              # one component per module
├─ lib/ipc.ts            # typed invoke wrappers + drag-out helper
└─ types.ts              # shared payload types
```

The SQLite database lives at `%TEMP%/dynamic-island.db3` with `clipboard_items`, `shelf_items`, `shelf_history`, `todo_items` and `settings` tables. The internal `%TEMP%` names still use the old project name; they are storage keys, not branding.

## Prerequisites

- Node.js 20+
- Rust (stable, MSVC toolchain)
- Microsoft Visual Studio Build Tools with the **Desktop development with C++** workload
- WebView2 runtime (preinstalled on Windows 11)

## Run

```bash
npm install
npm run tauri dev
```

Production build:

```bash
npm run tauri build
```

## Frontend-only check

```bash
npm run build
```

