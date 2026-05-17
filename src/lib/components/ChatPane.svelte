<script lang="ts">
  import type {
    ConfigChooser,
    PiCommandOption,
    PiModelOption,
    PiSession
  } from '../types/pi';
  import { formatTime } from '../utils/pi';

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
    onSend,
    onSteer,
    onAbort
  }: {
    activeSession: PiSession;
    activeChooser: ConfigChooser;
    thinkingLevels: string[];
    modelLoading: boolean;
    providerCounts: ProviderCount[];
    activeProviderModels: PiModelOption[];
    filteredModels: PiModelOption[];
    visibleCommands: PiCommandOption[];
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
    onSteer: () => void;
    onAbort: (kind?: 'abort' | 'abort_bash') => void;
  } = $props();

  const STREAMING_STATES = new Set([
    'streaming',
    'thinking',
    'generating',
    'waiting',
    'retrying',
    'compacting'
  ]);
  let isStreaming = $derived(STREAMING_STATES.has(activeSession.status));
  let isInBash = $derived(
    typeof activeSession.status === 'string' && activeSession.status.startsWith('running:')
  );
  let canSubmit = $derived(draft.trim().length > 0);

  function handlePromptKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    event.preventDefault();
    if (event.repeat) return;
    if (isStreaming) {
      if (canSubmit) onSteer();
      return;
    }
    onSend();
  }
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
          {:else if message.content_parts && message.content_parts.length}
            {#each message.content_parts as part}
              {#if part.type === 'text'}
                <p>{part.text}</p>
              {:else if part.type === 'tool_use'}
                <details class="tool-call-block">
                  <summary>tool · {part.name}</summary>
                  <pre>{JSON.stringify(part.input, null, 2)}</pre>
                </details>
              {:else if part.type === 'tool_result'}
                <details class="tool-result-block" class:error={part.is_error}>
                  <summary>result{part.is_error ? ' · error' : ''}</summary>
                  <pre>{typeof part.content === 'string' ? part.content : JSON.stringify(part.content, null, 2)}</pre>
                </details>
              {:else if part.type === 'custom'}
                <p class="custom-part"><em>custom:{part.customType}</em></p>
              {/if}
            {/each}
          {:else}
            <p>{message.content}</p>
          {/if}
        </article>
      {/if}
    {/each}
  </div>
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
      placeholder={isStreaming ? `Steer Pi in ${activeSession.name}…` : `Ask Pi in ${activeSession.name}…`}
      oninput={onEnsureCommandOptions}
      onkeydown={handlePromptKeydown}
    />
    {#if isStreaming}
      <button onclick={() => onAbort(isInBash ? 'abort_bash' : 'abort')} title={isInBash ? 'Abort running bash' : 'Abort current turn'}>Abort</button>
      <button onclick={onSteer} disabled={!canSubmit} title="Steer the in-flight response">Steer</button>
    {:else}
      <button onclick={onSend} disabled={!canSubmit}>Send</button>
    {/if}
  </div>
  {#if error}<p class="error">{error}</p>{/if}
</div>
