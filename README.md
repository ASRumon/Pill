# Pill
<img src="https://pub-827da1ef12554ad5b31522d9852de6b7.r2.dev/image_1096_20261030_082858_k19ks.png" alt="Pill collapsed at the top of the screen" width="100%">


**A floating dynamic island for Windows.**

Pill lives at the top of your screen as a small, always-on-top pill. Hover it and it opens into a compact panel with everything you were about to switch windows for: your clipboard history, files you dragged in, tasks, media controls, focus sessions, and search.

<p align="center">
  <b>Version</b> 0.1.0 &nbsp;·&nbsp; <b>Platform</b> Windows 10 / 11 (x64) &nbsp;·&nbsp; <b>Free to use</b>
</p>

Author: **Asaduzzaman Rumon** — [info.asrumon.workers.dev](https://info.asrumon.workers.dev)

---

## Screenshots

<table width="100%" cellspacing="8" cellpadding="0" border="0">
  <tr>
    <td width="50%" align="center" valign="top">
      <img src="https://pub-827da1ef12554ad5b31522d9852de6b7.r2.dev/image_1093_20261030_081843_q1k8f.png" alt="Pill collapsed at the top of the screen" width="100%">
      <sub><b>File shelf</b> — Drop files </sub>
    </td>
    <td width="50%" align="center" valign="top">
      <img src="https://pub-827da1ef12554ad5b31522d9852de6b7.r2.dev/image_1092_20261030_081843_3y9y8.png" alt="Pill expanded showing the full panel" width="100%">
      <sub><b> clipboard </b> — access and convert clipboard </sub>
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <img src="https://pub-827da1ef12554ad5b31522d9852de6b7.r2.dev/image_1091_20261030_081843_tv1y6.png" alt="Media widget with artwork and controls" width="100%">
      <sub><b> search </b> — search file videos or even Internet </sub>
    </td>
    <td width="50%" align="center" valign="top">
      <img src="https://pub-827da1ef12554ad5b31522d9852de6b7.r2.dev/image_1090_20261030_081843_u2abz.png" alt="The minified thin pill" width="100%">
      <sub><b> media control </b> </sub>
    </td>
  </tr>
  <tr>
    <td width="50%" align="center" valign="top">
      <img src="https://pub-827da1ef12554ad5b31522d9852de6b7.r2.dev/image_1089_20261030_081843_fkwle.png" alt="Clipboard widget with history and format chips" width="100%">
      <sub><b>Focus</b> — timer right before your eyes </sub>
    </td>
    <td width="50%" align="center" valign="top">
      &nbsp;
    </td>
  </tr>
</table>

---

## Table of contents

- [What is Pill?](#what-is-pill)
- [Requirements](#requirements)
- [Install](#install)
- [First run](#first-run)
- [The island](#the-island)
- [Widgets](#widgets)
  - [Clipboard](#clipboard)
  - [File Shelf](#file-shelf-drop-inbox)
  - [Tasks](#tasks)
  - [Media](#media)
  - [Focus](#focus)
  - [Search](#search)
- [Keyboard & mouse reference](#keyboard--mouse-reference)
- [Drag and drop](#drag-and-drop)
- [Tray menu](#tray-menu)
- [Settings](#settings)
- [File search with Everything](#file-search-with-everything)
- [Your data and privacy](#your-data-and-privacy)
- [Questions & answers](#questions--answers)
- [Troubleshooting](#troubleshooting)
- [Support Pill](#support-pill)
- [License](#license)

---

## What is Pill?

Pill is a small floating bar that sits at the top of your screen. It stays out of the way until you need it — hover it and it opens into a compact panel with the things you reach for all day: your clipboard history, files you dragged in, a quick task list, media controls, focus timers, and search.

- No account. No sign-in.
- No admin rights needed.
- Works offline for everything except web search.
- Always a hover away, never in your way.

---

## Requirements

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr><td width="30%"><b>OS</b></td><td>Windows 10 (2004+) or Windows 11, 64-bit</td></tr>
  <tr><td><b>Disk</b></td><td>~15 MB installed</td></tr>
  <tr><td><b>Optional</b></td><td><a href="https://www.voidtools.com/">Everything</a> by voidtools — only needed for local file search</td></tr>
  <tr><td><b>Not required</b></td><td>No admin rights, no account, no network connection for core features</td></tr>
</table>

Pill installs for your user account only. It never asks for administrator permission.

---

## Install

1. Run `Pill_<version>_x64-setup.exe`.
2. Pick a location (or keep the default folder) and finish.
3. Pill starts immediately and adds an icon to your system tray.

> Upgrading from a pre-release build named *dynamic-island*? Uninstall that build first — Windows treats it as a separate product.

---

## First run

- The pill appears **top-centre of your primary monitor**, collapsed.
- **Hover it** to open the panel. **Move the cursor away** and it collapses again.
- Use the tab strip to switch between widgets, or press **←** / **→**.
- Right-click any item for a context menu (copy, save as, edit, pin, delete).
- Open **Pill Settings** from the tray menu.

---

## The island

The island is one fixed window that never jumps or re-centres while it grows — it simply changes shape.

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr>
    <th align="left">State</th>
    <th align="left">Size</th>
    <th align="left">What you see</th>
  </tr>
  <tr><td>Collapsed</td><td>246 × 36 px</td><td>Quick-launch icons for every widget + local time</td></tr>
  <tr><td>Collapsed, session running</td><td>288 × 36 px</td><td>Live timer chip replaces the clock (see <a href="#focus">Focus</a>)</td></tr>
  <tr><td>Minified</td><td>92 × 24 px</td><td>A thin pill, just the island outline</td></tr>
  <tr><td>Minified, session running</td><td>170 × 34 px</td><td>Mode icon, label and countdown with a progress line</td></tr>
  <tr><td>Expanded</td><td>540 × 430 px</td><td>The full panel with header, tabs and the active widget</td></tr>
</table>

**Collapse behaviour** is pointer-driven: it closes about 200 ms after the cursor leaves, and it never collapses out from under the cursor or while you are dragging something onto it.

**Header** (expanded) shows a live clock, the date (`Thu 30 Oct 2026`) and seven weekday dots with today highlighted.

---

## Widgets

### Clipboard

A searchable history of everything you copy.

- **Automatic capture** of text and images, with a pause switch (Settings, tray, or the widget header).
- **Duplicate-safe:** copying the same text or image again moves the existing entry to the top instead of adding a copy.
- **Multi-select:** enable multi-select in the header, or `Ctrl`-click rows; `Shift`-click selects a range.
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
- **Drag out** to any app — inline text/HTML/image items are turned into real files first.
- Multiple files can be dropped at once.

### Tasks

A lightweight checklist for the day.

- Quick add (Enter), tick off, delete.
- Progress counter and bar (`3/5 Tasks Done Today`).
- Optional 1 / 2 / 3 column layout.

### Media

Mirrors the Windows system media session — whatever is playing in Spotify, a browser, or any other app that publishes a media session.

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

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr>
    <th align="left">Input</th>
    <th align="left">Where</th>
    <th align="left">Action</th>
  </tr>
  <tr><td>Hover the pill</td><td>Island</td><td>Expand</td></tr>
  <tr><td>Move the cursor away</td><td>Island</td><td>Collapse (~200 ms)</td></tr>
  <tr><td><code>←</code> / <code>→</code></td><td>Island open</td><td>Switch widget (wraps around)</td></tr>
  <tr><td><code>Ctrl</code> + <code>Alt</code> (default)</td><td>Anywhere</td><td>Make the island click-through so you can click behind it — changeable in Settings</td></tr>
  <tr><td><code>Ctrl</code> + <code>V</code></td><td>Island open</td><td>Capture the clipboard into the File Shelf</td></tr>
  <tr><td>Drag anything</td><td>Onto the island</td><td>Add to the File Shelf (island opens automatically)</td></tr>
  <tr><td>Click + drag</td><td>A clipboard row</td><td>Start a native drag-out</td></tr>
  <tr><td><code>Ctrl</code> + click</td><td>Clipboard list</td><td>Add to selection</td></tr>
  <tr><td><code>Shift</code> + click</td><td>Clipboard list</td><td>Select a range</td></tr>
  <tr><td>Click a format chip</td><td>Clipboard</td><td>Choose the drag-out format (drag the chip to export)</td></tr>
  <tr><td>Double-click a clip</td><td>Clipboard</td><td>Open the clip editor</td></tr>
  <tr><td>Right-click</td><td>Clipboard / Shelf items</td><td>Context menu</td></tr>
  <tr><td><code>Enter</code></td><td>Search box</td><td>Open the term in your browser</td></tr>
  <tr><td><code>Esc</code></td><td>Search box</td><td>Clear the query</td></tr>
  <tr><td><code>Enter</code></td><td>Search result</td><td>Open the file</td></tr>
  <tr><td><code>Enter</code></td><td>Tasks input</td><td>Add task</td></tr>
  <tr><td><code>Esc</code></td><td>Any context menu</td><td>Close it</td></tr>
  <tr><td>Left-click tray icon</td><td>Tray</td><td>Show / hide the island</td></tr>
</table>

> The click-through shortcut can be set to `Ctrl+Alt`, `Ctrl+Shift`, `Alt+Shift`, `Ctrl+Alt+Shift` or turned **off**, so it never fights another app's shortcut.

---

## Drag and drop

**Into Pill:**

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr>
    <th align="left">You drag</th>
    <th align="left">Pill stores it as</th>
  </tr>
  <tr><td>Files / folders</td><td>The original path, never copied</td></tr>
  <tr><td>Images</td><td>Image bytes copied into Pill's asset folder</td></tr>
  <tr><td>Text</td><td>Stored inline</td></tr>
  <tr><td>A URL</td><td>Opened in your browser on click</td></tr>
  <tr><td>Browser selection (HTML)</td><td>HTML, with embedded images extracted to assets</td></tr>
</table>

**Out of Pill:**

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr>
    <th align="left">From</th>
    <th align="left">Result</th>
  </tr>
  <tr><td>Clipboard row</td><td>Real file in your chosen format, dropped wherever you like</td></tr>
  <tr><td>Clipboard format chip</td><td>Converts + exports the whole selection as that format</td></tr>
  <tr><td>Shelf item</td><td>Real file (inline items are turned into files first)</td></tr>
  <tr><td>Search result</td><td>The original file, moved/copied by the target app</td></tr>
</table>

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

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr>
    <th align="left">Setting</th>
    <th align="left">What it does</th>
  </tr>
  <tr><td><b>Default text format</b></td><td>Format used when you drag a single clip without picking one</td></tr>
  <tr><td><b>Default image format</b></td><td>Same, for image clips</td></tr>
  <tr><td><b>Shelf auto-clear</b></td><td>1 hour / 24 hours / 7 days / Never for unpinned shelf items</td></tr>
  <tr><td><b>Click-through shortcut</b></td><td>Modifier combo that makes the island ignore the mouse</td></tr>
  <tr><td><b>Search engine</b></td><td>Which engine Enter uses (built-ins + your custom engines)</td></tr>
  <tr><td><b>Custom engines</b></td><td>Add your own with a name and a <code>…{query}…</code> URL</td></tr>
  <tr><td><b>Browser</b></td><td>System default, or a specific installed browser</td></tr>
  <tr><td><b>Everything</b></td><td>Detection status, HTTP port, manual <code>es.exe</code> path, recheck</td></tr>
  <tr><td><b>Clipboard capture</b></td><td>Pause or resume the clipboard watcher</td></tr>
  <tr><td><b>Start with Windows</b></td><td>Launch Pill automatically when you sign in (per-user, no admin)</td></tr>
</table>

Settings are stored locally. **Start with Windows** applies the moment you toggle it; the rest are applied with **Save**.

---

## File search with Everything

Pill does not bundle Everything — it talks to your existing install. Two routes, in this order:

1. **`es.exe`** — the Everything command line tool. Auto-detected next to `Everything.exe`, on your `PATH`, or pick it manually in Settings.
2. **Everything HTTP server** — enable it in Everything under *Tools → Options → HTTP Server*, then set the port in Pill (default `80`).

If neither is available the widget still works as a web-search launcher and says so.

---

## Your data and privacy

Everything Pill stores stays on your machine. There are no accounts, no telemetry, and no background uploads. Web searches only go out when you press Enter or click **Web**, and they open in your browser like any other link.

<table width="100%" cellspacing="0" cellpadding="6" border="1">
  <tr>
    <th align="left">What</th>
    <th align="left">Where</th>
  </tr>
  <tr><td>Clipboard, shelf, tasks, settings</td><td><code>%TEMP%\dynamic-island.db3</code></td></tr>
  <tr><td>Shelf image assets</td><td><code>%LOCALAPPDATA%\com.asrumon.pill\assets</code></td></tr>
  <tr><td>Thumbnails</td><td><code>%TEMP%\dynamic-island-thumbs</code></td></tr>
  <tr><td>Temporary export files for drag-out</td><td><code>%TEMP%\dynamic-island-export</code></td></tr>
</table>

Clearing the clipboard widget or removing shelf items deletes the underlying rows and image files. Uninstalling Pill removes the program and its startup entry; the folders above can be deleted manually.

---

## Questions & answers

**Does Pill need an account or internet connection?**
No. There is no sign-in, and every core feature works offline. Only web search uses the internet, and only when you press Enter or click **Web**.

**Does it need administrator rights?**
No. Pill installs for your user account and never asks for elevation.

**Where does my clipboard history go?**
It stays on your PC, in a local database. Nothing is uploaded anywhere.

**Can I pause clipboard capture?**
Yes — from Settings, the tray menu, or the clipboard widget header.

**Does Pill copy the files I drag onto it?**
No. Files and folders are referenced in place. Images and text you drag in are stored inside Pill's own folder.

**How do I stop the island from covering something?**
Hold the click-through shortcut (`Ctrl+Alt` by default) so clicks pass behind it, or hide it by left-clicking the tray icon. You can change or disable the shortcut in Settings.

**Why does file search say "Not found"?**
Pill uses your existing Everything install for local file search. Start Everything, then either enable its HTTP server or point Settings at `es.exe`, and press **Recheck**. Everything must also be indexing the drive you are searching.

**Can I use my own search engine?**
Yes. Add a custom engine in Settings with a name and a `…{query}…` URL. It appears as a chip in the Search widget.

**Can I change which browser search links open in?**
Yes. Pick the system default or a specific installed browser in Settings.

**What happens to unpinned shelf items?**
They clear automatically after the interval you choose (1 hour, 24 hours, 7 days, or never). Pinned items never expire.

**Does Pill start with Windows?**
Only if you turn on **Start with Windows** in Settings. It is a per-user startup entry and needs no admin rights.

**Can I move the pill to another monitor?**
Pill appears on your primary monitor, top-centre.

**Is Pill free?**
Yes. Pill is completely free to use.

---

## Troubleshooting

**The island is invisible.** It may be hidden — left-click the tray icon, or tray → *Show Pill*. It also auto-hides while another app is borderless full-screen (games, video players).

**Clicks pass through the island.** You are holding the click-through shortcut (`Ctrl+Alt` by default). Change it in Settings if it collides with another app.

**Everything shows "Not found".** Start Everything, then either enable its HTTP server or point Settings at `es.exe`, and press **Recheck**.

**File search is empty but Everything works.** Everything indexes only the volumes you added in its own settings; Pill can only see what Everything sees.

**The icon looks stale in Explorer.** Windows caches icons: run `ie4uinit.exe -show`, or move the exe.

**Start with Windows did nothing.** The toggle points at the executable that is running. Test it from an installed build, not a development copy.

**A clipboard entry shows a broken image.** Image assets live in Pill's app-data folder; if that folder was cleared, re-copy the image.

---

## Support Pill

Pill is free to use. If you enjoy it and would like to say thanks, you can buy me a coffee — it's optional and always appreciated.

<p align="center">
  <a href="https://buymeacoffee.com/asrumon"><b>☕ Buy me a coffee</b></a>
</p>

---

## License

**Free to use.**

Copyright © 2026 Asaduzzaman Rumon. All rights reserved.

Pill is free of charge for personal and commercial use, on any number of machines you own or operate.

[Everything](https://www.voidtools.com/) is optional, third-party software and is not bundled or redistributed with Pill.

---

Made by **Asaduzzaman Rumon** · [info.asrumon.workers.dev](https://info.asrumon.workers.dev)
