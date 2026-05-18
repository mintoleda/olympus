<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { open } from '@tauri-apps/plugin-dialog';
  import ChatPane from './lib/components/ChatPane.svelte';
  import ExtensionRequestDialog from './lib/components/ExtensionRequestDialog.svelte';
  import RadialDock from './lib/components/RadialDock.svelte';
  import TitleBar from './lib/components/TitleBar.svelte';
  import { attachPiEventListeners } from './lib/services/piEvents';
  import { piClient } from './lib/services/piClient';
  import {
    PRIMARY_CYCLE,
    thinkingLevels,
    type ConfigChooser,
    type ExtensionUiRequest,
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
    stripAnsi,
    relativeTime,
    historyDate
  } from './lib/utils/pi';

  let sessions: PiSession[] = [];
  let activeSessionId = '';
  let draft = '';
  let error = '';
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
  let pendingPiImports: PiSessionMeta[] = [];
  let piImportsLoaded = false;
  let piImportBusy = '';
  let sendInFlight = false;
  let steerInFlight = false;

  $: activeSession = sessions.find((session) => session.id === activeSessionId) ?? sessions[0];
  $: groupedSessions = Object.entries(
    sessions.reduce<Record<string, PiSession[]>>((groups, session) => {
      (groups[session.project_path] ??= []).push(session);
      return groups;
    }, {})
  ).sort(([a], [b]) => a.localeCompare(b));
  $: recentSessions = [...sessions]
    .sort((a, b) => latestTimestamp(b) - latestTimestamp(a))
    .slice(0, 10);
  $: activeStatuses = activeSession ? sessionStatuses.get(activeSession.id) ?? [] : [];
  $: presetStatus = activeStatuses.find((entry) => entry.key === 'opencode-preset' || entry.key === 'preset');
  $: activePreset = parsePreset(presetStatus?.text);
  $: commandSearch = draft.startsWith('/') ? draft.slice(1).split(/\s+/, 1)[0].toLowerCase() : '';
  $: visibleCommands = draft.startsWith('/') ? rankCommands(commandOptions, commandSearch).slice(0, 12) : [];
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
  $: if (activeSession?.id) {
    const cached = commandCache.get(activeSession.id);
    commandOptions = cached ?? [];
    loadCommandOptions(activeSession.id);
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
  }

  async function closeSession(id: string) {
    sessions = sessions.filter((s) => s.id !== id);
    if (activeSessionId === id) {
      activeSessionId = sessions[0]?.id ?? '';
    }
    extensionRequestQueue = extensionRequestQueue.filter((r) => r.session_id !== id);
    sessionStatuses.delete(id);
    sessionWidgets.delete(id);
    sessionStatuses = sessionStatuses;
    sessionWidgets = sessionWidgets;

    piClient.closeSession(id).then(() => {
      refreshSessions();
      refreshPiImports();
    }).catch((err) => {
      error = String(err);
    });
  }

  async function send() {
    if (sendInFlight || !activeSession || !draft.trim()) return;
    sendInFlight = true;
    try {
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
      if (event.key === '+' || event.key === '=') { event.preventDefault(); setZoom(zoom + 0.05); return; }
      if (event.key === '-' || event.key === '_') { event.preventDefault(); setZoom(zoom - 0.05); return; }
      if (event.key === '0') { event.preventDefault(); setZoom(1); return; }
    }
    if (event.key === 'Tab' && event.shiftKey) {
      const promptEl = document.getElementById('prompt-input');
      if (!promptEl || document.activeElement !== promptEl) return;
      event.preventDefault();
      cyclePrimary(event.ctrlKey || event.metaKey ? -1 : 1);
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
      if (cached) { commandOptions = cached; return; }
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
    const ok = await runAction(() => piClient.setPiModel(activeSession!.id, model.provider, model.id));
    if (ok !== undefined) {
      sessions = sessions.map((session) =>
        session.id === activeSession!.id
          ? { ...session, provider: model.provider, model: model.id, model_id: model.id, status: 'updating' }
          : session
      );
    }
  }

  async function selectModel(model: PiModelOption) {
    if (!activeSession) return;
    activeChooser = null;
    const ok = await runAction(() => piClient.setPiModel(activeSession!.id, model.provider, model.id));
    if (ok !== undefined) {
      sessions = sessions.map((session) =>
        session.id === activeSession!.id
          ? { ...session, provider: model.provider, model: model.id, model_id: model.id, status: 'updating' }
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
                  return { ...session, messages: session.messages.map((item) => item.id === message.id ? message : item) };
                }
                return {
                  ...session,
                  status: message.role === 'assistant' ? 'streaming' : session.status,
                  messages: session.messages.map((item) => item.id === message.id ? { ...item, content: item.content + message.content } : item)
                };
              }
              return {
                ...session,
                status: message.role === 'assistant' && !isCanonical ? 'streaming' : session.status,
                messages: [...session.messages, message]
              };
            });
            scrollChatToBottom();
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
                ? { ...session, messages: [...session.messages, { id: `${session_id}-notify-${Date.now()}`, role: 'status', content: `${level}: ${stripAnsi(message)}`, timestamp: Date.now() }] }
                : session
            );
          },
          onEditorText: ({ session_id, text }) => {
            if (activeSession && session_id === activeSession.id) draft = text;
          },
          onTitle: ({ session_id, title }) => {
            sessions = sessions.map((session) =>
              session.id === session_id ? { ...session, name: title || session.name } : session
            );
          },
          onSessionClosed: ({ session_id }) => {
            extensionRequestQueue = extensionRequestQueue.filter((req) => req.session_id !== session_id);
          }
        });

        if (disposed) { listeners.forEach((cleanup) => cleanup()); return; }
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
    };
  });
