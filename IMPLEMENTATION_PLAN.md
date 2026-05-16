# Pi Hub — Implementation Plan

- [x] Phase 0 — Project Setup

  Goal: create the minimal desktop shell.

  Tasks
  - Initialize Tauri app.
  - Set up Svelte frontend.
  - Configure project structure and build scripts.
  - Establish linting, formatting, and type checking.
  - Decide on state management approach.

  Deliverable
  - App launches successfully with a blank shell.

- [x] Phase 1 — App Shell and Layout

  Goal: build the foundational multi-pane interface.

  Tasks
  - Create the main multi-pane layout.
  - Add placeholder panes for:
    - home
    - chat
    - widgets/stats
    - search
    - settings
  - Implement responsive sizing for the desktop window.
  - Add basic theme styling and panel structure.

  Deliverable
  - The app visually resembles a real hub, even before feature completion.

- [x] Phase 2 — Pi Session Management

  Goal: make the chat feature real.

  Tasks
  - Add a headless Pi session manager in Rust.
  - Define a session model.
  - Create session creation, listing, switching, and closing.
  - Stream messages between frontend and backend.
  - Display sessions as vertical tabs.
  - Group sessions by project/directory.

  Deliverable
  - Multiple independent Pi chat sessions work inside the hub.

- [ ] Phase 3 — Home Screen

  Goal: establish the landing experience.

  Tasks
  - Implement the welcome screen.
  - Keep it minimal and lightweight.
  - Add a foundation for future widget placement.

  Deliverable
  - App opens to a clean home view.

- [ ] Phase 4 — Built-in Widgets

  Goal: introduce visible utility beyond chat.

  Tasks
  - Create a widget framework.
  - Build built-in AI tool widgets.
  - Build system stats widgets.
  - Add widget placement in the dashboard.

  Deliverable
  - The hub shows useful non-chat content.

- [ ] Phase 5 — Custom Widgets and Permissions

  Goal: support safe extensibility.

  Tasks
  - Design the widget manifest format.
  - Create permission prompts.
  - Implement sandboxed widget loading.
  - Add explicit capability approval.
  - Expose a limited API surface for widgets.

  Deliverable
  - Local custom widgets can be added safely.

- [ ] Phase 6 — Search and Settings

  Goal: finish the v1 usability story.

  Tasks
  - Add global search UI and service.
  - Add settings screens for theme, layout, sessions, and permissions.
  - Persist user preferences locally.

  Deliverable
  - Users can configure and navigate the hub comfortably.

- [ ] Phase 7 — Polish and Hardening

  Goal: prepare for real daily use.

  Tasks
  - Improve keyboard navigation.
  - Smooth out animations and loading states.
  - Fix platform-specific issues for Hyprland/Arch.
  - Add crash recovery and basic diagnostics.
  - Improve performance and startup time.

  Deliverable
  - A stable, enjoyable first release candidate.

## Immediate Build Order
If building now, the best order is:
1. Tauri + Svelte shell
2. Multi-pane layout
3. Headless Pi session manager
4. Vertical session tabs
5. Home placeholder
6. Built-in widgets
7. Search
8. Settings
9. Sandboxed custom widgets

## Key Technical Risks
- How Pi is embedded or automated in headless mode
- Designing a safe sandbox model for extensions
- Keeping the app lightweight while supporting multiple panes
- Cross-platform compatibility later
