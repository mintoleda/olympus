# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Olympus is a Tauri 2 desktop app that acts as a faithful host for the external `pi` CLI. The frontend (Svelte 5 + Vite) is purely a renderer; the Rust backend in `src-tauri/` owns one child `pi` process per chat session and bridges its JSON-RPC over stdio into Tauri events that the UI subscribes to.

**Guiding principle:** Olympus owns presentation, Pi owns semantics. Transcripts, session lifecycle, and slash command meaning all come from Pi. Olympus caches them locally for offline display.

Context for agents lives in `docs/` — read `PI_WORKINGS.md`, `OLYMPUS_SELF_EDITING_GUIDE.md`, and `EXTENSIBILITY_STRATEGY.md`. There is no `ARCHITECTURE.md`/`SPECS.md`/`IMPLEMENTATION_PLAN.md` despite older references.

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

### Rust backend (`src-tauri/src/`)

Split across a few files:

- `lib.rs` — process spawning, reader-loop event dispatch, RPC helpers, app run/exit. The reader loop matches on Pi event types and routes each to a handler.
- `commands.rs` — every `#[tauri::command]` handler. Add new commands here AND register them in `lib.rs::run()`'s `invoke_handler!` macro.
- `pi_events.rs` — message emission, status updates, and the canonical message-building helpers (`build_messages_from_pi`, `finalize_message_end`, `replace_session_transcript`).
- `state.rs` — shared types (`PiSession`, `ChatMessage`, `ContentPart`, `SessionStore`, etc.).
- `pi_import.rs` — JSONL parsing for imported Pi sessions.
- `persistence.rs` — `sessions.json` read/write.

Key behaviors:

- `SessionStore` (Tauri-managed state) — holds `sessions: HashMap<id, PiSession>`, `runtimes: HashMap<id, RunningSession>` (live child processes), the active session id, and caches for statuses/widgets/commands/pending-models/pending-commands.
- `spawn_pi` / `spawn_pi_inner` — idempotent: returns the existing `RunningSession` if one is already running for that id, otherwise spawns a new `pi` child and starts the reader thread. On resume, also issues `get_messages` so Olympus rehydrates from Pi's canonical transcript.
- Reader thread loop matches on `event.type`:
  - `agent_start` → clears stream state and marks streaming.
  - `message_start` → captures Pi's real message id.
  - `message_update` (with `text_delta` / `thinking_delta`) → ephemeral display deltas.
  - `message_update.done` → no-op (message_end is canonical).
  - **`message_end`** → canonical persistence via `finalize_message_end`. Preserves text/thinking/tool_use/tool_result/custom content parts plus `customType`, `details`, `display`.
  - `agent_end` → defensive fallback persistence if Pi did not emit `message_end`; otherwise just marks idle.
  - `response` for `get_state`, `get_messages`, `get_commands`, `list_models`, `set_model` → state mutations and async-channel deliveries to `#[tauri::command]` handlers.
  - `extension_ui_request` with method `select|confirm|input|editor|custom` → forwarded to UI; `setStatus|setWidget|notify|setTitle|set_editor_text` are handled internally and re-emitted as `pi://status` / `pi://widget` / `pi://title` / etc.
- Graceful shutdown: `stop_session`, `close_session`, and app-exit all go through `graceful_shutdown(runtime, session_id, timeout)` — send `session_shutdown` RPC, poll `try_wait` for up to 2 s, then `kill()` as fallback. App exit parallelizes shutdowns across all runtimes.
- Persistence: sessions are serialised to `<app_data_dir>/sessions.json` on every mutation via `save_sessions`. Runtime state (running children, channels, caches) is not persisted. Olympus's `sessions.json` is a *cache* — Pi's session files on disk are the source of truth.

### Frontend (`src/App.svelte`)

The entire UI is in one `App.svelte` (~950 lines) plus `src/animations.ts` (animejs scopes) and `src/styles.css`. There is no component decomposition yet; reactive `$:` blocks drive everything off `sessions`, `activeSessionId`, and the event-derived `sessionStatuses`/`sessionWidgets` maps.

