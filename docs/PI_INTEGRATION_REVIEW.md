I took a look at `~/repos/olympus`. Build health is good:

```txt
npm run build ✅
cargo check ✅
svelte-check ✅
```

My high-level take: **the direction is right**. Olympus is already using the right integration shape: Tauri/Rust owns the app shell, Pi runs as `pi --mode rpc`, and extension UI requests are bridged into frontend events.

But the current integration is still more of a **Pi-inspired chat wrapper** than a faithful Pi host. The biggest risks are around session truth, slash command semantics, and incomplete extension UI handling.

## What you got right

### 1. Correct fundamental architecture

In `src-tauri/src/lib.rs`, you spawn Pi like this:

```rust
Command::new("pi")
  .current_dir(&project_path)
  .arg("--mode")
  .arg("rpc")
```

That’s the correct model. Project-local extensions should work because `current_dir` is set to the project path.

### 2. Extension UI bridge already exists

You handle:

```txt
extension_ui_request
```

and route:

```txt
confirm / select / input / editor -> frontend modal
setStatus -> status feed
setWidget -> widgets
notify -> frontend event
set_editor_text -> prompt editor
```

That is the exact right place for the `ctx.ui` adapter.

### 3. Session import/resume is pointed in the right direction

You’re reading Pi session files and starting RPC with:

```rust
--session <target>
```

That meshes with Pi’s storage model.

---

# Critical issues

## 1. You are reconstructing Pi messages instead of trusting Pi messages

This is probably the biggest architectural mismatch.

In `src-tauri/src/lib.rs`, the reader builds assistant messages manually from `message_update` deltas, then finalizes them on `agent_end`.

Relevant area:

```txt
src-tauri/src/lib.rs
  message_update handling
  agent_end handling
  finalize_assistant(...)
```

This creates several problems:

- You invent message IDs instead of using Pi’s real message IDs.
- Tool-only turns can become fake `"Pi returned no output."` messages.
- `message_update.done` can finalize, then `agent_end` can finalize again.
- Extension-injected messages and custom messages are mostly invisible.
- You miss the actual `message_start` / `message_end` source-of-truth events.

The subagent review caught a concrete bug here too: you can double-finalize assistant output because `message_update.done` finalizes and `agent_end` also finalizes.

### Better approach

Use Pi’s own `message_start`, `message_update`, and especially `message_end` events as your canonical transcript source.

Treat `message_update` as temporary display state only.

At the end of a message, replace/update using the actual Pi message object.

---

## 2. Olympus has its own slash command layer that conflicts with Pi

In `src/App.svelte`:

```ts
if (content.startsWith('/') && await handleSlashCommand(content)) {
  draft = '';
  return;
}
```

Then `handleSlashCommand` intercepts things like:

```txt
/model
/new
/clear
/fork
/compact
/name
/session
/resume
/tree
/quit
```

Some of these are okay as native Olympus commands, but several are semantically wrong from Pi’s perspective.

Examples:

### `/fork`

Currently:

```ts
case 'fork':
  await createSession(activeSession?.project_path);
  return true;
```

That does **not** fork the Pi session tree. It creates a new Olympus/Pi session in the same project.

That loses Pi’s real fork semantics.

### `/resume` and `/tree`

Currently they are swallowed:

```ts
case 'resume':
case 'tree':
  sessionsCollapsed = false;
  return true;
```

So a user typing `/tree` thinks they are invoking Pi, but Olympus eats the command and does not invoke Pi tree navigation.

### `/quit`

Currently closes the Olympus session, not necessarily Pi’s own shutdown flow.

### Built-ins listed but not actually supported

In `BUILTIN_COMMANDS`, you include things like:

```rust
("login", "Configure provider authentication"),
("logout", "Remove provider authentication"),
```

But those are interactive Pi commands, not necessarily RPC prompt commands. If the user selects `/login`, Olympus may send it as a prompt and Pi may not handle it as expected.

### Better approach

Separate command namespaces:

```txt
/pi:<command>      forwarded to Pi
/olympus:<command> handled by Olympus
```

Or mark commands as:

```ts
source: "pi" | "olympus"
execution: "forward" | "local"
```

Do not silently shadow Pi commands unless Olympus faithfully implements their semantics.

---

## 3. Extension UI support is only partially complete

You have the right bridge, but the frontend modal is incomplete.

In `src/lib/components/ExtensionRequestDialog.svelte`, you handle:

```txt
confirm ✅
select ✅
input ✅
editor ❌
```

`editor` falls into:

```svelte
<p>{JSON.stringify(extensionRequest.request)}</p>
```

So any extension using:

```ts
await ctx.ui.editor(...)
```

cannot actually work.

Given Pi’s extension UI protocol, `editor` should be a multiline modal/textarea that responds with:

```json
{ "value": "edited text" }
```

or:

```json
{ "cancelled": true }
```

---

## 4. Only one extension UI request can exist at a time

In `src/App.svelte`:

```ts
let extensionRequest: ExtensionUiRequest | null = null;
```

And:

```ts
onExtensionRequest: (payload) => {
  extensionRequest = payload;
}
```

If two requests arrive close together, the second overwrites the first.

This is fragile for extensions. You want a queue:

```ts
let extensionRequests: ExtensionUiRequest[] = [];
let activeExtensionRequest = extensionRequests[0];
```

