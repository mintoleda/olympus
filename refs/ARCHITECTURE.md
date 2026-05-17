# Pi Hub — Architecture

## Recommended Stack
- Desktop shell: Tauri
- Frontend: Svelte
- Backend: Rust

### Why this stack
- Tauri keeps the app lightweight compared to Electron.
- Svelte is a strong fit for a small, fast desktop UI.
- Rust provides a good native foundation for Pi integration, filesystem access, background tasks, and sandboxing.

## High-Level System Layout

### Frontend (Svelte)
Responsible for:
- app shell and layout
- home dashboard UI
- chat/session UI
- widget rendering
- search UI
- settings UI
- permission prompts and state presentation

### Backend (Rust)
Responsible for:
- managing the headless Pi session(s)
- session lifecycle
- filesystem access
- persistence
- widget permission enforcement
- IPC between frontend and backend
- platform-specific integrations where needed

## Main UI Regions
- Home / welcome pane
- Chat pane with vertical session tabs
- Widgets / stats pane
- Search pane
- Settings pane

The app should use a multi-pane layout rather than forcing everything into one chat canvas.

## Data Model Concepts

### Session
Represents one Pi conversation context.
Fields likely include:
- session id
- project/directory path
- title
- created/updated timestamps
- messages
- associated model/configuration

### Widget
Represents a built-in or custom extension.
Fields likely include:
- widget id
- name
- type
- layout slot or location
- enabled state
- permissions
- config

### Permission Grant
Represents explicit access a widget may have.
Examples:
- file read/write
- network access
- Pi tool execution
- notifications
- app state access

## Pi Integration
The hub should own its own Pi session rather than connecting to an existing terminal session.

Recommended approach:
- A Rust backend service manages the Pi lifecycle.
- Chat UI sends messages to the backend.
- Backend forwards requests to the Pi agent/session.
- Responses stream back to the UI.

## Session Organization
- Sessions are displayed as vertical tabs.
- Sessions are grouped by project or directory.
- The tab list should make it easy to switch between active contexts.

## Widget System Architecture

### Built-in Widgets
Ship with the app and are trusted by default.
Examples:
- AI tools
- system stats

### Custom Widgets
Loaded from local code in a sandboxed environment.
Required properties:
- explicit capability requests
- user approval flow
- separation from core app state

### Recommended sandbox direction
For v1, prefer a constrained plugin API over arbitrary direct execution.
This keeps the extension system safer and easier to reason about.

Possible implementation options later:
- WASM-based widgets
- isolated JS runtime
- IPC-only plugin processes

## Search Architecture
Search should be implemented as a shared service that can query:
- sessions
- widgets
- commands
- app state
- future file/content indexes

It should start simple and become the app’s global find surface over time.

## Persistence
Persist locally:
- session metadata
- recent sessions
- widget settings
- permission grants
- layout preferences
- theme preferences

Use a lightweight local store suitable for desktop use.

## Theming / Styling
- Theme should be data-driven.
- Support dark mode first.
- Use accent colors for active states and emphasis.
- Allow future theme customization.

## Recommended Internal Boundaries
1. UI shell
2. App state store
3. Pi session manager
4. Widget manager
5. Permission system
6. Search service
7. Persistence layer

## Future Extension Points
- command palette / launcher
- more advanced search indexing
- more widget categories
- plugin marketplace or local registry
- multi-profile support
- cross-device sync
