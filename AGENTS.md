# Repository Guidelines

These rules apply to every coding agent working in this repository (Claude Code, Codex, and others). Shared rules and project knowledge belong in this file; an agent's own file (for example `CLAUDE.md`) holds only notes specific to that agent.

## Fork Policy

- This repository is a personal fork of [`sqmw/desk_tidy_sticky`](https://github.com/sqmw/desk_tidy_sticky). **Never contribute upstream.** Do not open pull requests, issues or discussions on the upstream repository, and never push to it. `origin` is the fork (`sammykenny2/desk_tidy_sticky-fork`). If an upstream remote is ever added, use it only for fetching.
- **Commit every change and push it directly to `main` on `origin`.** Do not create feature branches or pull requests.
- The upstream URLs in `package.json`, `src-tauri/Cargo.toml` and the READMEs are upstream metadata. Leave them as they are unless asked to change them.

## Project

Desk Tidy Sticky is a Tauri 2 desktop app with a SvelteKit/Svelte 5 frontend and a Rust backend. It provides desktop sticky notes and a "workstation" with notes, a focus timer and break control. Windows and macOS are the main targets. Linux is supported on Raspberry Pi OS (labwc/Wayland) and packaged as a `.deb`; see "Linux" under Platform caveats. The Android/iOS shells (`src-tauri/gen/`) boot only the plugin commands. The frontend is mostly plain JS type-checked through JSDoc (`jsconfig.json` sets `checkJs` and `strict`); the plugin runtime has a little TypeScript. The UI defaults to Simplified Chinese (`zh`), and most docs and error messages are in Chinese.

## Commands

`make <target>` runs `scripts/make/task.ps1` on Windows and `scripts/make/task.sh` elsewhere. Each target is shown with its pnpm/cargo equivalent.

| Purpose | make | Direct |
|---|---|---|
| Install deps | `make install` | `pnpm install` (pnpm@10.28.2, pinned by `packageManager`) |
| Run desktop app (dev) | `make dev` | `pnpm tauri dev` (Vite on fixed port 1420, `strictPort`) |
| Frontend only | `make frontend-dev` | `pnpm dev` |
| Type/Svelte check | `make check-frontend` | `pnpm check` |
| Rust check | `make check-rust` | `cargo check --manifest-path src-tauri/Cargo.toml` |
| All checks | `make check` | both of the above |
| Frontend tests | `make test-frontend` | `pnpm test:frontend` (`node --test tests/frontend/*.test.js`) |
| Rust tests | `make test-rust` | `cargo test --manifest-path src-tauri/Cargo.toml` |
| Release exe (no bundle) | `make build` | `pnpm tauri build --no-bundle` |
| Platform bundle | `make package` | `pnpm tauri build` |
| Windows portable zip | `make package-portable` (`-stop` variant kills a running `desk_tidy_sticky.exe` first) | `scripts/windows/build-portable-zip.ps1` |
| Linux `.deb` into `release/` | `make package-deb` (`make package-deb-smoke` validates it) | `scripts/linux/build-deb.sh` (`--install-deps` installs the apt build packages) |

Before committing, run `make check`, `make test` and `git diff --check`, and make sure they pass.

To run a single test:
- Frontend: `node --test tests/frontend/timetable-v2.test.js`. Add `--test-name-pattern "<regex>"` to run only matching tests.
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml <name_filter>`, for example `preferences`.

The optional browser DOM regressions, `pnpm test:browser` and `pnpm test:plugins:browser`, need Playwright, which the project does not install. Point them at an existing install with `BROWSER_TEST_NODE_MODULES` and `PLAYWRIGHT_CHROMIUM_EXECUTABLE`. They are not part of `make test` and do not exercise Tauri IPC.

The app version appears in three places that must stay in sync: `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.

## Running the App Locally

- **The dev app shares an installed release's identity and data.** It uses the same identifier (`com.desk-tidy.sticky`) and the same data directory (`directories::ProjectDirs("com", "desk_tidy", "desk_tidy_sticky")`, which on Windows is `%APPDATA%\desk_tidy\desk_tidy_sticky\data` and on Linux `~/.local/share/desk_tidy_sticky`). Quit any installed instance first: the single-instance plugin would otherwise just focus that instance, and the dev app exits. The dev app reads and writes the user's real `notes.json` and `preferences.json`.
- **`pnpm tauri dev` watches `src-tauri/`.** It rebuilds and relaunches the app after every Rust change. Stopping the app ends the session. Before starting a new session, make sure no older `tauri dev`/`cargo` process is still running; an app it relaunches will hold the single instance and block the new one.
- **Debugging sticky window layers.** Set `DESK_TIDY_LAYER_DEBUG=1` to print every sticky-window layer and input-state transition to stderr, including the window's actual parent and styles. Layer failures are otherwise silent, because the Win32 calls report success while the window ends up elsewhere.
- **Preferences written by release 1.2.5 are zeroed.** Shortcuts are empty, which means disabled, and minute values are 0. Deleting `preferences.json` restores the current defaults. Break reminders are off by default; only an explicit `true` enables them.

## Architecture

### Multi-window model

Each window is a separate webview with its own Svelte state. All windows share one SPA build: `adapter-static` with an `index.html` fallback and `ssr = false`. A window's label determines its role:

- `main` → `/`: the "mini" panel (Ctrl+Shift+N).
- `workspace` → `/workspace`: the workstation. Rust creates it hidden at startup (`desktop/panel.rs`) with background throttling disabled, because it hosts the focus and break timers.
- `note-{id}` → `/note/[id]`: one window per pinned sticky. The frontend creates these in `src/lib/panel/use-window-sync.js`, and only while `overlayEnabled` ("启用桌面贴纸") is on. Rust looks them up by the `note-` prefix.
- Break overlay windows → `/break-overlay`. The separate `plugins` window → `/plugins` is legacy; plugins now render inside the workspace.

Windows keep each other in sync through Tauri events (`notes_changed`, `global_control_changed`, …) and the preferences broadcast in `src/lib/preferences/preferences-sync.js`. Any Window/Webview API the frontend calls needs a matching permission in `src-tauri/capabilities/default.json`.

### Sticky layers and global operation

Each pinned note has one of three layers:

- Topmost (`isAlwaysOnTop`): stays interactive.
- Desktop layer above the icons: the default when a note is pinned.
- Wallpaper layer below the icons (`isWallpaper`).

`GlobalControlState` holds the "global operation" switch (Ctrl+Shift+O, the tray, or "启用贴纸全局操作"). When it is off (the default), non-topmost notes ignore the cursor and stay embedded in the desktop. When it is on, every note is detached and raised topmost so it can be moved and edited. The `desktopStickiesSelectable` preference (off by default; "桌面层贴纸可选取文字" in the mini panel's and the workspace's settings, plus the third workspace sidebar switch, which dims while hidden stickies or global operation override it) lets desktop-layer notes take the mouse for text selection while global operation is off; they stay read-only and in place, and wallpaper-layer notes keep letting clicks through. The shared decision lives in `desktop/sticky/layer.rs` (`resolve_note_ignore_cursor`, `apply_note_window_layer_with_interaction_by_label`), and the note page mirrors it in `src/lib/note/note-interaction-policy.js`; keep the two in step. Apply cursor changes to note windows through `set_note_ignore_cursor` / the `set_note_window_ignore_cursor` command rather than `set_ignore_cursor_events` directly.

### Rust backend (`src-tauri/src`)

- `lib.rs` gates modules with `#[cfg(desktop)]`/`#[cfg(mobile)]` and `include!`s `desktop_app.rs`, so that Tauri's command macros resolve at the crate root. **Register new commands in the `generate_handler!` list in `desktop_app.rs`**, and also in `mobile_app.rs` if mobile needs them.
- `notes/`: commands → service → repository, with Flutter-era import in `notes/compat`. All note file access goes through `NotesStore::run` / `with_notes_store` (`notes/store.rs`), which serializes access. If the primary `notes.json` can't be read, the store switches to `recoveryRequired` and blocks writes. Never add code that repairs or overwrites user data on that path.
- `runtime/atomic_file.rs`: the shared atomic writer. It writes a synced temp file in the same directory, replaces the target without deleting it first, and keeps a last-known-good backup. Use it for any persisted JSON. Data lives in the OS app-data directory (`runtime/paths.rs`), never in the repo.
- `preferences/`: the frontend sends field patches, which are applied under a lock. Per-field serde defaults in `model.rs` are the first-run values and must match the frontend fallbacks (`p.x ?? true`). An existing empty value is kept as-is; it is not replaced by the default.
- `markdown_storage/`: Markdown export/import of notes.
- `desktop/`: tray, global shortcuts, panel windows, and sticky behavior (`sticky/`: layer and z-order, auto-hide to screen edge, display recovery, frost effects).
- `platform/`: native code. Windows attaches notes to the desktop through WorkerW (`platform/windows/workerw`). macOS uses NSPanel through the vendored `vendor/tauri-nspanel` plus `macOSPrivateApi`. Linux turns note windows into wlr-layer-shell surfaces (`platform/linux/note_surface.rs`).
- `breaks/`: the break reminder watchdog and overlay presentation.
- The autostart plugin is registered only in release builds (`#[cfg(not(debug_assertions))]`). The frontend checks `is_autostart_available` before using it.

### Frontend (`src/`)

- Route files are large, but most logic lives in `src/lib/**` as plain-JS factories that receive their dependencies as arguments (e.g. `createWorkspaceStartupActions({ ...deps })`). This lets `tests/frontend/*.test.js` import them directly from `../../src/lib/...` and run them under `node:test` without Tauri. Write new logic the same way so it stays testable.
- `src/lib/workspace/controllers/` holds the workspace route's extracted controllers. `pomodoro/` and `break-control/` hold the timer and break runtime.
- `src/lib/markdown/` plus `src/lib/note/block-note-editor-controller.js` form the block editor. Only one block is edited at a time, in a single textarea; the others render as Markdown (`docs/architecture/2026-06-29-single-active-block-editor.md`).
- Saving note text uses optimistic concurrency. `update_note_text` requires `expectedText`, and change events carry `sourceWindow`. Editors keep the user's draft when a save fails (`src/lib/note/text-commit.js`). Never drop a draft silently, and never auto-merge.
- Strings live in `src/lib/strings.js` as `en`/`zh` tables. Add both when adding a key.
- Dev-only UI is gated on `import.meta.env.DEV` (`src/lib/runtime/dev-flags.js`). `VITE_ENABLE_REVIEW_DEV_FIXTURES=false` turns off the review fixtures.

### App-installed plugins (not Tauri plugins)

- Rust `plugins/package.rs` admits a package only if its SHA-256 matches an allow-list: the `include_str!` of `plugins/timetable-v01/package.sha256` plus one pinned older digest. Arbitrary third-party packages are deliberately rejected. `plugins/admission.rs`, `broker.rs` and `manifest.rs` compile only for tests and are not wired to IPC.
- After editing `plugins/timetable-v01/{entry.js,schema-v2.*}`, run `node scripts/build-timetable-package.mjs`. It regenerates `timetable.dtplugin` and `package.sha256`, and the host must then be rebuilt to embed the new digest. Changing the digest approves new code, so treat it as a security decision.
- The frontend runs plugin source in a module Web Worker (`src/lib/plugins/runtime.ts`) that exposes the `parse`/`view`/`plan` actions with a 4s timeout. Messages are serialized through `wire.js` first, because Svelte 5 `$state` proxies can't be structured-cloned.
- The timetable data contract is `docs/plugins/timetable-schema-v2.md`. v1 input is still accepted.

### Platform caveats

Desktop layering is the most fragile area. It covers WorkerW, wallpaper and icon layers, topmost and bottom placement, transparency and frost, dragging without Aero Snap, and mixed DPI. Automated tests don't cover it, and Windows and macOS behave differently. Before changing `desktop/sticky` or `platform/`, search `docs/issues/` for the root-cause write-ups of earlier regressions, and smoke-test changes on the real OS.

- On Windows 11 24H2+ (build 26200 verified), the wallpaper WorkerW is a child of `Progman` sitting behind `SHELLDLL_DefView`. A note embedded there is under the icon layer.
- tao rewrites a window's whole style from its own flags on every `set_always_on_top`/`set_ignore_cursor_events` call. This drops the `WS_CHILD` that attach added, after which `GetParent` reports NULL even though the note is still inside WorkerW. To tell whether a window is embedded, use `GetAncestor(GA_PARENT)` (`workerw::read_parent`), never `GetParent`.
- Transparent pixels in an embedded note do not show the desktop correctly, so CSS alone cannot round a Windows note. Note windows are clipped with a native rounded window region (`desktop/sticky/corners.rs`), reapplied after every layer change and on `Resized`/`ScaleFactorChanged`, because a region does not follow the window size. Without one, Windows gives an embedded note the legacy themed caption region, which rounds only the top corners. Keep the radius equal to `noteWindowRadius` in the note route.

#### Linux

Verified on Raspberry Pi OS (Debian 13 trixie, arm64) with labwc and pcmanfm's desktop, WebKitGTK 2.54. Root causes and evidence: `docs/issues/2026-10-02-linux-webkitgtk-layer-shell.md`. Build and install: `docs/build/2026-10-02-linux-deb.md`.

- A Wayland toplevel cannot place itself or pick a stacking layer, and labwc refuses XWayland always-on-top by default. On a compositor with wlr-layer-shell, `configure_note_panel_window` turns each note window into a layer surface before it is realized: Bottom is the desktop layer (above pcmanfm's icons, below windows), Top is topmost and global operation, Background stands in for the wallpaper layer (pcmanfm draws wallpaper and icons on one surface, so nothing fits under the icons). `DESK_TIDY_LAYER_SHELL=0` keeps notes as regular windows. Without layer-shell (X11, GNOME) the generic `set_always_on_top` fallback applies.
- A layer surface reports no position (`outer_position` is 0, 0), ignores `set_position`/`set_size`, and takes its size from the GTK size request. `platform/linux/note_surface.rs` tracks each note's logical geometry; the frontend reads and changes it through `get_note_window_position` and `set_note_window_size`, and `move_note_window_without_activation` moves it. Pointer `screenX` is relative to the surface there, so the drag controller keeps a fixed grab point (`surfaceRelativePointer`) and ignores events while a move is in flight or settling, since those are still relative to the old position.
- A WebKitGTK window created `transparent` or with a fully transparent `backgroundColor` repaints only damaged regions after it is shown again or reconfigured, leaving the rest blank or stale. On Linux the main panel (`tauri.linux.conf.json`), the workspace and note windows are created without either, and `note_surface.rs` forces a full repaint (a brief root-opacity flip) after each move, resize or layer change of a note surface. `main.rs` sets `WEBKIT_DISABLE_DMABUF_RENDERER=1` unless the user set it, because the DMA-BUF renderer draws garbled frames on the Pi's GPU.
- tao gives every Wayland window a header bar, which makes GTK treat it as client-side decorated; GTK then rebuilds the window's input region on each size allocation, which a layer-surface move or layer change triggers, discarding the empty region tao's `set_ignore_cursor_events(true)` set. Linux therefore sets the input shape on the GTK widget (`platform/linux::set_note_ignore_cursor`), which survives the rebuild.
- Global shortcuts go through an X11 key grab and do not see keys typed into Wayland windows. A second launch with `--toggle-panel`, `--toggle-global-operation`, `--hide-or-reveal-stickies` or `--quit` runs that action in the running instance (`run_command_line_action`); bind these in labwc's `rc.xml`.
- Tauri's tray (libayatana-appindicator) publishes an absolute PNG path as `IconName` and no `IconPixmap`, which wf-panel-pi cannot draw. On Linux `desktop/tray_linux.rs` publishes the tray itself through ksni with the icon as an ARGB pixmap, the way Electron apps do, and falls back to Tauri's tray when no StatusNotifierItem host is reachable. Both trays share the labels and actions in `desktop/tray.rs`.

## Docs and Project Tracking

- Read `docs/README.md` (the doc index) first, then `docs/agent-context/current.md` (the current delivery line, risks and accepted gaps).
- `docs/TODO.md` is the single source of truth for task state. `docs/DECISIONS.md` holds the accepted decisions (D-EXT-xxx), and `docs/STEPS.md` holds step status as `tracking-task`/`tracking-step` JSON blocks. Record new work in TODO; do not start a separate task list.
- `docs/issues`, `docs/refactor`, `docs/product` and older `docs/architecture` files are historical ("P3", see `docs/agent-context/p3-index.md`). Open them only for targeted investigation.
- Keep docs portable: use paths relative to the repository, never machine-specific paths.
- Commit messages follow Conventional Commits with a scope, e.g. `feat(plugins): …`, `fix(editor): …`, `docs(plan): …`.