Also include the session/project in the modal so the user knows which Pi runtime is asking.

---

## 5. `ctx.ui.custom()` remains a hard compatibility gap

Your own current Pi extensions include terminal-custom UI usage:

```txt
checkpoints-memory-ui
tokenjuice
```

Olympus cannot handle `ctx.ui.custom()` through RPC because Pi RPC degrades it.

This matters more than it looks.

For `tokenjuice`, `/tj status` may silently not show the rich panel because the extension sees `ctx.ui.custom` exists and calls it, but RPC returns `undefined`.

For `checkpoints-memory-ui`, `/todos panel` is worse: it calls:

```ts
const panelPromise = ctx.ui.custom(...)
void panelPromise.finally(...)
```

If RPC returns `undefined`, this can throw.

### Better approach

Add an Olympus-native extension protocol later, but for now you need compatibility fallbacks in those extensions.

Example pattern:

```ts
const result = await ctx.ui.custom?.(...);

if (result === undefined) {
  ctx.ui.setWidget("tokenjuice", lines);
  // or ctx.ui.notify(...)
}
```

---

## 6. You are not yet ready for custom Tauri UI from Pi extensions

Earlier we talked about extensions sending custom structured UI messages like:

```ts
pi.sendMessage({
  customType: "tauri-ui",
  display: false,
  details: {
    component: "tokenjuice-status-panel",
    props: {}
  }
});
```

Olympus currently does not really have a path for this because you mostly ignore actual Pi `message_start` / `message_end` messages and custom message types.

If Olympus wants first-class Pi extension compatibility, it needs to preserve/render:

```txt
customType
details
display
role
content parts
tool calls
tool results
```

Right now `ChatMessage` is flattened to:

```ts
{
  role,
  content: string,
  type?
}
```

That is too lossy.

---

## 7. Session truth is split between Olympus and Pi

Your docs say:

> pi session files remain the source of truth

But implementation-wise, Olympus stores its own messages in `sessions.json` and appends synthetic messages during streaming.

This creates drift:

- If a Pi session changes outside Olympus, Olympus won’t fully sync it.
- If Olympus creates fake messages, they do not exist in Pi.
- If Pi emits custom messages/tool results/compaction entries, they may not appear correctly.
- Imported sessions are flattened, then future display relies on Olympus’ cache.

### Better approach

For each session, Pi should be canonical.

Olympus can cache UI state, but transcript should come from:

```txt
get_messages
message_start
message_update
message_end
agent_end
```

not your own reconstructed transcript.

---

## 8. Model listing should use RPC, not CLI table parsing

In `src-tauri/src/commands.rs`:

```rust
Command::new("pi")
  .arg("--list-models")
```

Then you parse whitespace columns.

This is brittle and separate from the active RPC process.

Pi RPC supports model listing directly via protocol. You should use the RPC command instead so the UI reflects the same runtime, extensions, provider registrations, and current project context.

---

## 9. Streaming behavior is less capable than Pi RPC

In `send_message`:

```rust
if session.status == "streaming" || session.status == "waiting" {
  return Err("session is already streaming");
}
```

But Pi RPC supports:

```json
{
  "type": "prompt",
  "message": "...",
  "streamingBehavior": "steer"
}
```

and:

```txt
steer
follow_up
abort
abort_bash
abort_retry
```

Olympus currently blocks that whole interaction model.

That means it will feel less like Pi during long tool runs.

---

## 10. Process lifecycle is too hard-kill oriented

`stop_session`, `close_session`, and app exit kill the Pi process.

That may skip graceful extension shutdown work like:

```ts
pi.on("session_shutdown", ...)
```

That matters for extensions like:

```txt
mcp
latex-preview
desktop-notifications
tokenjuice
```

Killing is useful as a fallback, but Olympus should prefer graceful shutdown/abort flows first.

---

# What I would fix first

## P0 — Fix transcript handling

Stop finalizing messages twice. Then move toward using Pi’s real message lifecycle.

Minimum immediate fix:

- Do not finalize on both `message_update.done` and `agent_end`.
- Do not emit `"Pi returned no output."` automatically on empty agent end.
- Track whether a text message actually started.

Better fix:

- Use `message_end.message` as canonical.

## P1 — Fix extension UI adapter

Add:

```txt
editor support
request queue
timeout cleanup
setTitle frontend listener
session label in dialog
```

## P2 — Stop shadowing Pi commands incorrectly

Especially fix:

```txt
/fork
/tree
/resume
/login
/logout
```

Either implement them correctly through Pi RPC/session APIs or don’t claim they work.

## P3 — Move model listing to RPC

Replace `pi --list-models` table parsing with the RPC model command.

## P4 — Add extension/custom-message preservation

Extend `ChatMessage` beyond string content so Olympus can support rich Pi messages and custom extension UI later.

---

# Bottom line

Olympus is on the right track, and the basic Pi RPC shape is correct.

But the current implementation treats Pi more like a streaming text backend than a full runtime with:

```txt
sessions
message objects
extension commands
extension UI
custom messages
tool lifecycle
branching
compaction
```

To mesh deeply with Pi, Olympus should **stop reconstructing Pi behavior locally** and instead become a faithful host for Pi’s RPC protocol.

The best design principle going forward:

> Olympus should own presentation, not Pi semantics.
