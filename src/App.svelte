<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    animateChatHistory,
    animateInspectorRefresh,
    animateLatestMessage,
    animateMetricTick,
    animatePaneChange,
    animateVoidEnter,
    animateStreamingStatus,
    attachInteractionAnimations,
    createAppAnimationScope
  } from './animations';
  import type { Scope } from 'animejs';

  type PaneId = 'home' | 'chat' | 'search' | 'settings';
  type ChatMessage = { id: string; role: 'user' | 'assistant' | 'status'; content: string; timestamp: number; type?: string };
  type PiSession = {
    id: string;
    name: string;
    project_path: string;
    status: string;
    messages: ChatMessage[];
    pi_session_id?: string | null;
    pi_session_file?: string | null;
    model?: string | null;
    model_id?: string | null;
    provider?: string | null;
    thinking_level?: string | null;
  };
  type PiSessionMeta = {
    session_id: string;
    session_file: string;
    project_path: string;
    started_at: string;
    last_activity_ms: number;
    message_count: number;
    preview: string;
    provider?: string | null;
    model_id?: string | null;
    thinking_level?: string | null;
  };
  type SessionEvent = { session_id: string; message: ChatMessage };
  type SessionUpdateEvent = { session: PiSession };
  type PiModelOption = { provider: string; id: string; context: string; max_output: string; reasoning: boolean; images: boolean };
  type PiCommandOption = { name: string; description: string; source: string; location?: string; path?: string };
  type ExtensionUiRequest = { session_id: string; request: Record<string, any> };
  type StatusEntry = { key: string; text: string };
  type StatusEvent = { session_id: string; statuses: StatusEntry[] };
  type WidgetEntry = { key: string; lines: string[]; placement: string };
  type WidgetEvent = { session_id: string; widgets: WidgetEntry[] };
  type NotifyEvent = { session_id: string; message: string; level: string };
  type EditorTextEvent = { session_id: string; text: string };
  type PrimaryPreset = 'plan' | 'build' | 'ask' | 'off';
  const PRIMARY_CYCLE: PrimaryPreset[] = ['plan', 'build', 'ask', 'off'];
  type ConfigChooser = 'provider' | 'model' | 'thinking' | null;

  const thinkingLevels = ['off', 'minimal', 'low', 'medium', 'high', 'xhigh'];

  const panes: Array<{ id: PaneId; label: string; key: string; description: string }> = [
    { id: 'home', label: 'Home', key: 'HM', description: 'Resume recent work, open a folder, or start a clean Pi context.' },
    { id: 'chat', label: 'Chat', key: 'CH', description: 'Project-bound Pi conversations with model/provider controls.' },
    { id: 'search', label: 'Find', key: 'FD', description: 'A planned index for sessions, files, commands, and Pi context.' },
    { id: 'settings', label: 'Settings', key: 'ST', description: 'Preferences, permissions, theme, layout, and platform details.' }
  ];

  let activePane: PaneId = 'home';
  let sessions: PiSession[] = [];
  let activeSessionId = '';
  let draft = '';
  let error = '';
  let sessionsCollapsed = false;
  let activeChooser: ConfigChooser = null;
  let modelOptions: PiModelOption[] = [];
  let modelLoading = false;
  let modelFilter = '';
  let commandOptions: PiCommandOption[] = [];
  let commandCache = new Map<string, PiCommandOption[]>();
  let commandFetchInFlight = '';
  let extensionRequest: ExtensionUiRequest | null = null;
  let sessionStatuses = new Map<string, StatusEntry[]>();
  let sessionWidgets = new Map<string, WidgetEntry[]>();
  let rootEl: HTMLElement;
  let chatLogEl: HTMLElement;
  let zoom = 1;
  let animationScope: Scope | undefined;
  let animationReady = false;
  let lastAnimatedPane: PaneId = activePane;
  let lastAnimatedCollapsed = sessionsCollapsed;
  let menuOpen = false;
  let infoCardVisible = true;
  let pendingPiImports: PiSessionMeta[] = [];
  let piImportsLoaded = false;
  let piImportBusy = '';

  function menuClickOutside(node: HTMLElement) {
    const handle = (e: MouseEvent) => {
      if (!node.contains(e.target as Node)) menuOpen = false;
    };
    const add = () => document.addEventListener('mousedown', handle);
    const remove = () => document.removeEventListener('mousedown', handle);
    setTimeout(add, 0);
    return { destroy: remove };
  }
  let lastAnimatedMessageCount = 0;
  let lastAnimatedStatus = '';
  let lastAnimatedSessionId = '';
  let lastAnimatedSessionCount = 0;
  let lastAnimatedMetricKey = '';
  let sessionRailAnimationFrame = 0;
  $: active = panes.find((pane) => pane.id === activePane) ?? panes[0];
  $: activeSession = sessions.find((session) => session.id === activeSessionId) ?? sessions[0];
  $: groupedSessions = Object.entries(
    sessions.reduce<Record<string, PiSession[]>>((groups, session) => {
      (groups[session.project_path] ??= []).push(session);
      return groups;
    }, {})
  ).sort(([a], [b]) => a.localeCompare(b));
  $: recentSessions = [...sessions]
    .sort((a, b) => latestTimestamp(b) - latestTimestamp(a))
    .slice(0, 6);
  $: importedFiles = new Set(sessions.map((s) => s.pi_session_file).filter((p): p is string => !!p));
  $: visiblePiImports = pendingPiImports.filter((meta) => !importedFiles.has(meta.session_file));
  $: activeProjectName = activeSession?.project_path.split('/').filter(Boolean).at(-1) ?? 'workspace';
  $: piRuntimeFacts = [
    { key: 'provider' as const, label: 'Provider', value: activeSession?.provider ?? 'detecting', note: 'current Pi backend' },
    { key: 'model' as const, label: 'Model', value: activeSession?.model ?? 'waiting', note: activeSession?.model_id ?? 'selected model' },
    { key: 'thinking' as const, label: 'Thinking', value: activeSession?.thinking_level ?? 'default', note: 'reasoning level' }
  ];
  $: piWrapperDetails = [
    ...piRuntimeFacts,
    { label: 'Pi session', value: activeSession?.pi_session_id ?? 'not linked yet', note: 'resume identifier' },
    { label: 'Session file', value: activeSession?.pi_session_file ?? 'managed by Pi', note: 'local transcript source' },
    { label: 'Status', value: activeSession?.status ?? 'offline', note: activeSession ? 'RPC connection state' : 'create a session to start Pi' }
  ];
  $: providers = Array.from(new Set(modelOptions.map((model) => model.provider))).sort();
  $: providerCounts = providers.map((provider) => ({
    provider,
    count: modelOptions.filter((model) => model.provider === provider).length
  }));
  $: activeProviderModels = modelOptions.filter((model) => model.provider === activeSession?.provider);
  $: modelSearch = modelFilter.trim().toLowerCase();
  $: filteredModels = activeProviderModels
    .filter((model) => !modelSearch || model.id.toLowerCase().includes(modelSearch))
    .slice(0, 160);
  $: activeStatuses = activeSession ? sessionStatuses.get(activeSession.id) ?? [] : [];
  $: activeWidgets = activeSession ? sessionWidgets.get(activeSession.id) ?? [] : [];
  $: presetStatus = activeStatuses.find((entry) => entry.key === 'opencode-preset' || entry.key === 'preset');
  $: activePreset = parsePreset(presetStatus?.text);
  $: nonPresetStatuses = activeStatuses
    .filter((entry) => entry.key !== 'opencode-preset' && entry.key !== 'preset')
    .filter((entry) => stripAnsi(entry.text).trim().length > 0);
  $: aboveWidgets = activeWidgets.filter((widget) => widget.placement !== 'belowEditor');
  $: belowWidgets = activeWidgets.filter((widget) => widget.placement === 'belowEditor');
  $: commandSearch = draft.startsWith('/') ? draft.slice(1).split(/\s+/, 1)[0].toLowerCase() : '';
  $: visibleCommands = draft.startsWith('/') ? rankCommands(commandOptions, commandSearch).slice(0, 12) : [];
  $: homeStats = [
    { label: 'Sessions', value: String(sessions.length).padStart(2, '0'), note: sessions.length === 1 ? 'context mounted' : 'contexts mounted' },
    { label: 'Project', value: activeProjectName, note: activeSession?.status ?? 'waiting' },
    { label: 'Events', value: String(sessions.reduce((count, session) => count + session.messages.length, 0)), note: 'local transcript entries' }
  ];
  $: activeMessageCount = activeSession?.messages.length ?? 0;
  $: metricKey = `${sessions.length}:${activeProjectName}:${activeMessageCount}`;
  $: if (animationReady && rootEl && activePane !== lastAnimatedPane) {
    lastAnimatedPane = activePane;
    tick().then(() => animatePaneChange(rootEl));
  }
  $: if (animationReady && rootEl && activeSession?.id && activeSession.id !== lastAnimatedSessionId) {
    lastAnimatedSessionId = activeSession.id;
    tick().then(() => {
      animateChatHistory(rootEl);
      animateInspectorRefresh(rootEl);
    });
  }
  $: if (activeSession?.id) {
    const cached = commandCache.get(activeSession.id);
    commandOptions = cached ?? [];
    loadCommandOptions(activeSession.id);
  }
  $: if (animationReady && rootEl && sessions.length !== lastAnimatedSessionCount) {
    lastAnimatedSessionCount = sessions.length;
    tick().then(() => animateMetricTick(rootEl));
  }
  $: if (animationReady && rootEl && metricKey !== lastAnimatedMetricKey) {
    lastAnimatedMetricKey = metricKey;
    tick().then(() => animateMetricTick(rootEl));
  }
  $: if (animationReady && rootEl && activeMessageCount > lastAnimatedMessageCount) {
    lastAnimatedMessageCount = activeMessageCount;
    tick().then(() => {
      animateLatestMessage(rootEl);
      scrollChatToBottom();
    });
  }
  $: if (animationReady && rootEl && activeSession?.status && activeSession.status !== lastAnimatedStatus) {
    lastAnimatedStatus = activeSession.status;
    if (activeSession.status === 'streaming') tick().then(() => animateStreamingStatus(rootEl));
  }

  const timeFormatter = new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' });

  function stripAnsi(text: string): string {
    return text ? text.replace(/\[[0-9;]*[A-Za-z]/g, '') : '';
  }

  function parsePreset(text: string | undefined): PrimaryPreset | undefined {
    if (!text) return undefined;
    const cleaned = stripAnsi(text).trim().toLowerCase();
    const match = cleaned.match(/primary\s*[:=]\s*(plan|build|ask|off)/);
    if (match) return match[1] as PrimaryPreset;
    if (['plan', 'build', 'ask', 'off'].includes(cleaned)) return cleaned as PrimaryPreset;
    return undefined;
  }

  function nextPreset(current: PrimaryPreset | undefined, direction: 1 | -1): PrimaryPreset {
    const idx = current ? PRIMARY_CYCLE.indexOf(current) : -1;
    const len = PRIMARY_CYCLE.length;
    const nextIdx = direction === 1
      ? (idx + 1 + len) % len
      : (idx <= 0 ? len - 1 : idx - 1);
    return PRIMARY_CYCLE[nextIdx];
  }

  async function cyclePrimary(direction: 1 | -1 = 1) {
    if (!activeSession) return;
    const target = nextPreset(activePreset, direction);
    await runAction(() => invoke('send_pi_command', { id: activeSession!.id, content: `/primary ${target}` }));
  }

  function formatTime(timestamp: number) {
    if (!timestamp) return 'no activity';
    return timeFormatter.format(new Date(timestamp));
  }

  function latestTimestamp(session: PiSession): number {
    return session.messages.reduce((max, message) => Math.max(max, message.timestamp), 0);
  }



  async function scrollChatToBottom() {
    await tick();
    if (chatLogEl) chatLogEl.scrollTop = chatLogEl.scrollHeight;
  }

  async function refreshSessions() {
    sessions = await invoke<PiSession[]>('list_sessions');
    const stillExists = sessions.some((session) => session.id === activeSessionId);
    if (!stillExists) {
      activeSessionId = sessions.find((session) => session.status === 'active')?.id ?? sessions[0]?.id ?? '';
    }
  }

  async function refreshPiImports() {
    try {
      pendingPiImports = await invoke<PiSessionMeta[]>('list_pi_imports', { projectPath: null });
    } catch (err) {
      pendingPiImports = [];
      error = String(err);
    } finally {
      piImportsLoaded = true;
    }
  }

  async function importPiSession(meta: PiSessionMeta) {
    if (piImportBusy) return;
    piImportBusy = meta.session_file;
    const session = await runAction(() => invoke<PiSession>('import_pi_session', { sessionFile: meta.session_file }));
    piImportBusy = '';
    if (!session) return;
    sessions = [...sessions.filter((item) => item.id !== session.id), session];
    activeSessionId = session.id;
    activePane = 'chat';
    await refreshSessions();
    await refreshPiImports();
    await scrollChatToBottom();
  }

  function piImportPreview(meta: PiSessionMeta): string {
    const trimmed = meta.preview.trim();
    if (trimmed) return trimmed;
    return `${meta.message_count} message${meta.message_count === 1 ? '' : 's'}`;
  }

  function relativeTime(ms: number): string {
    if (!ms) return '';
    const diff = Date.now() - ms;
    if (diff < 0) return 'just now';
    const sec = Math.floor(diff / 1000);
    if (sec < 60) return `${sec}s ago`;
    const min = Math.floor(sec / 60);
    if (min < 60) return `${min}m ago`;
    const hr = Math.floor(min / 60);
    if (hr < 24) return `${hr}h ago`;
    const day = Math.floor(hr / 24);
    if (day < 30) return `${day}d ago`;
    const mo = Math.floor(day / 30);
    if (mo < 12) return `${mo}mo ago`;
    return `${Math.floor(mo / 12)}y ago`;
  }

  async function runAction<T>(fn: () => Promise<T>): Promise<T | undefined> {
    try {
      const result = await fn();
      error = '';
      return result;
    } catch (err) {
      error = String(err);
      return undefined;
    }
  }

  async function createSession(path?: string) {
    const session = await runAction(() => invoke<PiSession>('create_session', { projectPath: path || null }));
    if (!session) return;
    sessions = [...sessions.filter((item) => item.id !== session.id), session];
    activeSessionId = session.id;
    activePane = 'chat';
    await refreshSessions();
    await refreshPiImports();
    await scrollChatToBottom();
  }

  async function pickProjectAndCreate() {
    const selected = await runAction(() => open({ directory: true, multiple: false, title: 'Choose a project folder' }));
    if (typeof selected !== 'string') return;
    await createSession(selected);
  }

  async function switchSession(id: string) {
    const ok = await runAction(() => invoke('switch_session', { id }));
    if (ok === undefined) return;
    activeSessionId = id;
    await refreshSessions();
    await scrollChatToBottom();
  }

  async function openSession(id: string) {
    await switchSession(id);
    activePane = 'chat';
  }

  async function closeSession(id: string) {
    const ok = await runAction(() => invoke('close_session', { id }));
    if (ok === undefined) return;
    await refreshSessions();
    await refreshPiImports();
  }

  async function send() {
    if (!activeSession || !draft.trim()) return;
    activePane = 'chat';
    const content = draft.trim();
    if (content.startsWith('/') && await handleSlashCommand(content)) {
      draft = '';
      return;
    }
    const ok = await runAction(() => invoke('send_message', { id: activeSession!.id, content: draft }));
    if (ok !== undefined) draft = '';
  }

  function clampZoom(value: number) {
    return Math.min(1.4, Math.max(0.75, Math.round(value * 100) / 100));
  }

  function setZoom(value: number) {
    zoom = clampZoom(value);
    document.documentElement.style.setProperty('--app-zoom', String(zoom));
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && !event.shiftKey) {
      if (event.key === '+' || event.key === '=') {
        event.preventDefault();
        setZoom(zoom + 0.05);
        return;
      }
      if (event.key === '-' || event.key === '_') {
        event.preventDefault();
        setZoom(zoom - 0.05);
        return;
      }
      if (event.key === '0') {
        event.preventDefault();
        setZoom(1);
        return;
      }
    }
    if (event.key === 'Tab' && event.shiftKey) {
      const promptEl = document.getElementById('prompt-input');
      if (!promptEl || document.activeElement !== promptEl) return;
      event.preventDefault();
      cyclePrimary(event.ctrlKey || event.metaKey ? -1 : 1);
    }
  }

  let hotkeysOpen = false;
  let fps = 0;

  function startFpsCounter() {
    let frames = 0;
    let last = performance.now();
    let frameId = 0;

    const loop = (now: number) => {
      frames += 1;
      if (now - last >= 1000) {
        fps = Math.round((frames * 1000) / (now - last));
        frames = 0;
        last = now;
      }
      frameId = requestAnimationFrame(loop);
    };

    frameId = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(frameId);
  }

  async function handleSlashCommand(content: string): Promise<boolean> {
    const [command, ...rest] = content.slice(1).split(/\s+/);
    const args = rest.join(' ').trim();
    switch (command) {
      case 'model':
      case 'scoped-models':
        await openConfigChooser('model');
        if (args) modelFilter = args;
        return true;
      case 'settings':
        activePane = 'settings';
        return true;
      case 'hotkeys':
        hotkeysOpen = true;
        return true;
      case 'new':
        await createSession();
        return true;
      case 'compact':
        await runAction(() => invoke('compact_session', { id: activeSession!.id, customInstructions: args || null }));
        return true;
      case 'name':
        if (args) await runAction(() => invoke('rename_pi_session', { id: activeSession!.id, name: args }));
        return true;
      case 'session':
        sessions = sessions.map((session) => session.id === activeSession!.id ? {
          ...session,
          messages: [...session.messages, {
            id: `${session.id}-session-${Date.now()}`,
            role: 'status',
            content: `session: ${session.pi_session_id ?? session.id}\nmodel: ${session.provider ?? 'unknown'}/${session.model_id ?? session.model ?? 'unknown'}\nthinking: ${session.thinking_level ?? 'default'}\nmessages: ${session.messages.length}`,
            timestamp: Date.now()
          }]
        } : session);
        return true;
      case 'stop':
        if (activeSession) await runAction(() => invoke('stop_session', { id: activeSession!.id }));
        return true;
      case 'resume':
      case 'tree':
        sessionsCollapsed = false;
        return true;
      case 'quit':
        if (activeSession) await closeSession(activeSession.id);
        return true;
      default:
        return false;
    }
  }

  function fuzzyScore(name: string, query: string): number {
    if (!query) return 1;
    const lower = name.toLowerCase();
    if (lower === query) return 1000;
    if (lower.startsWith(query)) return 500 - (lower.length - query.length);
    let qi = 0;
    let score = 0;
    let prevMatch = -2;
    for (let i = 0; i < lower.length && qi < query.length; i++) {
      if (lower[i] === query[qi]) {
        score += 10;
        if (i === prevMatch + 1) score += 8;
        if (i === 0 || /[^a-z0-9]/.test(lower[i - 1])) score += 5;
        prevMatch = i;
        qi++;
      }
    }
    if (qi < query.length) return -1;
    return score - (lower.length - query.length);
  }

  function rankCommands(commands: PiCommandOption[], query: string): PiCommandOption[] {
    if (!query) return [...commands].sort((a, b) => a.name.localeCompare(b.name));
    const scored = commands
      .map((command) => ({ command, score: fuzzyScore(command.name, query) }))
      .filter((entry) => entry.score >= 0)
      .sort((a, b) => b.score - a.score || a.command.name.localeCompare(b.command.name));
    return scored.map((entry) => entry.command);
  }

  async function loadCommandOptions(sessionId: string, force = false) {
    if (!sessionId) return;
    if (!force) {
      const cached = commandCache.get(sessionId);
      if (cached) {
        commandOptions = cached;
        return;
      }
    }
    if (commandFetchInFlight === sessionId) return;
    commandFetchInFlight = sessionId;
    try {
      const commands = await runAction(() => invoke<PiCommandOption[]>('list_pi_commands', { id: sessionId }));
      if (commands) {
        commandCache.set(sessionId, commands);
        if (activeSession?.id === sessionId) commandOptions = commands;
      }
    } finally {
      if (commandFetchInFlight === sessionId) commandFetchInFlight = '';
    }
  }

  async function ensureCommandOptions() {
    if (!activeSession) return;
    await loadCommandOptions(activeSession.id);
  }

  async function refreshCommandOptions() {
    if (!activeSession) return;
    await loadCommandOptions(activeSession.id, true);
  }

  function chooseCommand(command: PiCommandOption) {
    draft = `/${command.name} `;
  }

  async function respondToExtensionRequest(response: Record<string, any>) {
    if (!extensionRequest) return;
    await runAction(() => invoke('respond_extension_ui', {
      id: extensionRequest!.session_id,
      requestId: extensionRequest!.request.id,
      response
    }));
    extensionRequest = null;
  }

  async function ensureModelOptions() {
    if (!activeSession || modelLoading || modelOptions.length) return;
    modelLoading = true;
    const models = await runAction(() => invoke<PiModelOption[]>('list_pi_models', { id: activeSession!.id }));
    if (models) modelOptions = models;
    modelLoading = false;
  }

  async function openConfigChooser(kind: Exclude<ConfigChooser, null>) {
    activeChooser = activeChooser === kind ? null : kind;
    modelFilter = '';
    if (kind === 'provider' || kind === 'model') await ensureModelOptions();
  }

  async function selectProvider(provider: string) {
    if (!activeSession) return;
    const preferredId = activeSession.model_id ?? activeSession.model?.toLowerCase();
    const model = modelOptions.find((item) => item.provider === provider && item.id === preferredId) ?? modelOptions.find((item) => item.provider === provider);
    if (!model) return;
    activeChooser = null;
    const ok = await runAction(() => invoke('set_pi_model', { id: activeSession!.id, provider: model.provider, modelId: model.id }));
    if (ok !== undefined) {
      sessions = sessions.map((session) => session.id === activeSession!.id ? { ...session, provider: model.provider, model: model.id, model_id: model.id, status: 'updating' } : session);
    }
  }

  async function selectModel(model: PiModelOption) {
    if (!activeSession) return;
    activeChooser = null;
    const ok = await runAction(() => invoke('set_pi_model', { id: activeSession!.id, provider: model.provider, modelId: model.id }));
    if (ok !== undefined) {
      sessions = sessions.map((session) => session.id === activeSession!.id ? { ...session, provider: model.provider, model: model.id, model_id: model.id, status: 'updating' } : session);
    }
  }

  async function selectThinking(level: string) {
    if (!activeSession) return;
    activeChooser = null;
    const ok = await runAction(() => invoke('set_pi_thinking_level', { id: activeSession!.id, level }));
    if (ok !== undefined) {
      sessions = sessions.map((session) => session.id === activeSession!.id ? { ...session, thinking_level: level, status: 'updating' } : session);
    }
  }

  onMount(() => {
    let unlisteners: Array<() => void> = [];
    let disposed = false;
    setZoom(zoom);
    window.addEventListener('keydown', handleGlobalKeydown);
    const detachInteractions = attachInteractionAnimations(rootEl);
    const stopFpsCounter = startFpsCounter();
    animationScope = createAppAnimationScope(rootEl);
    animateVoidEnter(rootEl, animationScope);
    animationReady = true;
    lastAnimatedMessageCount = activeMessageCount;
    lastAnimatedStatus = activeSession?.status ?? '';
    lastAnimatedSessionId = activeSession?.id ?? '';
    lastAnimatedSessionCount = sessions.length;
    lastAnimatedMetricKey = metricKey;

    (async () => {
      try {
        const messageCleanup = await listen<SessionEvent>('pi://message', (event) => {
          const { session_id, message } = event.payload;
          sessions = sessions.map((session) => {
            if (session.id !== session_id) return session;
            if (message.role === 'status') return { ...session, status: message.content || 'idle' };
            if (message.role === 'assistant') {
              const existing = session.messages.find((item) => item.id === message.id);
              if (existing) {
                return {
                  ...session,
                  status: 'streaming',
                  messages: session.messages.map((item) =>
                    item.id === message.id ? { ...item, content: item.content + message.content } : item
                  )
                };
              }
            }
            return { ...session, status: message.role === 'assistant' ? 'streaming' : session.status, messages: [...session.messages, message] };
          });
        });
        const sessionCleanup = await listen<SessionUpdateEvent>('pi://session', (event) => {
          const updated = event.payload.session;
          sessions = sessions.some((session) => session.id === updated.id)
            ? sessions.map((session) => (session.id === updated.id ? updated : session))
            : [...sessions, updated];
          if (!activeSessionId) activeSessionId = updated.id;
        });
        const extensionCleanup = await listen<ExtensionUiRequest>('pi://extension-ui-request', (event) => {
          extensionRequest = event.payload;
        });
        const statusCleanup = await listen<StatusEvent>('pi://status', (event) => {
          const next = new Map(sessionStatuses);
          next.set(event.payload.session_id, event.payload.statuses);
          sessionStatuses = next;
        });
        const widgetCleanup = await listen<WidgetEvent>('pi://widget', (event) => {
          const next = new Map(sessionWidgets);
          next.set(event.payload.session_id, event.payload.widgets);
          sessionWidgets = next;
        });
        const notifyCleanup = await listen<NotifyEvent>('pi://notify', (event) => {
          const { session_id, message, level } = event.payload;
          sessions = sessions.map((session) => session.id === session_id ? {
            ...session,
            messages: [...session.messages, {
              id: `${session_id}-notify-${Date.now()}`,
              role: 'status',
              content: `${level}: ${stripAnsi(message)}`,
              timestamp: Date.now()
            }]
          } : session);
        });
        const editorTextCleanup = await listen<EditorTextEvent>('pi://editor-text', (event) => {
          if (activeSession && event.payload.session_id === activeSession.id) {
            draft = event.payload.text;
          }
        });
        if (disposed) {
          messageCleanup();
          sessionCleanup();
          extensionCleanup();
          statusCleanup();
          widgetCleanup();
          notifyCleanup();
          editorTextCleanup();
          return;
        }
        unlisteners = [messageCleanup, sessionCleanup, extensionCleanup, statusCleanup, widgetCleanup, notifyCleanup, editorTextCleanup];
        await refreshSessions();
        await refreshPiImports();
        const importedSet = new Set(sessions.map((s) => s.pi_session_file).filter(Boolean));
        const hasResumable = pendingPiImports.some((meta) => !importedSet.has(meta.session_file));
        if (sessions.length === 0 && !hasResumable) await createSession();
      } catch (err) {
        error = String(err);
      }
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener('keydown', handleGlobalKeydown);
      if (sessionRailAnimationFrame) cancelAnimationFrame(sessionRailAnimationFrame);
      detachInteractions();
      stopFpsCounter();
      animationScope?.revert();
    };
  });
</script>

<main class="void" bind:this={rootEl}>
  <!-- Top-left chrome -->
  <header class="void-chrome">
    <button class="menu-pill" on:click={() => (menuOpen = !menuOpen)}>MENU</button>
    <button class="gear-btn" on:click={() => { activePane = 'settings'; menuOpen = false; }} aria-label="Settings">⚙</button>
    {#if menuOpen}
      <div class="menu-popover" use:menuClickOutside>
        <div class="menu-section">
          <p class="menu-head">TOOLS</p>
          {#each panes as pane}
            <button class="menu-row" class:active={pane.id === activePane} on:click={() => { activePane = pane.id; if (pane.id !== 'chat') activeChooser = null; menuOpen = false; }}>
              <span>{pane.label}</span>
            </button>
          {/each}
        </div>
        <div class="menu-section">
          <p class="menu-head">SESSIONS</p>
          {#if recentSessions.length}
            {#each recentSessions as session}
              <button class="menu-row" class:current={session.id === activeSessionId} on:click={() => { openSession(session.id); menuOpen = false; }}>
                <span>{session.name}</span><small>{session.status}</small>
              </button>
            {/each}
          {:else}
            <p class="menu-empty">No sessions yet</p>
          {/if}
        </div>
        <div class="menu-footer">
          <button on:click={() => { pickProjectAndCreate(); menuOpen = false; }}>Open folder</button>
          <button on:click={() => { createSession(); menuOpen = false; }}>New session</button>
        </div>
      </div>
    {/if}
  </header>

  <!-- Stage -->
  <section class="void-stage">
    <div class="tool-shell" class:no-info={!(infoCardVisible && (activePane === 'chat' || activePane === 'home'))}>

      <!-- Main tool card -->
      <div class="tool-card">
        <div class="tool-card__head">
          <div class="tool-card__icon">{active.key}</div>
          <span class="tool-card__label">{active.label}</span>
          <div class="tool-card__actions">
            {#if activePane === 'chat' && activeSession}
              <button
                class="preset-chip preset-{activePreset ?? 'off'}"
                on:click={() => cyclePrimary(1)}
                title="Cycle plan → build → ask → off"
              ><small>primary</small><strong>{activePreset ?? 'off'}</strong></button>
              <span class="pi-state" class:streaming={activeSession.status === 'streaming'}>{activeSession.status}</span>
            {/if}
            {#if activePane === 'chat' || activePane === 'home'}
              <button class="info-toggle" on:click={() => (infoCardVisible = !infoCardVisible)} title={infoCardVisible ? 'Hide info' : 'Show info'}>
                {infoCardVisible ? '↦' : '↤'}
              </button>
            {/if}
          </div>
        </div>

        {#if activePane === 'chat' && activeSession}
          <div class="chat-body">
            <div class="transcript-head">
              <div><p class="eyebrow">Active transcript</p><p class="session-name">{activeSession.name}</p></div>
              <span class="status-pill" class:streaming={['streaming','thinking','generating'].includes(activeSession.status)}>{activeSession.status}</span>
            </div>
            <div class="chat-log" bind:this={chatLogEl}>
              {#each activeSession.messages as message, index}
                <article class="message {message.role}" style={`--i: ${index}`}>
                  <header><span>{message.role}</span><time>{formatTime(message.timestamp)}</time></header>
                  {#if message.type === 'thinking'}
                    <details class="thinking-block" open>
                      <summary>Thinking</summary>
                      <pre>{message.content}</pre>
                    </details>
                  {:else if message.type === 'tool'}
                    <p class="tool-call">{message.content}</p>
                  {:else}
                    <p>{message.content}</p>
                  {/if}
                </article>
              {/each}
            </div>
            {#if nonPresetStatuses.length || aboveWidgets.length}
              <div class="pi-status-feed">
                {#if nonPresetStatuses.length}
                  <div class="status-row">
                    {#each nonPresetStatuses as status}
                      <span class="status-chip"><small>{status.key}</small><strong>{stripAnsi(status.text).trim()}</strong></span>
                    {/each}
                  </div>
                {/if}
                {#each aboveWidgets as widget (widget.key)}
                  <pre class="pi-widget" aria-label={`pi widget ${widget.key}`}>{widget.lines.map(stripAnsi).join('\n')}</pre>
                {/each}
              </div>
            {/if}
          </div>

          {#if activeChooser}
            <section class="config-popover panel" aria-label="Pi configuration chooser">
              <div class="panel-head">
                <span>Switch {activeChooser}</span>
                <button on:click={() => (activeChooser = null)}>Close</button>
              </div>
              {#if activeChooser === 'thinking'}
                <div class="choice-grid compact">
                  {#each thinkingLevels as level}
                    <button class:chosen={level === activeSession?.thinking_level} on:click={() => selectThinking(level)}>
                      <strong>{level}</strong>
                    </button>
                  {/each}
                </div>
              {:else if modelLoading}
                <p class="empty-note">Loading Pi model registry…</p>
              {:else if activeChooser === 'provider'}
                <div class="choice-grid compact">
                  {#each providerCounts as option}
                    <button class:chosen={option.provider === activeSession?.provider} on:click={() => selectProvider(option.provider)}>
                      <strong>{option.provider}</strong>
                      <small>{option.count} models available</small>
                    </button>
                  {/each}
                </div>
              {:else}
                <input class="model-search" bind:value={modelFilter} placeholder={`Filter ${activeSession?.provider ?? 'current provider'} models…`} />
                {#if activeProviderModels.length && !modelFilter}
                  <p class="chooser-hint">Showing {activeProviderModels.length} models for {activeSession?.provider}.</p>
                {:else if !activeProviderModels.length}
                  <p class="chooser-hint">No models found for the current provider yet.</p>
                {/if}
                <div class="choice-grid models">
                  {#each filteredModels as model}
                    <button class:chosen={model.provider === activeSession?.provider && model.id === activeSession?.model_id} on:click={() => selectModel(model)}>
                      <strong>{model.id}</strong>
                      <small>{model.provider} · ctx {model.context} · out {model.max_output} · thinking {model.reasoning ? 'yes' : 'no'}</small>
                    </button>
                  {/each}
                </div>
              {/if}
            </section>
          {/if}

          {#if draft.startsWith('/')}
            <section class="slash-menu panel" aria-label="Pi slash commands">
              {#if visibleCommands.length}
                {#each visibleCommands as command}
                  <button on:click={() => chooseCommand(command)}>
                    <strong>/{command.name}</strong>
                    <small>
                      {command.description || 'no description'}
                      <em class="cmd-source cmd-{command.source}">{command.source}{command.location ? ` · ${command.location}` : ''}</em>
                    </small>
                  </button>
                {/each}
              {:else}
                <p class="empty-note">No matching commands. Type to fuzzy-filter.</p>
              {/if}
              <button class="slash-refresh" on:click={refreshCommandOptions} type="button">Refresh commands</button>
            </section>
          {/if}

          <div class="card-footer">
            <div class="command-dock">
              <label for="prompt-input">Prompt</label>
              <input id="prompt-input" bind:value={draft} placeholder={`Ask Pi in ${activeSession.name}…`} on:input={ensureCommandOptions} on:keydown={(event) => event.key === 'Enter' && send()} />
              <button on:click={send}>Send</button>
            </div>
            {#if belowWidgets.length}
              <div class="pi-status-feed below">
                {#each belowWidgets as widget (widget.key)}
                  <pre class="pi-widget" aria-label={`pi widget ${widget.key}`}>{widget.lines.map(stripAnsi).join('\n')}</pre>
                {/each}
              </div>
            {/if}
            {#if error}<p class="error">{error}</p>{/if}
          </div>

        {:else if activePane === 'chat'}
          <div class="tool-card__body">
            <div class="placeholder-copy">
              <p class="eyebrow">CHAT / No session</p>
              <h1>No active session.</h1>
              <p>Open a project folder or create a new session to get started.</p>
            </div>
          </div>
          <div class="card-footer">
            <div class="command-dock">
              <label for="prompt-input">Prompt</label>
              <input id="prompt-input" bind:value={draft} placeholder="Create a session first…" disabled />
              <button disabled>Send</button>
            </div>
          </div>

        {:else if activePane === 'home'}
          <div class="home-body">
            <section class="launch-card">
              <p class="eyebrow">Local command center</p>
              <h1>Pick up the thread.</h1>
              <p>Mount a project folder, resume a Pi context, or start a clean session.</p>
              <div class="home-actions" aria-label="Home quick actions">
                <button class="primary-action" on:click={() => (activePane = 'chat')} disabled={!activeSession}>Resume session</button>
                <button on:click={pickProjectAndCreate}>Open folder</button>
                <button on:click={() => createSession()}>New session</button>
              </div>
            </section>
            <section class="home-metrics" aria-label="Workspace overview">
              {#each homeStats as stat}
                <article>
                  <span>{stat.label}</span>
                  <strong>{stat.value}</strong>
                  <small>{stat.note}</small>
                </article>
              {/each}
            </section>
            <section class="pi-imports" aria-label="Resume from pi">
              <div class="panel-head">
                <span class="eyebrow">Resume from pi</span>
                <small>{visiblePiImports.length} found</small>
              </div>
              {#if visiblePiImports.length}
                <div class="pi-import-list">
                  {#each visiblePiImports as meta (meta.session_file)}
                    <button
                      class="pi-import-row"
                      class:busy={piImportBusy === meta.session_file}
                      disabled={!!piImportBusy}
                      on:click={() => importPiSession(meta)}
                    >
                      <div class="pi-import-row__head">
                        <strong>{meta.project_path.split('/').filter(Boolean).at(-1) ?? meta.project_path}</strong>
                        <small>{relativeTime(meta.last_activity_ms)}</small>
                      </div>
                      <small class="pi-import-row__path">{meta.project_path}</small>
                      <p class="pi-import-row__preview">{piImportPreview(meta)}</p>
                      <div class="pi-import-row__meta">
                        <span>{meta.message_count} msg{meta.message_count === 1 ? '' : 's'}</span>
                        {#if meta.model_id}<span>{meta.model_id}</span>{/if}
                        {#if meta.provider}<span>{meta.provider}</span>{/if}
                      </div>
                    </button>
                  {/each}
                </div>
              {:else if piImportsLoaded && sessions.length === 0}
                <p class="empty-note">No pi conversations found in <code>~/.pi/agent/sessions/</code>.</p>
              {:else if !piImportsLoaded}
                <p class="empty-note">Scanning pi sessions…</p>
              {/if}
            </section>
          </div>

        {:else}
          <div class="tool-card__body">
            <div class="placeholder-copy">
              <p class="eyebrow">{active.key} / {active.label}</p>
              <h1>{active.description}</h1>
              <p>This surface will grow into a practical inspector for Pi, project state, permissions, and local tools.</p>
            </div>
          </div>
        {/if}
      </div>

      <!-- Right info card -->
      {#if infoCardVisible && (activePane === 'chat' || activePane === 'home')}
        <div class="tool-card info-card">
          <div class="tool-card__head">
            <span class="tool-card__label">{activePane === 'chat' ? 'Pi wrapper' : 'Recent work'}</span>
            <div class="tool-card__actions">
              {#if activePane === 'chat'}
                <small class="pi-state" class:streaming={activeSession?.status === 'streaming'}>{activeSession?.status ?? 'offline'}</small>
              {/if}
            </div>
          </div>
          {#if activePane === 'chat'}
            <div class="tool-card__body info-body">
              <div class="meters">
                {#each piRuntimeFacts as fact}
                  <button class="meter-button" on:click={() => openConfigChooser(fact.key)}>
                    <span>{fact.label}</span><strong>{fact.value}</strong><small>{fact.note}</small>
                  </button>
                {/each}
              </div>
            </div>
          {:else if activePane === 'home'}
            <div class="tool-card__body info-body">
              <div class="panel-head"><span class="eyebrow">Sessions</span><small>{recentSessions.length} entries</small></div>
              {#if recentSessions.length}
                <div class="recent-list">
                  {#each recentSessions as session}
                    <button class:current={session.id === activeSession?.id} on:click={() => openSession(session.id)}>
                      <span class="session-dot"></span>
                      <strong>{session.name}</strong>
                      <small>{session.project_path}</small>
                      <em>{session.status}</em>
                    </button>
                  {/each}
                </div>
              {:else}
                <p class="empty-note">No sessions yet.</p>
              {/if}
            </div>
          {/if}
        </div>
      {/if}

    </div>
  </section>

  <!-- Global overlays -->
  {#if hotkeysOpen}
    <section class="global-dialog panel" aria-label="Keyboard shortcuts">
      <div class="panel-head"><span>Keyboard shortcuts</span><button on:click={() => (hotkeysOpen = false)}>Close</button></div>
      <ul class="hotkeys-list">
        <li><kbd>Enter</kbd> Send prompt</li>
        <li><kbd>/</kbd> Open command palette</li>
        <li><kbd>Ctrl/⌘ +</kbd> / <kbd>Ctrl/⌘ -</kbd> Zoom in/out</li>
        <li><kbd>Ctrl/⌘ 0</kbd> Reset zoom</li>
      </ul>
    </section>
  {/if}

  {#if extensionRequest}
    <section class="global-dialog panel" aria-label="Pi request">
      <div class="panel-head">
        <span>{extensionRequest.request.title ?? extensionRequest.request.method}</span>
        <button on:click={() => respondToExtensionRequest({ cancelled: true })}>Cancel</button>
      </div>
      {#if extensionRequest.request.method === 'confirm'}
        <p>{extensionRequest.request.message}</p>
        <div class="dialog-actions">
          <button on:click={() => respondToExtensionRequest({ confirmed: false })}>No</button>
          <button class="primary-action" on:click={() => respondToExtensionRequest({ confirmed: true })}>Yes</button>
        </div>
      {:else if extensionRequest.request.method === 'select'}
        <div class="choice-grid compact">
          {#each extensionRequest.request.options ?? [] as option}
            <button on:click={() => respondToExtensionRequest({ value: option.value ?? option.id ?? option })}><strong>{option.label ?? option.name ?? option.value ?? option}</strong></button>
          {/each}
        </div>
      {:else if extensionRequest.request.method === 'input'}
        <input class="model-search" placeholder={extensionRequest.request.placeholder ?? ''} on:keydown={(event) => event.key === 'Enter' && respondToExtensionRequest({ value: (event.currentTarget as HTMLInputElement).value })} />
      {:else}
        <p>{JSON.stringify(extensionRequest.request)}</p>
      {/if}
    </section>
  {/if}

  <div class="fps-overlay">fps: {fps}</div>
</main>
