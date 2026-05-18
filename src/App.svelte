<script lang="ts">
  import { onMount, tick } from 'svelte';
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
  import ChatPane from './lib/components/ChatPane.svelte';
  import ExtensionRequestDialog from './lib/components/ExtensionRequestDialog.svelte';
  import HomePane from './lib/components/HomePane.svelte';
  import RadialDock from './lib/components/RadialDock.svelte';
  import { attachPiEventListeners } from './lib/services/piEvents';
  import { piClient } from './lib/services/piClient';
  import {
    PRIMARY_CYCLE,
    panes,
    thinkingLevels,
    type ConfigChooser,
    type ExtensionUiRequest,
    type PaneId,
    type PiCommandOption,
    type PiModelOption,
    type PiSession,
    type PiSessionMeta,
    type StatusEntry,
    type WidgetEntry
  } from './lib/types/pi';
  import {
    latestTimestamp,
    nextPreset,
    parsePreset,
    rankCommands,
    stripAnsi
  } from './lib/utils/pi';

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
  let extensionRequestQueue: ExtensionUiRequest[] = [];
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
  let sendInFlight = false;
  let steerInFlight = false;

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
  $: activeExtensionRequest = extensionRequestQueue[0] ?? null;
  $: extensionRequestSession = activeExtensionRequest
    ? sessions.find((s) => s.id === activeExtensionRequest!.session_id)
    : undefined;
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
    if (['streaming','waiting','resetting'].includes(activeSession.status)) tick().then(() => animateStreamingStatus(rootEl));
  }


  async function cyclePrimary(direction: 1 | -1 = 1) {
    if (!activeSession) return;
    const target = nextPreset(activePreset, PRIMARY_CYCLE, direction);
    await runAction(() => piClient.sendPiCommand(activeSession!.id, `/primary ${target}`));
  }


  async function scrollChatToBottom() {
    await tick();
    if (chatLogEl) chatLogEl.scrollTop = chatLogEl.scrollHeight;
  }

  async function refreshSessions() {
    sessions = await piClient.listSessions();
    const stillExists = sessions.some((session) => session.id === activeSessionId);
    if (!stillExists) {
      activeSessionId = sessions.find((session) => session.status === 'active')?.id ?? sessions[0]?.id ?? '';
    }
  }

  async function refreshPiImports() {
    try {
      pendingPiImports = await piClient.listPiImports();
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
    const session = await runAction(() => piClient.importPiSession(meta.session_file));
    piImportBusy = '';
    if (!session) return;
    sessions = [...sessions.filter((item) => item.id !== session.id), session];
    activeSessionId = session.id;
    activePane = 'chat';
    await refreshSessions();
    await refreshPiImports();
    await scrollChatToBottom();
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
    const session = await runAction(() => piClient.createSession(path || null));
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
    const ok = await runAction(() => piClient.switchSession(id));
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
    const ok = await runAction(() => piClient.closeSession(id));
    if (ok === undefined) return;
    await refreshSessions();
    await refreshPiImports();
  }

  async function send() {
    if (sendInFlight || !activeSession || !draft.trim()) return;
    sendInFlight = true;
    try {
      activePane = 'chat';
      const sessionId = activeSession.id;
      const content = draft.trim();
      if (content.startsWith('/') && await handleSlashCommand(content)) {
        draft = '';
        return;
      }
      const ok = await runAction(() => piClient.sendMessage(sessionId, content));
      if (ok !== undefined) draft = '';
    } finally {
      sendInFlight = false;
    }
  }

  async function steer() {
    if (steerInFlight || !activeSession || !draft.trim()) return;
    steerInFlight = true;
    try {
      const sessionId = activeSession.id;
      const content = draft.trim();
      const ok = await runAction(() => piClient.steerSession(sessionId, content));
      if (ok !== undefined) draft = '';
    } finally {
      steerInFlight = false;
    }
  }

  async function abort(kind: 'abort' | 'abort_bash' = 'abort') {
    if (!activeSession) return;
    await runAction(() => piClient.abortSession(activeSession!.id, kind));
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
      case 'clear':
        if (activeSession) await runAction(() => piClient.resetPiSession(activeSession!.id));
        return true;
      case 'compact':
        await runAction(() => piClient.compactSession(activeSession!.id, args || null));
        return true;
      case 'name':
        if (args) await runAction(() => piClient.renamePiSession(activeSession!.id, args));
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
        if (activeSession) await runAction(() => piClient.stopSession(activeSession!.id));
        return true;
      case 'quit':
        if (activeSession) await closeSession(activeSession.id);
        return true;
      default:
        return false;
    }
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
      const commands = await runAction(() => piClient.listPiCommands(sessionId));
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
    const current = extensionRequestQueue[0];
    if (!current) return;
    await runAction(() =>
      piClient.respondExtensionUi(current.session_id, current.request.id, response)
    );
    extensionRequestQueue = extensionRequestQueue.slice(1);
  }

  async function ensureModelOptions() {
    if (!activeSession || modelLoading || modelOptions.length) return;
    modelLoading = true;
    const models = await runAction(() => piClient.listPiModels(activeSession!.id));
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
    const model =
      modelOptions.find((item) => item.provider === provider && item.id === preferredId) ??
      modelOptions.find((item) => item.provider === provider);
    if (!model) return;
    activeChooser = null;
    const ok = await runAction(() =>
      piClient.setPiModel(activeSession!.id, model.provider, model.id)
    );
    if (ok !== undefined) {
      sessions = sessions.map((session) =>
        session.id === activeSession!.id
          ? {
              ...session,
              provider: model.provider,
              model: model.id,
              model_id: model.id,
              status: 'updating'
            }
          : session
      );
    }
  }

  async function selectModel(model: PiModelOption) {
    if (!activeSession) return;
    activeChooser = null;
    const ok = await runAction(() =>
      piClient.setPiModel(activeSession!.id, model.provider, model.id)
    );
    if (ok !== undefined) {
      sessions = sessions.map((session) =>
        session.id === activeSession!.id
          ? {
              ...session,
              provider: model.provider,
              model: model.id,
              model_id: model.id,
              status: 'updating'
            }
          : session
      );
    }
  }

  async function selectThinking(level: string) {
    if (!activeSession) return;
    activeChooser = null;
    const ok = await runAction(() => piClient.setPiThinkingLevel(activeSession!.id, level));
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
        const listeners = await attachPiEventListeners({
          onMessage: ({ session_id, message }) => {
            sessions = sessions.map((session) => {
              if (session.id !== session_id) return session;
              if (message.role === 'status') return { ...session, status: message.content || 'idle' };
              const isCanonical = Array.isArray(message.content_parts) && message.content_parts.length > 0;
              const existing = session.messages.find((item) => item.id === message.id);
              if (existing) {
                if (isCanonical) {
                  return {
                    ...session,
                    messages: session.messages.map((item) =>
                      item.id === message.id ? message : item
                    )
                  };
                }
                return {
                  ...session,
                  status: message.role === 'assistant' ? 'streaming' : session.status,
                  messages: session.messages.map((item) =>
                    item.id === message.id ? { ...item, content: item.content + message.content } : item
                  )
                };
              }
              return {
                ...session,
                status: message.role === 'assistant' && !isCanonical ? 'streaming' : session.status,
                messages: [...session.messages, message]
              };
            });
          },
          onSessionUpdate: ({ session: updated }) => {
            sessions = sessions.some((session) => session.id === updated.id)
              ? sessions.map((session) => (session.id === updated.id ? updated : session))
              : [...sessions, updated];
            if (!activeSessionId) activeSessionId = updated.id;
          },
          onExtensionRequest: (payload) => {
            extensionRequestQueue = [...extensionRequestQueue, payload];
          },
          onStatus: ({ session_id, statuses }) => {
            const next = new Map(sessionStatuses);
            next.set(session_id, statuses);
            sessionStatuses = next;
          },
          onWidget: ({ session_id, widgets }) => {
            const next = new Map(sessionWidgets);
            next.set(session_id, widgets);
            sessionWidgets = next;
          },
          onNotify: ({ session_id, message, level }) => {
            sessions = sessions.map((session) =>
              session.id === session_id
                ? {
                    ...session,
                    messages: [
                      ...session.messages,
                      {
                        id: `${session_id}-notify-${Date.now()}`,
                        role: 'status',
                        content: `${level}: ${stripAnsi(message)}`,
                        timestamp: Date.now()
                      }
                    ]
                  }
                : session
            );
          },
          onEditorText: ({ session_id, text }) => {
            if (activeSession && session_id === activeSession.id) {
              draft = text;
            }
          },
          onTitle: ({ session_id, title }) => {
            sessions = sessions.map((session) =>
              session.id === session_id ? { ...session, name: title || session.name } : session
            );
          },
          onSessionClosed: ({ session_id }) => {
            extensionRequestQueue = extensionRequestQueue.filter(
              (req) => req.session_id !== session_id
            );
          }
        });

        if (disposed) {
          listeners.forEach((cleanup) => cleanup());
          return;
        }
        unlisteners = listeners;
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
              <span class="pi-state" class:streaming={['streaming','thinking','generating','waiting','resetting','retrying','compacting'].includes(activeSession.status)}>{activeSession.status}</span>
            {/if}
            {#if activePane === 'chat' || activePane === 'home'}
              <button class="info-toggle" on:click={() => (infoCardVisible = !infoCardVisible)} title={infoCardVisible ? 'Hide info' : 'Show info'}>
                {infoCardVisible ? '↦' : '↤'}
              </button>
            {/if}
          </div>
        </div>

        {#if activePane === 'chat' && activeSession}
          <ChatPane
            activeSession={activeSession}
            activeChooser={activeChooser}
            {thinkingLevels}
            {modelLoading}
            {providerCounts}
            {activeProviderModels}
            {filteredModels}
            {visibleCommands}
            nonPresetStatuses={nonPresetStatuses}
            aboveWidgets={aboveWidgets}
            belowWidgets={belowWidgets}
            {error}
            bind:chatLogEl
            bind:draft
            bind:modelFilter
            onCloseChooser={() => (activeChooser = null)}
            onSelectThinking={selectThinking}
            onSelectProvider={selectProvider}
            onSelectModel={selectModel}
            onChooseCommand={chooseCommand}
            onRefreshCommandOptions={refreshCommandOptions}
            onEnsureCommandOptions={ensureCommandOptions}
            onSend={send}
            onSteer={steer}
            onAbort={abort}
          />

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
          <HomePane
            {activeSession}
            {homeStats}
            {visiblePiImports}
            {piImportBusy}
            {piImportsLoaded}
            sessionCount={sessions.length}
            onResumeSession={() => (activePane = 'chat')}
            onOpenFolder={pickProjectAndCreate}
            onNewSession={() => createSession()}
            onImportPiSession={importPiSession}
          />

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
                <small class="pi-state" class:streaming={['streaming','thinking','generating','waiting','resetting','retrying','compacting'].includes(activeSession?.status ?? '')}>{activeSession?.status ?? 'offline'}</small>
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

              <section class="info-section" aria-label="Pi telemetry">
                <div class="panel-head"><span class="eyebrow">Pi telemetry</span><small>{nonPresetStatuses.length + aboveWidgets.length + belowWidgets.length} items</small></div>
                {#if nonPresetStatuses.length || aboveWidgets.length || belowWidgets.length}
                  <div class="pi-status-feed side">
                    {#if nonPresetStatuses.length}
                      <div class="status-row side">
                        {#each nonPresetStatuses as status}
                          <span class="status-chip"><small>{status.key}</small><strong>{stripAnsi(status.text).trim()}</strong></span>
                        {/each}
                      </div>
                    {/if}
                    {#each [...aboveWidgets, ...belowWidgets] as widget (widget.key)}
                      <pre class="pi-widget" aria-label={`pi widget ${widget.key}`}>{widget.lines.map(stripAnsi).join('\n')}</pre>
                    {/each}
                  </div>
                {:else}
                  <p class="empty-note">No extension status or widgets yet.</p>
                {/if}
              </section>
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

  {#if activeExtensionRequest}
    <ExtensionRequestDialog
      extensionRequest={activeExtensionRequest}
      sessionLabel={extensionRequestSession?.name ?? activeExtensionRequest.session_id}
      pendingCount={extensionRequestQueue.length}
      onRespond={respondToExtensionRequest}
    />
  {/if}

  <RadialDock
    {groupedSessions}
    {activeSessionId}
    onOpenSession={openSession}
    onCloseSession={closeSession}
  />

  <div class="fps-overlay">fps: {fps}</div>
</main>
