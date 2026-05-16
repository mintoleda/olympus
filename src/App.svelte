<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
  import {
    animateActiveNav,
    animateChatHistory,
    animateInspectorRefresh,
    animateLatestMessage,
    animateMetricTick,
    animatePaneChange,
    animateSessionRail,
    animateSessionStack,
    animateShellEnter,
    animateStreamingStatus,
    attachInteractionAnimations,
    createAppAnimationScope
  } from './animations';
  import type { Scope } from 'animejs';

  type PaneId = 'home' | 'chat' | 'search' | 'settings';
  type ChatMessage = { id: string; role: 'user' | 'assistant' | 'status'; content: string; timestamp: number };
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
  type SessionEvent = { session_id: string; message: ChatMessage };
  type SessionUpdateEvent = { session: PiSession };
  type PiModelOption = { provider: string; id: string; context: string; max_output: string; reasoning: boolean; images: boolean };
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
  let rootEl: HTMLElement;
  let chatLogEl: HTMLElement;
  let zoom = 1;
  let animationScope: Scope | undefined;
  let animationReady = false;
  let lastAnimatedPane: PaneId = activePane;
  let lastAnimatedCollapsed = sessionsCollapsed;
  let lastAnimatedMessageCount = 0;
  let lastAnimatedStatus = '';
  let lastAnimatedSessionId = '';
  let lastAnimatedSessionCount = 0;
  let lastAnimatedMetricKey = '';
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
  $: if (animationReady && rootEl && sessionsCollapsed !== lastAnimatedCollapsed) {
    lastAnimatedCollapsed = sessionsCollapsed;
    tick().then(() => animateSessionRail(rootEl, sessionsCollapsed));
  }
  $: if (animationReady && rootEl && activeSession?.id && activeSession.id !== lastAnimatedSessionId) {
    lastAnimatedSessionId = activeSession.id;
    tick().then(() => {
      animateChatHistory(rootEl);
      animateInspectorRefresh(rootEl);
      animateActiveNav(rootEl);
    });
  }
  $: if (animationReady && rootEl && sessions.length !== lastAnimatedSessionCount) {
    lastAnimatedSessionCount = sessions.length;
    tick().then(() => {
      animateSessionStack(rootEl);
      animateMetricTick(rootEl);
    });
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

  function handleZoomShortcut(event: KeyboardEvent) {
    if (!event.ctrlKey && !event.metaKey) return;
    if (event.key === '+' || event.key === '=') {
      event.preventDefault();
      setZoom(zoom + 0.05);
    } else if (event.key === '-' || event.key === '_') {
      event.preventDefault();
      setZoom(zoom - 0.05);
    } else if (event.key === '0') {
      event.preventDefault();
      setZoom(1);
    }
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
      case 'resume':
      case 'tree':
        sessionsCollapsed = false;
        return true;
      case 'quit':
        return true;
      default:
        return false;
    }
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
    window.addEventListener('keydown', handleZoomShortcut);
    const detachInteractions = attachInteractionAnimations(rootEl);
    animationScope = createAppAnimationScope(rootEl);
    animateShellEnter(rootEl, animationScope);
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
        if (disposed) {
          messageCleanup();
          sessionCleanup();
          return;
        }
        unlisteners = [messageCleanup, sessionCleanup];
        await refreshSessions();
        if (sessions.length === 0) await createSession();
      } catch (err) {
        error = String(err);
      }
    })();

    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
      window.removeEventListener('keydown', handleZoomShortcut);
      detachInteractions();
      animationScope?.revert();
    };
  });
</script>