The UI listens for these Tauri events (set up in `onMount` via `attachPiEventListeners`):
- `pi://message` — append/stream chat messages. Messages with non-empty `content_parts` are canonical (from `message_end`) and *replace* the streamed version; messages without `content_parts` are deltas and *append* to the same id.
- `pi://session` — full session snapshot replace (sent on state changes, transcript hydration, etc.).
- `pi://extension-ui-request` — queued in `extensionRequestQueue` (FIFO). The head request drives `ExtensionRequestDialog`.
- `pi://session-closed` — emitted when a session's reader thread exits; the frontend drops any queued extension requests for that session.
- `pi://status`, `pi://widget`, `pi://notify`, `pi://editor-text`, `pi://title`.

`ChatMessage` shape (`src/lib/types/pi.ts`) — `content` is a flattened text view kept for back-compat; `content_parts` is a tagged union of `text | thinking | tool_use | tool_result | custom` for rich rendering. Rendering in `ChatPane.svelte` prefers `content_parts` when present.

Custom UI components are registered in `src/lib/components/customUI/registry.ts`. Pi extensions that call `ctx.ui.custom({ component, props })` route to the registered Svelte component; unknown components fall back to a JSON-stringified panel with a Close button.

Panes are `home | chat | search | settings` — only home and chat are real; the other two render placeholder copy.

### Slash commands

Two layers, kept deliberately separate so we don't shadow Pi semantics:

1. **Olympus-local** (`handleSlashCommand` in `App.svelte`) — commands that change Olympus UI state or call dedicated Olympus RPCs: `/model`, `/scoped-models`, `/settings`, `/hotkeys`, `/new`, `/clear`, `/compact`, `/name`, `/session`, `/stop`, `/quit`. The `BUILTIN_COMMANDS` table in `lib.rs` lists exactly these for autocomplete.
2. **Pi-side** — *everything else* (`/fork`, `/tree`, `/resume`, `/login`, `/logout`, extension commands, prompt templates) falls through to `send_message`, which forwards to Pi. Autocomplete entries for these come dynamically from Pi's `get_commands` RPC, not from `BUILTIN_COMMANDS`.

When adding a new command:
- If it changes Olympus UI state or wraps a dedicated Olympus RPC: add to `BUILTIN_COMMANDS` and `handleSlashCommand`.
- If it changes Pi session state: do not intercept it. Let it pass through and let Pi handle it.
- Never advertise a command in `BUILTIN_COMMANDS` if Olympus cannot actually execute it correctly — Pi will already advertise its own commands via `get_commands`.

### Streaming behavior

`send_message` accepts an optional `streamingBehavior` ∈ {`steer`, `follow_up`, `abort`, `abort_bash`, `abort_retry`}. When the session is busy and a behavior is provided, the prompt is forwarded with that behavior; without a behavior, busy sessions are rejected. The `ChatPane` swaps the Send button for Steer/Abort buttons while streaming; `abort_bash` is preferred when status starts with `running:` (a tool call in flight).

## Conventions worth knowing

- Tauri command names use snake_case in Rust but **the frontend `invoke(...)` calls also use snake_case for the command name and camelCase for arguments** (e.g. `invoke('set_pi_model', { id, provider, modelId })` ↔ `fn set_pi_model(id, provider, model_id, ...)`). Tauri does the case conversion on args only.
- All event names are prefixed `pi://` — keep that prefix for any new ones.
- Sessions are keyed by an internal `session-<n>` id (monotonic counter in `SessionStore`). The separate `pi_session_id` / `pi_session_file` fields are Pi's own identifiers used to resume the conversation when respawning the child.
- The Vite dev server is pinned to `127.0.0.1:1420` (`vite.config.ts` + `tauri.conf.json` `devUrl`); don't change one without the other.
- Tauri capabilities are declared in `src-tauri/capabilities/default.json`. Adding a new Tauri plugin (e.g. `tauri-plugin-fs`) requires both the Cargo dep and a capability entry, otherwise calls will be rejected at runtime.
