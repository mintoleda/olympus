# Olympus Docs

This folder is for one core goal:

> Make Olympus easy for agents to understand, extend, and edit from inside Olympus.

Olympus wraps `pi` (RPC mode) in a Tauri desktop app. If we want pi-like extensibility, agents need a fast path to:

1. understand how pi works,
2. understand how Olympus currently integrates pi,
3. make safe, targeted edits,
4. progressively push Olympus toward a richer extension model.

---

## Read order

1. **[PI_WORKINGS.md](./PI_WORKINGS.md)**
   - How pi is designed (modes, sessions, RPC protocol, extension surfaces).
   - What parts matter most for Olympus.

2. **[OLYMPUS_SELF_EDITING_GUIDE.md](./OLYMPUS_SELF_EDITING_GUIDE.md)**
   - Exact map of Olympus source files.
   - Runtime/event flow.
   - “If you want to change X, edit Y” recipes.

3. **[EXTENSIBILITY_STRATEGY.md](./EXTENSIBILITY_STRATEGY.md)**
   - Concrete roadmap to make Olympus emulate pi’s extensibility quality.

---

## Current architecture in one screen

- **Frontend:** Svelte 5 + Vite (`src/App.svelte` is currently the main UI surface)
- **Backend:** Rust + Tauri (`src-tauri/src/lib.rs` owns session/runtime behavior)
- **Agent runtime:** one `pi --mode rpc` child process per Olympus session
- **Bridge:** pi JSON-RPC/stdout events → Rust parser → `pi://...` Tauri events → Svelte UI
- **Persistence:**
  - Olympus metadata: Tauri app data `sessions.json`
  - pi transcript/session truth: `~/.pi/agent/sessions/*.jsonl`

---

## Dev commands

- `bun install`
- `bun run tauri dev` (normal full-app loop)
- `bun run dev` (frontend only)
- `bun run build`
- `bun run tauri build`
- `bunx svelte-check --tsconfig ./tsconfig.json`

---

## Important constraints

- `pi` binary must be available on `PATH`.
- Vite + Tauri dev URL/port must stay aligned (`127.0.0.1:1420`).
- Tauri command names are `snake_case`; frontend invoke args are camelCase.

---

## Source references used for these docs

- Olympus code in this repo (`src/`, `src-tauri/`, `refs/`)
- Local pi docs installed with the pi coding agent package:
  - `README.md`
  - `docs/rpc.md`, `docs/extensions.md`, `docs/skills.md`, `docs/prompt-templates.md`, `docs/themes.md`, `docs/packages.md`, `docs/sdk.md`, `docs/session-format.md`, `docs/compaction.md`, `docs/settings.md`, `docs/models.md`, `docs/custom-provider.md`, `docs/tui.md`, `docs/keybindings.md`
