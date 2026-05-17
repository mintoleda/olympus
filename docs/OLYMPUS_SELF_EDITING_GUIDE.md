# Olympus self-editing guide

This guide is the operational map for agents (and humans) editing Olympus.

If your goal is “change behavior quickly without breaking integration,” start here.

---

## 1) Project structure and ownership boundaries

## Frontend (renderer)

- `src/App.svelte`
  - Main UI surface (currently monolithic)
  - Pane logic (`home | chat | search | settings`)
  - Session list + transcript rendering
  - Slash command interception
  - Tauri event listeners
  - Tauri command invocation calls
- `src/styles.css`
  - Main styling rules
- `src/animations.ts`
  - Animation helper functions
- `src/main.ts`
  - App bootstrap

## Backend (runtime + IPC)

- `src-tauri/src/lib.rs`
  - Session store and lifecycle
  - pi child process spawning
  - RPC command writing
  - stdout/stderr parsing threads
  - Tauri command handlers
  - Tauri event emission
- `src-tauri/src/pi_import.rs`
  - Discovery/import of existing pi sessions from local disk
- `src-tauri/src/main.rs`
  - Entrypoint (`olympus_lib::run()`)

## Config/build

- `package.json` (bun scripts)
- `vite.config.ts` (dev server settings)
- `src-tauri/tauri.conf.json` (devUrl / bundling)
- `src-tauri/capabilities/default.json` (Tauri capability permissions)

---

## 2) Runtime process model

Olympus process model:

1. Tauri app starts.
2. Backend owns session state.
3. For each active Olympus session, backend may spawn one `pi --mode rpc` child process (cwd = session project path).
4. Backend reads pi stdout as JSONL and transforms it into Tauri events.
5. Frontend subscribes to `pi://...` events and updates UI state.

Design implication:

- Frontend never calls pi directly.
- Rust backend is the single integration boundary.

---

## 3) SessionStore model (backend)

`SessionStore` in `src-tauri/src/lib.rs` holds:

- `sessions`: persisted session metadata
- `runtimes`: active pi child handles + stdin channels
- `active`: active Olympus session ID
- caches for commands/status/widgets/pending command requests

Persistence:

- sessions serialized to app data `sessions.json` on mutation
- runtime handles/caches are in-memory only

---

## 4) Current Tauri command surface

Implemented in `src-tauri/src/lib.rs` (`#[tauri::command]`):

- `create_session`
- `reset_pi_session`
- `list_pi_imports`
- `import_pi_session`
- `list_sessions`
- `switch_session`
- `send_pi_command`
- `list_pi_models`
- `set_pi_model`
- `set_pi_thinking_level`
- `list_pi_commands`
- `respond_extension_ui`
- `compact_session`
- `rename_pi_session`
- `stop_session`
- `close_session`
- `send_message`

Rule:

- When adding a new backend command, define function + register in `invoke_handler![]`.

---

## 5) Frontend event subscriptions (`pi://...`)

`App.svelte` listens to:

- `pi://message`
- `pi://session`
- `pi://extension-ui-request`
- `pi://status`
- `pi://widget`
- `pi://notify`
- `pi://editor-text`

If you add new backend-emitted events, add matching frontend listener logic.

---

## 6) pi event translation in backend

Reader thread in `lib.rs` parses pi events and maps them to Olympus behavior.

Important mappings:

- `agent_start` → mark streaming status
- `message_update` deltas:
  - text deltas stream into assistant message
  - thinking deltas stream into thinking block message
- `agent_end` / done paths → finalize assistant/thinking messages
- `response` for:
  - `get_state`
  - `set_model`
  - `get_commands`
- `extension_ui_request` methods:
  - interactive: `select|confirm|input|editor` forwarded to frontend modal
  - fire-and-forget: `setStatus|setWidget|notify|setTitle|set_editor_text` handled + re-emitted

If streamed output appears “wrong”, check this mapping layer first.

---

## 7) Slash command layering (easy to confuse)

There are two slash command layers:

1. **Olympus client-side slash commands** (`handleSlashCommand` in `App.svelte`)
   - purely local UX/state control or direct invoke wrappers
2. **pi-side slash commands**
   - passed through to pi (usually via `send_message` or `send_pi_command`)

Use local interception only for app-local behavior.

---

## 8) File-by-file “what to edit” recipes

## Add a new backend action exposed to UI

1. Add command function in `src-tauri/src/lib.rs`.
2. Register it in `invoke_handler![]`.
3. Add frontend `invoke(...)` call in `src/App.svelte`.
4. If command changes session metadata, emit/update session event so UI refreshes deterministically.

## Add a new pane/view

1. Update `PaneId` union and pane metadata in `src/App.svelte`.
2. Add markup branch for pane content.
3. Add any actions/invokes/event listeners needed.
4. Style in `src/styles.css`.

## Add support for a new pi extension UI method

1. In backend reader event switch, match the new method under `extension_ui_request`.
2. Either:
   - forward request to frontend (if interactive), or
   - convert to an internal event/cache update.
3. Add frontend listener/render logic for new `pi://...` event if required.

## Add new session-level metadata from pi state

1. Parse field in `handle_state_response`.
2. Store in `PiSession` model.
3. Persist and emit `pi://session` update.
4. Render in UI details panel.

---

## 9) Invocation conventions and naming

- Tauri command names used by frontend are snake_case (`invoke('set_pi_model', ...)`).
- Args in frontend are usually camelCase; backend fields are snake_case.

When mismatch bugs happen, inspect both ends explicitly.

---

## 10) Session import behavior

`src-tauri/src/pi_import.rs` supports:

- discovering pi sessions under `~/.pi/agent/sessions`
- lightweight metadata extraction from JSONL
- importing transcript into Olympus session model
- fallback logic if original cwd no longer exists

If import bugs occur, inspect:

- path encoding/decoding for project directories
- metadata parsing and timestamp handling
- content flattening rules for tool blocks

---

## 11) Build/dev constraints

- Package manager: `bun`
- Full dev loop: `bun run tauri dev`
- Frontend-only mode does not provide backend invoke commands
- `pi` must exist on PATH for runtime session creation
- Vite/Tauri host+port alignment (`127.0.0.1:1420`) is required

---

## 12) Safe edit workflow for agents

1. Locate exact boundary (frontend UI, backend RPC bridge, or both).
2. Change smallest surface first.
3. Keep command/event contracts stable.
4. If adding new data fields, update:
   - Rust model
   - persistence behavior
   - emitted events
   - frontend type and rendering
5. Re-test flow with:
   - create session
   - send prompt
   - stream response
   - slash command path
   - extension UI interaction path

---

## 13) Drift warning (important)

This guide reflects the current architecture where:

- backend logic is mostly in one Rust file (`lib.rs`)
- frontend logic is mostly in one Svelte file (`App.svelte`)

If the app is decomposed (recommended), update this guide immediately to preserve agent edit velocity.
