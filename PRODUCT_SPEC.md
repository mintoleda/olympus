# Pi Hub — Product Specification

## Overview
Pi Hub is a lightweight desktop home base for Pi. It replaces the terminal-first experience with a dedicated app that combines:
- a welcome/home surface
- multiple Pi chat sessions
- widgets/extensions
- search
- settings

The product is intentionally broad: it should feel like a central place to launch, chat, inspect, and extend Pi-based workflows.

## Product Goals
- Replace the need to run Pi directly in the terminal.
- Stay lightweight and avoid Electron.
- Be fast to open and responsive during use.
- Support multiple Pi sessions grouped by project/directory.
- Provide a safe extension system for widgets.
- Keep the first release focused, simple, and desktop-native.

## Primary Users
- First user: the creator themselves.
- Later: potentially other power users who want a Pi-centric desktop hub.

## Supported Platforms
- First target: Arch Linux / Hyprland
- Next: Windows
- Later: macOS and broader cross-platform support

## Design Principles
- Lightweight over feature-heavy.
- Desktop-first.
- Multi-pane, not single-purpose.
- Keyboard-friendly.
- Extensible without requiring a monolithic core.

## Core Feature Areas

### 1. Home / Welcome
- Default landing screen.
- Minimal content in v1.
- Acts as the visual hub for future widgets and quick actions.

### 2. Chat Sessions
- Headless Pi session owned by the hub.
- Multiple independent sessions.
- Sessions organized as vertical tabs.
- Tabs grouped/sorted by project or directory.
- Designed to support work across multiple contexts.

### 3. Widgets / Extensions
- Built-in widgets for core data and utilities.
- Custom local widgets/extensions are supported.
- Widgets are sandboxed.
- Widgets request permissions explicitly.

### 4. Search
- Included in v1.
- Intended to evolve into a global search surface across sessions, commands, widgets, files, and app state.

### 5. Settings
- Included in v1.
- Keep settings minimal and focused.

## Initial Widget Types
- AI tools
- System stats

## Extension Model
Widgets can be:
- built-in
- custom local extensions

Custom extensions should be sandboxed and capability-based.

### Suggested permission categories
- UI rendering
- application state access
- Pi tool access
- file access
- network access
- notifications

## Visual Direction
- Dark theme.
- Neon or warm accent highlights.
- Rounded panels and soft contrast.
- Clean and modern without visual noise.

## Out of Scope for v1
- Electron-based implementation
- Heavy multi-account or team collaboration features
- Complex home dashboard content
- Full plugin marketplace
- Overly detailed settings UI

## Success Criteria
The first version is successful if it:
- opens as the main way to use Pi
- supports multiple chat sessions cleanly
- feels lightweight and fast
- allows a home screen to grow into widget-driven customization
- establishes a safe foundation for custom extensions
