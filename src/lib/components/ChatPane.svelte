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
    'streaming', 'thinking', 'generating', 'waiting', 'retrying', 'compacting'
  ]);
  let isStreaming = $derived(STREAMING_STATES.has(activeSession.status));
  let isInBash = $derived(
    typeof activeSession.status === 'string' && activeSession.status.startsWith('running:')
  );
  let canSubmit = $derived(draft.trim().length > 0);
  let hasMessages = $derived(activeSession.messages.length > 0);

  function handlePromptKeydown(event: KeyboardEvent) {
    if (event.key !== 'Enter') return;
    if (event.shiftKey) return;
    event.preventDefault();
    if (event.repeat) return;
    if (isStreaming) {
      if (canSubmit) onSteer();
      return;
    }
    onSend();
  }
</script>

<div class="conversation">
  <h3 class="conversation-heading">Conversation</h3>
  {#if hasMessages}
    <div class="chat-log" bind:this={chatLogEl}>
      {#each activeSession.messages as message, index}
        {#if message.role === 'system'}
          <div class="chat-separator"><span>{message.content}</span></div>
        {:else}
          <article class="message {message.role}">
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
  {:else}
    <div class="empty-state">
      <svg class="empty-icon" viewBox="0 0 48 48" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M8 10C8 8.89543 8.89543 8 10 8H38C39.1046 8 40 8.89543 40 10V30C40 31.1046 39.1046 32 38 32H18L12 38V32H10C8.89543 32 8 31.1046 8 30V10Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/>
      </svg>
      <p>Send a message to start</p>
    </div>
  {/if}
</div>

{#if activeChooser}
  <section class="config-popover panel" aria-label="Configuration">
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
      <p class="empty-note">Loading model registry…</p>
    {:else if activeChooser === 'provider'}
      <div class="choice-grid compact">
        {#each providerCounts as option}
          <button class:chosen={option.provider === activeSession?.provider} onclick={() => onSelectProvider(option.provider)}>
            <strong>{option.provider}</strong>
            <small>{option.count} models</small>
          </button>
        {/each}
      </div>
    {:else}
      <input class="model-search" bind:value={modelFilter} placeholder={`Filter ${activeSession?.provider ?? ''} models…`} />
      <div class="choice-grid models">
        {#each filteredModels as model}
          <button
            class:chosen={model.provider === activeSession?.provider && model.id === activeSession?.model_id}
            onclick={() => onSelectModel(model)}
          >
            <strong>{model.id}</strong>
            <small>{model.provider} · ctx {model.context} · out {model.max_output}</small>
          </button>
        {/each}
      </div>
    {/if}
  </section>
{/if}

{#if draft.startsWith('/')}
  <section class="slash-menu panel" aria-label="Commands">
    {#if visibleCommands.length}
      {#each visibleCommands as command}
        <button onclick={() => onChooseCommand(command)}>
          <strong>/{command.name}</strong>
          <small>{command.description || 'no description'}</small>
        </button>
      {/each}
    {:else}
      <p class="empty-note">No matching commands.</p>
    {/if}
  </section>
{/if}

<div class="card-footer">
  <div class="input-bar input-bar--tall">
    <textarea
      id="prompt-input"
      bind:value={draft}
      placeholder={isStreaming ? 'Steer the conversation…' : 'Type your input here…'}
      rows="3"
      oninput={onEnsureCommandOptions}
      onkeydown={handlePromptKeydown}
    ></textarea>
    <div class="input-bar-bottom">
      <div class="input-actions">
        <button class="input-action" title="Attach file">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21.44 11.05l-9.19 9.19a6 6 0 01-8.49-8.49l9.19-9.19a4 4 0 015.66 5.66l-9.2 9.19a2 2 0 01-2.83-2.83l8.49-8.48"/></svg>
        </button>
        <button class="input-action" title="Upload">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 2H6a2 2 0 00-2 2v16a2 2 0 002 2h12a2 2 0 002-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="12" y1="18" x2="12" y2="12"/><polyline points="9 15 12 12 15 15"/></svg>
        </button>
        <button class="input-action" title="Screenshot">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="3" y="3" width="18" height="18" rx="2" ry="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>
        </button>
        <button class="input-action" title="Paste">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M16 4h2a2 2 0 012 2v14a2 2 0 01-2 2H6a2 2 0 01-2-2V6a2 2 0 012-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg>
        </button>
        <button class="input-action" title="Voice">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 1a3 3 0 00-3 3v8a3 3 0 006 0V4a3 3 0 00-3-3z"/><path d="M19 10v2a7 7 0 01-14 0v-2"/><line x1="12" y1="19" x2="12" y2="23"/><line x1="8" y1="23" x2="16" y2="23"/></svg>
        </button>
      </div>
      {#if isStreaming}
        <button class="btn-abort" onclick={() => onAbort(isInBash ? 'abort_bash' : 'abort')}>Abort</button>
      {/if}
      <button class="btn-send" onclick={isStreaming ? onSteer : onSend} disabled={!canSubmit}>
        {isStreaming ? 'Steer' : 'Send'}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>
      </button>
    </div>
  </div>
  {#if error}<p class="error">{error}</p>{/if}
</div>
