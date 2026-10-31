# Pill — build & architecture notes

> Public overview, screenshots and install instructions: [README.md](README.md).
> App-workspace notes: [dynamic-island/README.md](dynamic-island/README.md).

**A floating dynamic island for Windows.** Pill lives at the top of your screen as a small, always-on-top pill. Hover it and it opens into a compact panel with everything you were about to switch windows for: your clipboard history, files you dragged in, tasks, media controls, focus sessions, and search.

<p align="left">
  <b>Version</b> 2.0.0 &nbsp;·&nbsp; <b>Platform</b> Windows 10 / 11 (x64) &nbsp;·&nbsp; <b>License</b> Closed source, free to use
</p>

Author: **Asaduzzaman Rumon** — [info.asrumon.workers.dev](https://info.asrumon.workers.dev)

---

## Table of contents

- [Requirements](#requirements)
- [Install](#install)
- [First run](#first-run)
- [The island](#the-island)
- [Widgets](#widgets)
- [Keyboard & mouse reference](#keyboard--mouse-reference)
- [Drag and drop](#drag-and-drop)
- [Tray menu](#tray-menu)
- [Settings](#settings)
- [File search with Everything](#file-search-with-everything)
- [Data, privacy and storage](#data-privacy-and-storage)
- [Troubleshooting](#troubleshooting)
- [Build from source](#build-from-source)
- [License](#license)

---

## Requirements

| | |
|---|---|
| OS | Windows 10 (2004+) or Windows 11, 64-bit |
| Runtime | Microsoft Edge WebView2 (preinstalled on Windows 11; on Windows 10 the installer provides it if missing) |
| Disk | ~15 MB installed |
| Optional | [Everything](https://www.voidtools.com/) by voidtools — only needed for local file search |
| Not required | No admin rights, no account, no network connection for core features |

Pill is a per-user install. It never asks for elevation.

## Install

1. Run `Pill_<version>_x64-setup.exe`.
2. Pick a location (or keep the default per-user folder) and finish.
3. Pill starts immediately and adds a tray icon.

> Upgrading from a pre-release build named *dynamic-island*? Uninstall that build first — Pill uses its own application identity, so Windows treats it as a separate product.

## First run

- The pill appears **top-centre of your primary monitor**, collapsed.
- **Hover it** to open the panel. **Move the cursor away** and it collapses again.
- Use the tab strip to switch between widgets, or press **←** / **→**.
- Right-click any item for a context menu (copy, save as, edit, pin, delete).
- Open **Pill Settings** from the tray menu.

---

## The island

The island is a single fixed native window whose visible shape is driven by an animated region, so it can never jump or re-centre while it grows.

| State | Size | What you see |
|---|---|---|
| Collapsed | 246 × 36 px | Quick-launch icons for every widget + local time |
| Collapsed, session running | 288 × 36 px | Live timer chip replaces the clock (see [Focus](#focus)) |
| Minified | 92 × 24 px | A thin pill, just the island outline |
| Minified, session running | 170 × 34 px | Mode icon, label and countdown with a progress line |
| Expanded | 540 × 430 px | The full panel with header, tabs and the active widget |

**Collapse behaviour** is pointer-driven: it closes ~200 ms after the cursor leaves, and it will never collapse out from under the cursor or while a drop is in progress.

**Header** (expanded) shows a live clock, the date (`Thu 30 Oct 2026`) and seven weekday dots with today highlighted.

---

## Widgets

### Clipboard

Watches the Windows clipboard and keeps a searchable-free, chronological history of text and images.

- **Automatic capture** of text and images, with a pause switch (Settings, tray, or the widget header).
- **Duplicate-safe:** copying the same text or image again moves the existing entry to the top instead of adding a copy.
- **Multi-select:** enable multi-select in the header, or `Ctrl`/`Cmd`-click rows; `Shift`-click selects a range.
- **Drag out:** drag any row into Explorer, Slack, VS Code, a chat app or a browser upload field. Images drag as real files.
- **Convert and export:** the format strip under the search field converts the current selection and drops it out as that format. Text formats: `txt, md, html, json, csv, xml, yml, log, ini, css, js, ts, py, sql, bat, ps1, sh`. Image formats: `png, jpg, webp`.
- **Instant delete**, clear-all, and an **Edit** dialog that opens the clip in its own window and saves the result as a new clip.
- **Context menu:** Copy, Save as…, Edit, Delete.
- Layout switch (1 / 2 / 3 columns) in the panel toolbar.

### File Shelf (Drop Inbox)

A universal inbox for anything you drag onto the island.

- Accepts **files and folders, images, plain text, URLs and HTML** — including HTML with embedded images, which are extracted and stored as real assets.
- Files are **referenced in place**, never copied, so the shelf stays instant even for huge items.
- **Thumbnails** are taken from Explorer (real thumbnails first, then file-type icons).
- **Pin** items so they survive clears and never expire. Pinned items sort above the rest.
- **Auto-clear:** unpinned items leave the shelf after 1 hour, 24 hours or 7 days (or never). The drop history is kept separately.
- **Per-item actions:** Open, Copy file(s) to clipboard, Save as…, Pin/Unpin, Remove.
- **Drag out** to any app — inline text/HTML/image items are materialised into real files first.
- Multiple files can be dropped at once.

### Tasks

A lightweight checklist for the day.

- Quick add (Enter), tick off, delete.
- Progress counter and bar (`3/5 Tasks Done Today`).
- Optional 1 / 2 / 3 column layout.

### Media

Mirrors the Windows system media session (SMTC) — whatever is playing in Spotify, a browser, or any other app that publishes a media session.

- Artwork-tinted background, title, artist, source app.
- **Previous / Play-Pause / Next** controls.
- Progress bar and elapsed/total time. Refreshes every 3 seconds.

### Focus

Pomodoro-style sessions, a stopwatch, and custom timers.

- Modes: **Focus**, **Short break**, **Long break**, **Stopwatch**, and a **custom timer** (minutes + seconds).
- `Session N of M` with session dots; long break is recommended after every *N* focus sessions (configurable).
- Options panel: focus/break lengths, long-break interval, session alarm, stopwatch alarm.
- Optional audible alarm and desktop notification when a session ends.
- The running session is visible **outside** the panel: as a chip in the collapsed bar and as the minified pill. Click the chip to jump straight to Focus.

### Search

Two kinds of search in one box.

**Local file search (Everything)** — when Everything is reachable:

- Type two or more characters; results stream in live (name, folder, size, type) with a short debounce.
- Click a result to open it. **Drag a result into any app** to move/copy the real file — it is a genuine file drag, not a preview.
- Read-only: Pill never modifies or deletes files during a search.

**Web search** — always available:

- Press **Enter** or the **Web** button to open the current term in your browser.
- Engines: **Google**, **YouTube**, **DuckDuckGo**, plus **your own custom engines** (any `https://…?q={query}` template).
- The engine chips pick which one Enter uses; the choice is remembered.
- Search links open in the browser you select in Settings — never inside the island.

---

## Keyboard & mouse reference

| Input | Where | Action |
|---|---|---|
| Hover the pill | Island | Expand |
| Move the cursor away | Island | Collapse (~200 ms) |
| `←` / `→` | Island open | Switch widget (wraps around) |
| `Ctrl` + `Alt` (default) | Anywhere | Make the island click-through so you can click behind it — changeable in Settings |
| `Ctrl` + `V` | Island open | Capture the clipboard into the File Shelf |
| Drag anything | Onto the island | Add to the File Shelf (island opens automatically) |
| Click | Drag a clipboard row | Start a native drag-out |
| `Ctrl` / `Cmd` + click | Clipboard list | Add to selection |
| `Shift` + click | Clipboard list | Select a range |
| Click a format chip | Clipboard | Choose the drag-out format (drag the chip to export) |
| Double-click a clip | Clipboard | Open the clip editor |
| Right-click | Clipboard / Shelf items | Context menu |
| `Enter` | Search box | Open the term in your browser |
| `Esc` | Search box | Clear the query |
| `Enter` | Search result | Open the file |
| `Enter` | Tasks input | Add task |
| `Esc` | Any context menu | Close it |
| Left-click tray icon | Tray | Show / hide the island |

> The click-through shortcut can be set to `Ctrl+Alt`, `Ctrl+Shift`, `Alt+Shift`, `Ctrl+Alt+Shift` or turned **off**, so it never fights another app's shortcut.

---

## Drag and drop

**Into Pill:**

| You drag | Pill stores it as |
|---|---|
| Files / folders | `file` — the original path, never copied |
| Images | `image` — bytes copied into Pill's asset folder |
| Text | `text` — stored inline |
| A URL | `url` — opened in your browser on click |
| Browser selection (HTML) | `html` — with embedded images extracted to assets |

**Out of Pill:**

| From | Result |
|---|---|
| Clipboard row | Real file in your chosen format, dropped wherever you like |
| Clipboard format chip | Converts + exports the whole selection as that format |
| Shelf item | Real file (inline items are materialised first) |
| Search result | The original file, moved/copied by the target app |

---

## Tray menu

- **Show Pill** — bring the island back
- **Settings…** — open the settings window
- **Toggle Clipboard Capture** — pause/resume clipboard history
- **Minified Pill** — switch between the full bar and the thin pill
- **Restart Application**
- **Quit**

Left-clicking the tray icon shows or hides the island.

---

## Settings

| Setting | What it does |
|---|---|
| **Default text format** | Format used when you drag a single clip without picking one |
| **Default image format** | Same, for image clips |
| **Shelf auto-clear** | 1 hour / 24 hours / 7 days / Never for unpinned shelf items |
| **Click-through shortcut** | Modifier combo that makes the island ignore the mouse |
| **Search engine** | Which engine Enter uses (built-ins + your custom engines) |
| **Custom engines** | Add your own with a name and a `…{query}…` URL |
| **Browser** | System default, or a specific installed browser |
| **Everything** | Detection status, HTTP port, manual `es.exe` path, recheck |
| **Clipboard capture** | Pause or resume the clipboard watcher |
| **Start with Windows** | Launch Pill automatically when you sign in (per-user startup entry, no admin) |

Settings are stored locally. **Start with Windows** applies the moment you toggle it; the rest are applied with **Save**.

---

## File search with Everything

Pill does not bundle Everything — it talks to your existing install. Two routes, in this order:

1. **`es.exe`** — the Everything command line tool. Auto-detected next to `Everything.exe`, on your `PATH`, or pick it manually in Settings.
2. **Everything HTTP server** — enable it in Everything under *Tools → Options → HTTP Server*, then set the port in Pill (default `80`).

If neither is available the widget still works as a web-search launcher and says so.

---

## Data, privacy and storage

Everything Pill stores stays on your machine. There are no accounts, no telemetry, and no background uploads. Web searches only go out when you press Enter or click **Web**, and they open in your browser like any other link.

| What | Where |
|---|---|
| Clipboard, shelf, tasks, settings (SQLite) | `%TEMP%\dynamic-island.db3` |
| Shelf image assets | `%LOCALAPPDATA%\com.asrumon.pill\assets` |
| Thumbnails | `%TEMP%\dynamic-island-thumbs` |
| Temporary export files for drag-out | `%TEMP%\dynamic-island-export` |

Clearing the clipboard widget or removing shelf items deletes the underlying rows and image files. Uninstalling Pill removes the program and its startup entry; the folders above can be deleted manually.

---

## Troubleshooting

**The island is invisible.** It may be hidden — left-click the tray icon, or tray → *Show Pill*. It also auto-hides while another app is borderless full-screen (games, video players).

**Clicks pass through the island.** You are holding the click-through shortcut (`Ctrl+Alt` by default). Change it in Settings if it collides with another app.

**Everything shows "Not found".** Start Everything, then either enable its HTTP server or point Settings at `es.exe`, and press **Recheck**.

**File search is empty but Everything works.** Everything indexes only the volumes you added in its own settings; Pill can only see what Everything sees.

**The icon looks stale in Explorer.** Windows caches icons: run `ie4uinit.exe -show`, or move the exe.

**Start with Windows did nothing.** The toggle only points at the executable that is running. In a development build that is the debug binary, which needs the dev server — test it from an installed build.

**A clipboard entry shows a broken image.** Image assets live in Pill's app-data folder; if that folder was cleared, re-copy the image.

---

## Build from source

> Source distribution is not part of the free release. These notes are for the maintainer.

Prerequisites: Node.js 20+, Rust (stable) with the MSVC toolchain, and the WebView2 runtime.

```powershell
cd dynamic-island
npm install
npm run tauri dev          # develop
npm run tauri build -- --bundles nsis   # produce the installer
```

Outputs:

- `dynamic-island\src-tauri\target\release\pill.exe`
- `dynamic-island\src-tauri\target\release\bundle\nsis\Pill_<version>_x64-setup.exe`

Checks used while developing:

```powershell
npx tsc --noEmit -p tsconfig.json            # frontend types
npm run build                                # frontend bundle
cd src-tauri; cargo check --message-format short
cargo test --lib shelf::tests                # shelf/inbox unit tests
```

App icons are generated from `app-icon.svg`:

```powershell
npx tauri icon app-icon.svg
```

### Project layout

```
dynamic-island/
  src/                     React UI (IslandShell, widgets, settings, editor)
  src-tauri/src/
    lib.rs                 app setup + command registration
    state.rs               SQLite schema, AppState
    clipboard/             clipboard watcher + history
    shelf/                 drop inbox (native OLE drop target, thumbnails, export)
    search/                Everything lookups, browser launching
    media/                 Windows media session
    timer/  todo/          focus sessions and tasks
    settings.rs            settings, autostart
    utils/                 window shaping, click-through, drag icon
    tray.rs                tray icon and menu
```

---

## License

**Closed source. Free to use.**

Copyright © 2026 Asaduzzaman Rumon. All rights reserved.

Pill is free of charge for personal and commercial use, on any number of machines you own or operate. It is distributed as compiled software only: you may not copy, modify, decompile, reverse-engineer, redistribute, resell, sublicense or publish the software or any part of it, and you may not create derivative works from it.

The software is provided **as is**, without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose and non-infringement. In no event shall the author be liable for any claim, damages or other liability arising from, out of, or in connection with the software or its use.

Third-party components shipped with Pill (Rust crates, Tauri, Tailwind CSS, framer-motion, and others) remain under their own respective licenses. [Everything](https://www.voidtools.com/) is optional, third-party software and is not bundled or redistributed with Pill.

---

Made by **Asaduzzaman Rumon** · [info.asrumon.workers.dev](https://info.asrumon.workers.dev)
