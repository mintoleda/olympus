<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';

  type PaneId = 'home' | 'chat' | 'widgets' | 'search' | 'settings';
  type ChatMessage = { id: string; role: 'user' | 'assistant' | 'status'; content: string; timestamp: number };
  type PiSession = { id: string; name: string; project_path: string; status: string; messages: ChatMessage[] };
  type SessionEvent = { session_id: string; message: ChatMessage };

  const panes: Array<{ id: PaneId; label: string; key: string; description: string }> = [
    { id: 'home', label: 'Atrium', key: '01', description: 'A calm launch surface for Pi sessions, widgets, and command routing.' },
    { id: 'chat', label: 'Sessions', key: '02', description: 'Live headless Pi conversations grouped by project.' },
    { id: 'widgets', label: 'Modules', key: '03', description: 'Sandboxed AI tools and local system telemetry.' },
    { id: 'search', label: 'Index', key: '04', description: 'A future global search plane for files, chats, commands, and widgets.' },
    { id: 'settings', label: 'Control', key: '05', description: 'Preferences, permissions, theme, layout, and platform details.' }
  ];

  const widgets = [
    { label: 'CPU', value: '11%', note: 'quiet' },
    { label: 'Memory', value: '2.8G', note: 'steady' },
    { label: 'Pi', value: 'ready', note: 'headless' }
  ];

  let activePane: PaneId = 'home';
  let sessions: PiSession[] = [];
  let activeSessionId = '';
  let draft = '';
  let error = '';
  $: active = panes.find((pane) => pane.id === activePane) ?? panes[0];
  $: activeSession = sessions.find((session) => session.id === activeSessionId) ?? sessions[0];
  $: groupedSessions = Object.entries(
    sessions.reduce<Record<string, PiSession[]>>((groups, session) => {
      (groups[session.project_path] ??= []).push(session);
      return groups;
    }, {})
  );

  async function refreshSessions() {
    sessions = await invoke<PiSession[]>('list_sessions');
    activeSessionId = sessions.find((session) => session.status === 'active')?.id ?? sessions[0]?.id ?? '';
  }

  async function createSession(path?: string) {
    error = '';
    const session = await invoke<PiSession>('create_session', { projectPath: path || null });
    sessions = [...sessions.filter((item) => item.id !== session.id), session];
    activeSessionId = session.id;
    activePane = 'chat';
    await refreshSessions();
  }

  async function pickProjectAndCreate() {
    error = '';
    const selected = await open({ directory: true, multiple: false, title: 'Choose a project folder' });
    if (typeof selected !== 'string') return;
    await createSession(selected);
  }

  async function switchSession(id: string) {
    await invoke('switch_session', { id });
    activeSessionId = id;
    await refreshSessions();
  }

  async function closeSession(id: string) {
    await invoke('close_session', { id });
    await refreshSessions();
  }

  async function send() {
    if (!activeSession || !draft.trim()) return;
    await invoke('send_message', { id: activeSession.id, content: draft });
    draft = '';
  }

  onMount(async () => {
    const unlisten = await listen<SessionEvent>('pi://message', (event) => {
      const { session_id, message } = event.payload;
      sessions = sessions.map((session) => {
        if (session.id !== session_id) return session;
        if (message.role === 'status') return { ...session, status: 'idle' };
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

    try {
      await refreshSessions();
      if (sessions.length === 0) await createSession();
    } catch (err) {
      error = String(err);
    }

    return unlisten;
  });
</script>

<main class="observatory">
  <aside class="spine" aria-label="Primary navigation">
    <div class="seal"><span>Ω</span><small>Olympus</small></div>
    <nav class="pane-tabs">
      {#each panes as pane}
        <button class:active={pane.id === activePane} on:click={() => (activePane = pane.id)}><b>{pane.key}</b><span>{pane.label}</span></button>
      {/each}
    </nav>
    <div class="spine-footer"><span class="pulse"></span><p>{sessions.length} Pi sessions</p></div>
  </aside>

  <section class="stage">
    <header class="masthead">
      <div><p class="kicker">Pi workspace</p><h1>{active.label}</h1></div>
      <button class="ghost-button" on:click={pickProjectAndCreate}>＋ New session</button>
    </header>

    <div class="layout">
      <aside class="project-tabs panel">
        <div class="panel-head"><span>Projects</span><button on:click={pickProjectAndCreate}>＋</button></div>
        {#each groupedSessions as [project, projectSessions]}
          <p class="project-label">{project}</p>
          {#each projectSessions as session}
            <button class:chosen={session.id === activeSession?.id} on:click={() => switchSession(session.id)}>
              <strong>{session.name}</strong><small>{session.project_path}</small><em>{session.status}</em>
              <span class="close" role="button" tabindex="0" on:click|stopPropagation={() => closeSession(session.id)} on:keydown|stopPropagation={(event) => (event.key === 'Enter' || event.key === ' ') && closeSession(session.id)}>×</span>
            </button>
          {/each}
        {/each}
      </aside>

      <section class="hero panel chat-hero">
        {#if activePane === 'chat' && activeSession}
          <div class="chat-log">
            {#each activeSession.messages as message}
              <article class="message {message.role}"><span>{message.role}</span><p>{message.content}</p></article>
            {/each}
          </div>
        {:else}
          <div class="hero-copy"><p class="kicker">{active.key} / {active.label}</p><h2>{active.description}</h2><p>Phase 2 adds a real Rust session manager behind this shell. Create, switch, close, and stream independent Pi conversations without leaving the hub.</p></div>
        {/if}

        <div class="command-dock">
          <input bind:value={draft} placeholder={activeSession ? `Ask Pi in ${activeSession.name}…` : 'Create a session first…'} on:keydown={(event) => event.key === 'Enter' && send()} />
          <button on:click={send}>Send ↗</button>
        </div>
        {#if error}<p class="error">{error}</p>{/if}
      </section>

      <aside class="right-rail">
        <section class="panel widget-panel"><div class="panel-head"><span>Telemetry</span><small>live mock</small></div><div class="meters">{#each widgets as widget}<article><span>{widget.label}</span><strong>{widget.value}</strong><small>{widget.note}</small></article>{/each}</div></section>
        <section class="panel permission-card"><p class="kicker">Session model</p><h3>{activeSession?.name ?? 'No session'}</h3><p>{activeSession?.project_path ?? 'Create a project-bound Pi session.'}</p></section>
      </aside>
    </div>
  </section>
</main>
