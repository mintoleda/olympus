# Extensibility strategy: make Olympus feel like pi

This document is a concrete roadmap for making Olympus as easy to evolve as pi.

pi is strong at self-extension because its customization surfaces are first-class. Olympus should follow the same shape, not just wrap RPC.

---

## Desired end-state

Agents inside Olympus can:

1. discover Olympus architecture quickly,
2. add/modify behavior with small, local patches,
3. register new UX/runtime capabilities without editing core files every time,
4. iterate quickly via reloadable extension surfaces.

---

## Strategy pillars

## Pillar A — Discoverability

- Keep architecture and contracts documented in-repo.
- Keep event/command naming explicit and stable.
- Add "where to edit what" maps and recipes.

## Pillar B — Modularity

- Reduce monolithic hotspots (`App.svelte`, `lib.rs`) into stable modules.
- Minimize blast radius per edit.

## Pillar C — Explicit extension contracts

- Define Olympus-native extension hooks (frontend + backend).
- Use capability declarations and well-scoped APIs.

## Pillar D — Fast iteration loop

- Add reloadable resources and diagnostics.
- Make extension failures visible and non-fatal.

---

## Phased roadmap

## Phase 1 (Now): documentation foundation

Status: started with this docs set.

Deliverables:

- `docs/PI_WORKINGS.md`
- `docs/OLYMPUS_SELF_EDITING_GUIDE.md`
- `docs/EXTENSIBILITY_STRATEGY.md`

Outcome:

- agents can navigate current architecture without guesswork.

---

## Phase 2: structural decomposition for editability

### Frontend decomposition targets

Split `src/App.svelte` into component modules such as:

- `SessionRail`
- `ChatTranscript`
- `PromptComposer`
- `ModelControls`
- `ExtensionUiModal`
- `HomePane`
- `SettingsPane`

### Backend decomposition targets

Split `src-tauri/src/lib.rs` into modules:

- session store + persistence
- pi process/runtime management
- RPC IO + parser
- event translators
- tauri command handlers

Outcome:

- most feature edits affect 1–2 files, not giant cross-cutting files.

---

## Phase 3: Olympus extension API (first-class)

Design an Olympus extension contract inspired by pi extension APIs.

Potential API surfaces:

- register Olympus slash commands
- register sidebar widgets/status cards
- subscribe to Olympus lifecycle events
- intercept outbound prompt flow (optional, permissioned)
- contribute settings panels
- expose custom actions with keybindings

Security and control:

- capability declaration per extension
- explicit user enable/disable flow
- clear provenance of extension source/path

Outcome:

- many feature requests become extension code, not core edits.

---

## Phase 4: extension runtime and reload loop

Add:

- `/reload`-like extension reload command in Olympus UI
- extension diagnostics panel:
  - load errors
  - runtime errors
  - active capabilities
- event trace/debug panel for extension development

Outcome:

- extension development loop approaches pi-level speed.

---

## Phase 5: agent-native editing ergonomics

Provide in-project helpers that let agents modify Olympus with predictable scaffolds:

- “create new command” scaffold
- “create new pane” scaffold
- “create new event bridge” scaffold
- “create new widget” scaffold

Optional:

- ship Olympus-local skills/prompts specifically for Olympus codebase edits.

Outcome:

- high-confidence edits become repeatable templates.

---

## Immediate technical priorities (highest ROI)

1. **Stabilize command/event contract docs**
   - treat command and event names as public interfaces.

2. **Extract backend event parsing into dedicated module**
   - isolate pi protocol adaptation from unrelated logic.

3. **Extract frontend event listeners into dedicated store/service**
   - reduce reactive coupling in main component.

4. **Add contract tests around command/event payload shapes**
   - prevent regressions when refactoring.

---

## Proposed compatibility contract for future work

When adding a new Olympus integration feature, define:

- command name
- payload schema
- success/error response shape
- emitted events and payloads
- persistence semantics
- extension interaction points (if any)

This mirrors pi’s contract-first style and preserves editability at scale.

---

## Risks and mitigations

## Risk: monolithic files remain bottlenecks

Mitigation:

- prioritize decomposition before major feature expansion.

## Risk: undocumented event drift breaks UI quietly

Mitigation:

- maintain event contract table in docs.
- add lightweight integration checks.

## Risk: extension model introduces security concerns

Mitigation:

- capability gates, explicit consent, disable-by-default for risky scopes.

---

## Success criteria

This strategy is successful when:

1. Agents can implement common Olympus changes with minimal context loading.
2. Core changes become rarer than extension-level changes.
3. Regressions from protocol/UI drift are caught early.
4. Olympus can evolve workflow behavior without continuous core-file surgery.
