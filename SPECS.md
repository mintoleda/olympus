# SPECS.md — Pi Hub / Central Dashboard

## Summary
A lightweight, desktop-first central hub built on Pi. It is intended to replace running Pi in the terminal with a dedicated app that combines:
- a home dashboard
- a chat feature for multiple Pi sessions
- widgets/extensions
- settings
- search

The product should feel like a general home base for Pi usage rather than a single-purpose chatbot.

## Product Goals
- Be the main entry point for Pi instead of the terminal.
- Stay lightweight; avoid Electron.
- Support a polished, multi-pane desktop experience.
- Provide a flexible widget/extension system.
- Keep the first version focused and simple.

## Target Platforms
- Initial target: Hyprland / Arch Linux
- Next: Windows
- Later: macOS and broader cross-platform support

## Preferred Stack
- Desktop shell: Tauri
- Frontend: Svelte
- Reasoning:
  - Tauri offers a strong balance of performance and development speed.
  - Svelte is a good fit for a lightweight desktop hub with less boilerplate.

## Core V1 Structure
The app should use a multi-pane layout.

### Must-have panes in V1
1. Home / welcome
2. Chat sessions
3. Widgets / stats
4. Settings
5. Search

## Home / Welcome Pane
- Should start as a simple welcome screen for now.
- No heavy content requirements yet.
- Intended as the landing area for the app.

## Chat Feature
- The chat feature should manage many separate Pi sessions.
- There should not be only one global chat session.
- Sessions should be organized as vertical tabs.
- Tabs should be ordered by project directory.
- The chat feature is powered by a headless/embedded Pi session owned by the hub.

## Widget / Extension System
Widgets are a core feature.

### Widget types for the first phase
- AI tools
- System stats

### Extension model
- Both built-in widgets and custom local widgets/extensions should be supported.
- Custom widgets should be sandboxed.
- Sandbox permissions should exist.
- Widget permissions should be explicit and capability-based.

### Likely permission categories
- UI rendering
- App state access
- Pi tool access
- File access
- Network access
- Notifications

Exact permission granularity is still TBD.

## Search
- Search is considered part of V1.
- Exact search scope is still TBD.
- Likely future areas: sessions, widgets, commands, files, and app state.

## Settings
- Settings are part of V1.
- Keep them minimal and focused at first.

## Visual Direction
- The concept images suggest a dark, neon-accented style.
- The app should feel modern, soft, and high-contrast without becoming noisy.
- The overall design should remain clean and lightweight.

## Constraints
- Do not use Electron.
- Keep the app performant and lightweight.
- Prefer a desktop-native feel.
- Make the system extensible without making the initial app heavy.

## Open Questions / TBD
- Exact widget API
- Exact capability model for sandboxed widgets
- Search sources and behavior
- Whether AI tools and system stats are separate widgets or grouped under one widget system
- How the home screen should evolve beyond the welcome state
- Detailed settings structure
- Whether commands, quick actions, or a launcher surface should exist in V1

## Working Direction
The safest first implementation path is:
1. Build the Tauri + Svelte shell
2. Create the multi-pane layout
3. Add home, settings, and search placeholders
4. Implement the chat area with multiple Pi sessions
5. Add initial built-in widgets for AI tools and system stats
6. Introduce sandboxed custom widget support with permissions