</script>

<main class="void" bind:this={rootEl}>
  <TitleBar
    onSettingsClick={() => openConfigChooser('thinking')}
  />
  <section class="void-stage">
    <div class="app-layout">
      <!-- Main conversation card -->
      <div class="main-card">
        <!-- Tab bar -->
        <div class="tab-bar">
          {#each sessions as session}
            <div class="tab-wrapper" class:active={session.id === activeSessionId}>
              <button
                class="tab"
                class:active={session.id === activeSessionId}
                on:click={() => openSession(session.id)}
              >{#if session.id === activeSessionId}<span class="tab-dot"></span>{/if}{session.name}</button>
              <button
                class="tab-close"
                on:click|stopPropagation={() => closeSession(session.id)}
                title="Close session"
              >×</button>
            </div>
          {/each}
          <button class="tab tab-add" on:click={() => createSession()} title="New session">+</button>
        </div>

        <!-- Conversation -->
        {#if activeSession}
          <ChatPane
            activeSession={activeSession}
            activeChooser={activeChooser}
            {thinkingLevels}
            {modelLoading}
            {providerCounts}
            activeProviderModels={activeProviderModels}
            {filteredModels}
            {visibleCommands}
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
        {:else}
          <div class="empty-state">
            <svg class="empty-icon-svg" width="48" height="48" viewBox="0 0 48 48" fill="none">
              <path d="M8 10C8 8.895 8.895 8 10 8H38C39.105 8 40 8.895 40 10V30C40 31.105 39.105 32 38 32H18L12 38V32H10C8.895 32 8 31.105 8 30V10Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>
            </svg>
            <p class="empty-label">SEND A MESSAGE TO START</p>
          </div>
          <div class="card-footer">
            <div class="input-bar input-bar--tall">
              <textarea id="prompt-input" placeholder="Type your input here…" rows="3" disabled></textarea>
              <div class="input-bar-bottom">
                <div class="input-actions"></div>
                <button class="btn-send" disabled>Send</button>
              </div>
            </div>
          </div>
        {/if}
      </div>

      <!-- Sidebar -->
      <aside class="sidebar">
        <section class="sidebar-section">
          <h3 class="sidebar-heading">Model</h3>
          <button class="model-display" on:click={() => openConfigChooser('model')}>
            <span class="model-name">{activeSession?.model_id ?? activeSession?.model ?? 'No model'}</span>
            <svg class="model-chevron" width="10" height="10" viewBox="0 0 10 10" fill="none"><path d="M2.5 4L5 6.5L7.5 4" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round"/></svg>
          </button>
        </section>

        <section class="sidebar-section">
          <h3 class="sidebar-heading">Session metrics</h3>
          <div class="metrics-table">
            <div class="metric-row"><span>Status</span><span class="metric-value">{activeSession?.status ?? 'offline'}</span></div>
            <button class="metric-row clickable" on:click={() => openConfigChooser('provider')}><span>Provider</span><span class="metric-value">{activeSession?.provider ?? '—'}</span></button>
            <button class="metric-row clickable" on:click={() => openConfigChooser('thinking')}><span>Thinking</span><span class="metric-value">{activeSession?.thinking_level ?? 'default'}</span></button>
            <div class="metric-row"><span>Messages</span><span class="metric-value">{activeSession?.messages.length ?? 0}</span></div>
            <div class="metric-row"><span>Cost</span><span class="metric-value">${(activeSession?.cost ?? 0).toFixed(3)}</span></div>
            <div class="metric-row"><span>Input tokens</span><span class="metric-value">{activeSession?.input_tokens ?? 0}</span></div>
            <div class="metric-row"><span>Output tokens</span><span class="metric-value">{activeSession?.output_tokens ?? 0}</span></div>
            <div class="metric-row"><span>Total</span><span class="metric-value">{activeSession?.total_tokens ?? 0}</span></div>
          </div>
        </section>

        <section class="sidebar-section sidebar-history">
          <h3 class="sidebar-heading">History</h3>
          {#if recentSessions.length}
            <div class="history-list">
              {#each recentSessions as session}
                <button
                  class="history-item"
                  class:active={session.id === activeSessionId}
                  on:click={() => openSession(session.id)}
                >
                  <span class="history-dot"></span>
                  <div class="history-content">
                    <span class="history-name">{session.name}</span>
                    <span class="history-date">{historyDate(latestTimestamp(session))}</span>
                  </div>
                </button>
              {/each}
            </div>
          {:else}
            <p class="empty-note">No sessions yet</p>
          {/if}
        </section>
      </aside>
    </div>
  </section>

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
</main>
