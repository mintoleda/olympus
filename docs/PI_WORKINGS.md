# How pi works (for Olympus contributors)

This document explains pi from the perspective of Olympus integration and extensibility.

It is not a generic “how to use pi” guide; it is a **systems guide** for contributors and agents who need to modify Olympus confidently.

---

## 1) Core design philosophy

pi is intentionally small at the core and very extensible at the edges.

The core avoids embedding every workflow as a built-in feature. Instead, pi expects users to adapt behavior through:

- **Extensions** (TypeScript)
- **Skills** (Agent Skills style instruction bundles)
- **Prompt templates**
- **Themes**
- **Packages** (share bundles of extensions/skills/prompts/themes)

This architecture is why pi tends to be very good at “working on itself”: customization is expected and documented.

For Olympus, this philosophy matters because Olympus is a wrapper around pi. If Olympus wants pi-level adaptability, Olympus should expose similarly clear extension points.

---

## 2) pi run modes

pi supports multiple modes:

1. **Interactive TUI** (normal terminal app)
2. **Print mode / JSON mode** (one-shot or event stream output)
3. **RPC mode** (`pi --mode rpc`) for process integration
4. **SDK usage** (`@earendil-works/pi-coding-agent`) for in-process integrations

Olympus uses **RPC mode**.

---

## 3) Sessions and storage model

pi sessions are persisted as JSONL files under `~/.pi/agent/sessions/` (by default).

Key characteristics:

- Entries form a tree (`id`, `parentId`) rather than a strict linear log.
- Supports branching (`/tree`, `/fork`, `/clone`), compaction entries, model changes, thinking-level changes, labels, and custom extension entries.
- Session history is append-only event-like data in a single file.

Important for Olympus:

- Olympus keeps its own metadata (`sessions.json` in Tauri app data), but pi session files remain the source of truth for transcript continuity and resume behavior.

---

## 4) RPC protocol essentials

When running in RPC mode:

- Client writes LF-delimited JSON commands to stdin.
- pi emits JSON lines to stdout:
  - command responses (`type: "response"`)
  - lifecycle/stream/tool events (e.g., `message_update`, `tool_execution_start`, `agent_end`)

Framing detail:

- Records are separated by `\n` only (JSONL).
- Clients should parse robustly and not assume line-reading semantics that split on extra Unicode separators.

Common commands Olympus relies on:

- `prompt`
- `get_state`
- `set_model`
- `set_thinking_level`
- `get_commands`
- `compact`
- `new_session`
- plus queue/abort/session utilities depending on integration depth

---

## 5) Streaming event model in pi

pi streams incremental updates, not just final responses.

Important event families:

- `agent_start` / `agent_end`
- `turn_start` / `turn_end`
- `message_start` / `message_update` / `message_end`
- tool execution lifecycle
- compaction lifecycle
- retry lifecycle

For text generation, `message_update` carries deltas (like `text_delta`, thinking deltas, etc.).

Why this matters in Olympus:

- Olympus must merge deltas into the same assistant message ID while streaming.
- Status transitions should follow event lifecycle rather than naive timers.

---

## 6) Extension architecture in pi

Extensions are TypeScript modules that can:

- register custom tools
- register slash commands
- react to lifecycle events
- intercept tool calls/results
- modify context/system prompt behavior
- emit custom UI requests
- register providers/models
- maintain extension state across sessions

This is the key extensibility layer that makes pi adaptable without forking core.

---

## 7) Extension UI sub-protocol (critical for wrappers)

In non-TUI integrations (like Olympus via RPC), extension UI requests are emitted as protocol events.

Two categories:

### A) Request/response dialogs

- `select`
- `confirm`
- `input`
- `editor`

Host must collect user response and send an `extension_ui_response` payload correlated by request id.

### B) Fire-and-forget UI signals

- `notify`
- `setStatus`
- `setWidget`
- `setTitle`
- `set_editor_text`

Host may render these or map them into native UI constructs.

Olympus already maps these through Rust → `pi://...` frontend events.

---

## 8) Slash command ecosystem in pi

pi commands can come from multiple origins:

- built-in command set
- extension-registered commands
- prompt templates (filename-based commands)
- skills (typically `/skill:<name>`)

In RPC contexts, wrappers often query command metadata via `get_commands` and merge with local command UX.

---

## 9) Skills, prompt templates, themes, packages

These are “configuration-grade extensibility” layers:

- **Skills**: task-specific instruction packages, loaded on demand
- **Prompt templates**: reusable command-triggered prompt snippets
- **Themes**: TUI color/token customization
- **Packages**: distributable bundles of extensions + skills + prompts + themes

These layers are heavily file-driven and discoverable by convention, another reason pi is easy for agents to evolve.

---

## 10) Model/provider customization

pi supports:

- built-in providers/models
- custom provider/model definitions (e.g., `models.json`)
- extension-level provider registration
- thinking-level controls and model cycling

For wrappers like Olympus, this means model UI should be treated as dynamic runtime state, not hardcoded static options.

---

## 11) Compaction and context management

pi compaction can be manual or automatic. It summarizes older context while preserving recent context windows and branch continuity.

Relevant outputs:

- compaction lifecycle events
- compaction entries in session history

For wrapper UX:

- compaction status should be visible
- summaries are lossy context transforms, so UI should keep expectations clear

---

## 12) Why pi is “self-editable” in practice

pi has strong self-editability because it combines:

1. Stable extension APIs
2. Observable runtime/event contracts
3. File-based discoverable resources
4. Clear command/protocol documentation
5. Fast reload loops and clear customization boundaries

This is exactly the pattern Olympus should emulate for “agents editing Olympus from Olympus”.

---

## 13) What Olympus should take directly from pi

1. **Documented extension contracts first**
2. **Strict event contracts and state semantics**
3. **Minimal core + explicit extension points**
4. **Runtime reloadability for extension resources**
5. **First-class docs for “how to modify behavior safely”**

See:

- [OLYMPUS_SELF_EDITING_GUIDE.md](./OLYMPUS_SELF_EDITING_GUIDE.md)
- [EXTENSIBILITY_STRATEGY.md](./EXTENSIBILITY_STRATEGY.md)