<main class="ops-shell" bind:this={rootEl}>
  <aside class="nav-rail" aria-label="Primary navigation">
    <div class="product-mark"><span>OL</span><small>Olympus</small></div>
    <nav class="pane-tabs">
      {#each panes as pane}
        <button class:active={pane.id === activePane} on:click={() => { activePane = pane.id; if (pane.id !== 'chat') activeChooser = null; }}>
          <b>{pane.key}</b><span>{pane.label}</span>
        </button>
      {/each}
    </nav>
    <div class="rail-status"><span class="status-dot"></span><p>{sessions.length} mounted</p></div>
  </aside>

  <section class="workbench">
    <header class="topbar">
      <div class="crumbs"><span>workspace</span><strong>{activeProjectName}</strong><em>{active.label}</em></div>
      <div class="topbar-actions">
        <span class="pi-state" class:streaming={activeSession?.status === 'streaming'}>Pi: {activeSession?.status ?? 'offline'}</span>
        <button class="text-button" on:click={pickProjectAndCreate}>Open folder</button>
        <button class="solid-button" on:click={() => createSession()}>New session</button>
      </div>
    </header>

    <div class="layout" class:chat-layout={activePane === 'chat'} class:sessions-collapsed={activePane === 'chat' && sessionsCollapsed}>
      {#if activePane === 'chat'}
        {#if sessionsCollapsed}
          <button class="sessions-peek panel" on:click={() => (sessionsCollapsed = false)} aria-label="Expand session list">
            <span>Sessions</span><strong>{sessions.length}</strong>
          </button>
        {:else}
          <aside class="session-tree panel">
            <div class="panel-head">
              <button class="panel-title-button" on:click={() => (sessionsCollapsed = true)} aria-label="Collapse session list">Session stack</button>
              <div class="mini-actions">
                <button class="quiet-mini" on:click={pickProjectAndCreate}>Open</button>
                <button on:click={() => createSession()}>New</button>
              </div>
            </div>
            {#each groupedSessions as [project, projectSessions]}
              <p class="project-label">{project}</p>
              {#each projectSessions as session}
                <div class="session-row" class:chosen={session.id === activeSession?.id}>
                  <button class="session-select" on:click={() => switchSession(session.id)}>
                    <strong>{session.name}</strong><small>{session.project_path}</small><em>{session.status}</em>
                  </button>
                  <button class="close" aria-label={`Close ${session.name}`} on:click={() => closeSession(session.id)}>×</button>
                </div>
              {/each}
            {/each}
          </aside>
        {/if}
      {/if}

      <section class="main-panel panel" class:home-panel={activePane === 'home'} class:info-panel={activePane !== 'chat'}>
        {#if activePane === 'chat' && activeSession}
          <div class="transcript-head">
            <div><p class="eyebrow">Active transcript</p><h1>{activeSession.name}</h1></div>
            <span class:streaming={activeSession.status === 'streaming'}>{activeSession.status}</span>
          </div>
          <div class="chat-log" bind:this={chatLogEl}>
            {#each activeSession.messages as message, index}
              <article class="message {message.role}" style={`--i: ${index}`}>
                <header><span>{message.role}</span><time>{formatTime(message.timestamp)}</time></header>
                <p>{message.content}</p>
              </article>
            {/each}
          </div>
        {:else if activePane === 'home'}
          <div class="home-surface">
            <section class="launch-card">
              <p class="eyebrow">Local command center</p>
              <h1>Pick up the thread without opening another terminal.</h1>
              <p>Mount a project folder, resume a Pi context, or start a clean session. Olympus wraps Pi with transcript history, model/provider state, and session stack in one desktop surface.</p>
              <div class="home-actions" aria-label="Home quick actions">
                <button class="primary-action" on:click={() => (activePane = 'chat')} disabled={!activeSession}>Resume active session</button>
                <button on:click={pickProjectAndCreate}>Open project folder</button>
                <button on:click={() => createSession()}>New local session</button>
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

            <section class="recent-card">
              <div class="panel-head"><span>Recent work</span><small>{recentSessions.length} entries</small></div>
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
                <p class="empty-note">No sessions yet. Start one and it will appear here.</p>
              {/if}
            </section>
          </div>
        {:else}
          <div class="placeholder-copy">
            <p class="eyebrow">{active.key} / {active.label}</p>
            <h1>{active.description}</h1>
            <p>This surface will grow into a practical inspector for Pi, project state, permissions, and local tools. The shell is intentionally quiet until there is real state to show.</p>
          </div>
        {/if}

        {#if activePane === 'chat' && activeChooser}
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

        <div class="command-dock">
          <label for="prompt-input">Prompt</label>
          <input id="prompt-input" bind:value={draft} placeholder={activeSession ? `Ask Pi in ${activeSession.name}…` : 'Create a session first…'} on:keydown={(event) => event.key === 'Enter' && send()} />
          <button on:click={send}>Send</button>
        </div>
        {#if error}<p class="error">{error}</p>{/if}
      </section>

      {#if activePane === 'chat'}
        <aside class="inspector">
          <section class="panel runtime-panel">
            <div class="panel-head"><span>Pi wrapper</span><small>{activeSession?.status ?? 'offline'}</small></div>
            <div class="meters">
              {#each piRuntimeFacts as fact}
                <button class="meter-button" on:click={() => openConfigChooser(fact.key)}>
                  <span>{fact.label}</span><strong>{fact.value}</strong><small>{fact.note}</small>
                </button>
              {/each}
            </div>
          </section>
          <section class="panel context-card">
            <p class="eyebrow">Context</p>
            <h2>{activeSession?.name ?? 'No session'}</h2>
            <p>{activeSession?.project_path ?? 'Create a project-bound Pi session.'}</p>
          </section>
        </aside>
      {/if}
    </div>
  </section>
</main>
