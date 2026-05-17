# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Olympus is a Tauri 2 desktop app that acts as a hub/wrapper around the external `pi` CLI. The frontend (Svelte 5 + Vite) is purely a renderer; the Rust backend in `src-tauri/` owns one child `pi` process per chat session and bridges its JSON-RPC over stdio into Tauri events that the UI subscribes to.

Product/architecture intent lives in `docs/` — `PRODUCT_SPEC.md`, `ARCHITECTURE.md`, `SPECS.md`, `IMPLEMENTATION_PLAN.md`. The implementation is currently around Phase 3 (chat sessions work; widgets, search, settings are placeholders).

## Commands

Package manager is **bun** (see `bun.lock`; `tauri.conf.json` invokes `bun run dev` / `bun run build`).

- `bun install` — install JS deps
- `bun run tauri dev` — run the full desktop app (spawns Vite on `127.0.0.1:1420` then launches the Tauri window). This is the normal dev loop.
- `bun run dev` — frontend only, no Tauri shell (most code paths that call `invoke(...)` will error without the Rust backend)
- `bun run build` — build the frontend bundle into `dist/`
- `bun run tauri build` — produce a distributable desktop bundle
- `bunx svelte-check --tsconfig ./tsconfig.json` — type-check Svelte + TS (no npm script wired; run directly)

There are no tests, lint config, or formatter wired up in this repo yet.

The `pi` binary must be on `PATH` at runtime — the Rust backend shells out to it (`Command::new("pi")` in `src-tauri/src/lib.rs`). Without it, session creation will fail.

## Architecture

### Process model

One Tauri app process. For each `PiSession`, the Rust backend spawns a `pi --mode rpc` child (with `--session <id>` when resuming) in the session's `project_path`. stdin/stdout are piped; a per-session reader thread parses newline-delimited JSON events from stdout and forwards them as Tauri events. The frontend never touches `pi` directly.

### Rust backend (`src-tauri/src/lib.rs`)

Single file, intentionally. Key pieces:

- `SessionStore` (Tauri-managed state) — holds `sessions: HashMap<id, PiSession>`, `runtimes: HashMap<id, RunningSession>` (live child processes), the active session id, and caches for statuses/widgets/commands.
- `spawn_pi` / `spawn_pi_inner` — idempotent: returns the existing `RunningSession` if one is already running for that id, otherwise spawns a new `pi` child and starts the reader thread.
- Reader thread loop matches on `event.type`:
  - `agent_start` / `message_update` (with `text_delta` or `thinking_delta`) / `agent_end` → assistant streaming
  - `response` for `get_state`, `set_model`, `get_commands` → updates the persisted session and emits `pi://session`
  - `extension_ui_request` with method `select|confirm|input|editor` → forwarded to UI; `setStatus|setWidget|notify|setTitle|set_editor_text` are handled internally and re-emitted as `pi://status` / `pi://widget` / etc.
- `#[tauri::command]` handlers are the entire IPC surface: `create_session`, `list_sessions`, `switch_session`, `send_message`, `send_pi_command`, `list_pi_models`, `set_pi_model`, `set_pi_thinking_level`, `list_pi_commands`, `respond_extension_ui`, `compact_session`, `rename_pi_session`, `stop_session`, `close_session`. All are registered in `run()` — add new commands there and to the `invoke_handler!` macro.
- Persistence: sessions are serialised to `<app_data_dir>/sessions.json` on every mutation via `save_sessions`. Runtime state (running children, caches) is not persisted.

### Frontend (`src/App.svelte`)

The entire UI is in one `App.svelte` (~950 lines) plus `src/animations.ts` (animejs scopes) and `src/styles.css`. There is no component decomposition yet; reactive `$:` blocks drive everything off `sessions`, `activeSessionId`, and the event-derived `sessionStatuses`/`sessionWidgets` maps.

The UI listens for these Tauri events (set up in `onMount`):
- `pi://message` — append/stream chat messages (assistant deltas merge into the same message id)
- `pi://session` — full session snapshot replace
- `pi://extension-ui-request` — opens a modal that calls back via `respond_extension_ui`
- `pi://status`, `pi://widget`, `pi://notify`, `pi://editor-text`

Panes are `home | chat | search | settings` — only home and chat are real; the other two render placeholder copy.

### Slash commands

There are two layers and it's easy to confuse them:
1. **Client-side** (`handleSlashCommand` in `App.svelte`) — UI-only commands like `/model`, `/settings`, `/hotkeys`, `/new`, `/stop`, plus a few that call backend RPCs (`/compact`, `/name`, `/quit`).
2. **Pi-side** — anything not handled client-side falls through to `send_message`, which forwards to the `pi` child. The `BUILTIN_COMMANDS` table in `lib.rs` is just for the autocomplete menu and is merged with the dynamic command list returned by `pi`'s `get_commands` RPC.

When adding a new command, decide which layer it belongs in. Local UI state changes → client side. Anything that changes Pi's session state → let it pass through (or send via `send_pi_command`).

## Conventions worth knowing

- Tauri command names use snake_case in Rust but **the frontend `invoke(...)` calls also use snake_case for the command name and camelCase for arguments** (e.g. `invoke('set_pi_model', { id, provider, modelId })` ↔ `fn set_pi_model(id, provider, model_id, ...)`). Tauri does the case conversion on args only.
- All event names are prefixed `pi://` — keep that prefix for any new ones.
- Sessions are keyed by an internal `session-<n>` id (monotonic counter in `SessionStore`). The separate `pi_session_id` / `pi_session_file` fields are Pi's own identifiers used to resume the conversation when respawning the child.
- The Vite dev server is pinned to `127.0.0.1:1420` (`vite.config.ts` + `tauri.conf.json` `devUrl`); don't change one without the other.
- Tauri capabilities are declared in `src-tauri/capabilities/default.json`. Adding a new Tauri plugin (e.g. `tauri-plugin-fs`) requires both the Cargo dep and a capability entry, otherwise calls will be rejected at runtime.
