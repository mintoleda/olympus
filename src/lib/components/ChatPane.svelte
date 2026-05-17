<script lang="ts">
  import type {
    ConfigChooser,
    PiCommandOption,
    PiModelOption,
    PiSession,
    StatusEntry,
    WidgetEntry
  } from '../types/pi';
  import { formatTime, stripAnsi } from '../utils/pi';

  type ProviderCount = { provider: string; count: number };

  let {
    activeSession,
    activeChooser,
    thinkingLevels,
    modelLoading,
    providerCounts,
    activeProviderModels,
    filteredModels,
    visibleCommands,
    nonPresetStatuses,
    aboveWidgets,
    belowWidgets,
    error,
    chatLogEl = $bindable<HTMLElement>(),
    draft = $bindable<string>(),
    modelFilter = $bindable<string>(),
    onCloseChooser,
    onSelectThinking,
    onSelectProvider,
    onSelectModel,
    onChooseCommand,
    onRefreshCommandOptions,
    onEnsureCommandOptions,
    onSend
  }: {
    activeSession: PiSession;
    activeChooser: ConfigChooser;
    thinkingLevels: string[];
    modelLoading: boolean;
    providerCounts: ProviderCount[];
    activeProviderModels: PiModelOption[];
    filteredModels: PiModelOption[];
    visibleCommands: PiCommandOption[];
    nonPresetStatuses: StatusEntry[];
    aboveWidgets: WidgetEntry[];
    belowWidgets: WidgetEntry[];
    error: string;
    chatLogEl?: HTMLElement;
    draft: string;
    modelFilter: string;
    onCloseChooser: () => void;
    onSelectThinking: (level: string) => void;
    onSelectProvider: (provider: string) => void;
    onSelectModel: (model: PiModelOption) => void;
    onChooseCommand: (command: PiCommandOption) => void;
    onRefreshCommandOptions: () => void;
    onEnsureCommandOptions: () => void;
    onSend: () => void;
  } = $props();
</script>

<div class="chat-body">
  <div class="transcript-head">
    <div><p class="eyebrow">Active transcript</p><p class="session-name">{activeSession.name}</p></div>
    <span
      class="status-pill"
      class:streaming={['streaming', 'thinking', 'generating', 'waiting', 'resetting', 'retrying', 'compacting'].includes(activeSession.status)}
      >{activeSession.status}</span
    >
  </div>
  <div class="chat-log" bind:this={chatLogEl}>
    {#each activeSession.messages as message, index}
      {#if message.role === 'system'}
        <div class="chat-separator" style={`--i: ${index}`}><span>{message.content}</span></div>
      {:else}
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
      {/if}
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
      <button onclick={onCloseChooser}>Close</button>
    </div>
    {#if activeChooser === 'thinking'}
      <div class="choice-grid compact">
        {#each thinkingLevels as level}
          <button class:chosen={level === activeSession?.thinking_level} onclick={() => onSelectThinking(level)}>
            <strong>{level}</strong>
          </button>
        {/each}
      </div>
    {:else if modelLoading}
      <p class="empty-note">Loading Pi model registry…</p>
    {:else if activeChooser === 'provider'}
      <div class="choice-grid compact">
        {#each providerCounts as option}
          <button class:chosen={option.provider === activeSession?.provider} onclick={() => onSelectProvider(option.provider)}>
            <strong>{option.provider}</strong>
            <small>{option.count} models available</small>
          </button>
        {/each}
      </div>
    {:else}
      <input
        class="model-search"
        bind:value={modelFilter}
        placeholder={`Filter ${activeSession?.provider ?? 'current provider'} models…`}
      />
      {#if activeProviderModels.length && !modelFilter}
        <p class="chooser-hint">Showing {activeProviderModels.length} models for {activeSession?.provider}.</p>
      {:else if !activeProviderModels.length}
        <p class="chooser-hint">No models found for the current provider yet.</p>
      {/if}
      <div class="choice-grid models">
        {#each filteredModels as model}
          <button
            class:chosen={model.provider === activeSession?.provider && model.id === activeSession?.model_id}
            onclick={() => onSelectModel(model)}
          >
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
        <button onclick={() => onChooseCommand(command)}>
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
    <button class="slash-refresh" onclick={onRefreshCommandOptions} type="button">Refresh commands</button>
  </section>
{/if}

<div class="card-footer">
  <div class="command-dock">
    <label for="prompt-input">Prompt</label>
    <input
      id="prompt-input"
      bind:value={draft}
      placeholder={`Ask Pi in ${activeSession.name}…`}
      oninput={onEnsureCommandOptions}
      onkeydown={(event) => event.key === 'Enter' && onSend()}
    />
    <button onclick={onSend}>Send</button>
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
