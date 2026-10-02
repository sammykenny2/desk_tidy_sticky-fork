# Repository Guidelines

These rules apply to every coding agent working in this repository (Claude Code, Codex, and others). Shared rules and project knowledge belong in this file; an agent's own file (for example `CLAUDE.md`) holds only notes specific to that agent.

## Fork Policy

- This repository is a personal fork of [`sqmw/desk_tidy_sticky`](https://github.com/sqmw/desk_tidy_sticky). **Never contribute upstream.** Do not open pull requests, issues or discussions on the upstream repository, and never push to it. `origin` is the fork (`sammykenny2/desk_tidy_sticky-fork`). If an upstream remote is ever added, use it only for fetching.
- **Commit every change and push it directly to `main` on `origin`.** Do not create feature branches or pull requests.
- The upstream URLs in `package.json`, `src-tauri/Cargo.toml` and the READMEs are upstream metadata. Leave them as they are unless asked to change them.

## Project

Desk Tidy Sticky is a Tauri 2 desktop app with a SvelteKit/Svelte 5 frontend and a Rust backend. It provides desktop sticky notes and a "workstation" with notes, a focus timer and break control. Windows and macOS are the main targets. The Android/iOS shells (`src-tauri/gen/`) boot only the plugin commands. Linux builds, but `src-tauri/src/platform/` has no Linux desktop-layer implementation. The frontend is mostly plain JS type-checked through JSDoc (`jsconfig.json` sets `checkJs` and `strict`); the plugin runtime has a little TypeScript. The UI defaults to Simplified Chinese (`zh`), and most docs and error messages are in Chinese.

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

Before committing, run `make check`, `make test` and `git diff --check`, and make sure they pass.

To run a single test:
- Frontend: `node --test tests/frontend/timetable-v2.test.js`. Add `--test-name-pattern "<regex>"` to run only matching tests.
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml <name_filter>`, for example `preferences`.

The optional browser DOM regressions, `pnpm test:browser` and `pnpm test:plugins:browser`, need Playwright, which the project does not install. Point them at an existing install with `BROWSER_TEST_NODE_MODULES` and `PLAYWRIGHT_CHROMIUM_EXECUTABLE`. They are not part of `make test` and do not exercise Tauri IPC.

The app version appears in three places that must stay in sync: `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`.

## Running the App Locally

- **The dev app shares an installed release's identity and data.** It uses the same identifier (`com.desk-tidy.sticky`) and the same data directory (`directories::ProjectDirs("com", "desk_tidy", "desk_tidy_sticky")`, which on Windows is `%APPDATA%\desk_tidy\desk_tidy_sticky\data`). Quit any installed instance first: the single-instance plugin would otherwise just focus that instance, and the dev app exits. The dev app reads and writes the user's real `notes.json` and `preferences.json`.
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

`GlobalControlState` holds the "global operation" switch (Ctrl+Shift+O, the tray, or "启用贴纸全局操作"). When it is off (the default), non-topmost notes ignore the cursor and stay embedded in the desktop. When it is on, every note is detached and raised topmost so it can be moved and edited. The shared decision lives in `desktop/sticky/layer.rs` (`resolve_note_ignore_cursor`, `apply_note_window_layer_with_interaction_by_label`).

### Rust backend (`src-tauri/src`)

- `lib.rs` gates modules with `#[cfg(desktop)]`/`#[cfg(mobile)]` and `include!`s `desktop_app.rs`, so that Tauri's command macros resolve at the crate root. **Register new commands in the `generate_handler!` list in `desktop_app.rs`**, and also in `mobile_app.rs` if mobile needs them.
- `notes/`: commands → service → repository, with Flutter-era import in `notes/compat`. All note file access goes through `NotesStore::run` / `with_notes_store` (`notes/store.rs`), which serializes access. If the primary `notes.json` can't be read, the store switches to `recoveryRequired` and blocks writes. Never add code that repairs or overwrites user data on that path.
- `runtime/atomic_file.rs`: the shared atomic writer. It writes a synced temp file in the same directory, replaces the target without deleting it first, and keeps a last-known-good backup. Use it for any persisted JSON. Data lives in the OS app-data directory (`runtime/paths.rs`), never in the repo.
- `preferences/`: the frontend sends field patches, which are applied under a lock. Per-field serde defaults in `model.rs` are the first-run values and must match the frontend fallbacks (`p.x ?? true`). An existing empty value is kept as-is; it is not replaced by the default.
- `markdown_storage/`: Markdown export/import of notes.
- `desktop/`: tray, global shortcuts, panel windows, and sticky behavior (`sticky/`: layer and z-order, auto-hide to screen edge, display recovery, frost effects).
- `platform/`: native code. Windows attaches notes to the desktop through WorkerW (`platform/windows/workerw`). macOS uses NSPanel through the vendored `vendor/tauri-nspanel` plus `macOSPrivateApi`.
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

## Docs and Project Tracking

- Read `docs/README.md` (the doc index) first, then `docs/agent-context/current.md` (the current delivery line, risks and accepted gaps).
- `docs/TODO.md` is the single source of truth for task state. `docs/DECISIONS.md` holds the accepted decisions (D-EXT-xxx), and `docs/STEPS.md` holds step status as `tracking-task`/`tracking-step` JSON blocks. Record new work in TODO; do not start a separate task list.
- `docs/issues`, `docs/refactor`, `docs/product` and older `docs/architecture` files are historical ("P3", see `docs/agent-context/p3-index.md`). Open them only for targeted investigation.
- Keep docs portable: use paths relative to the repository, never machine-specific paths.
- Commit messages follow Conventional Commits with a scope, e.g. `feat(plugins): …`, `fix(editor): …`, `docs(plan): …`.
