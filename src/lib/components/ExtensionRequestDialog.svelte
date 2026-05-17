<script lang="ts">
  import type { ExtensionUiRequest } from '../types/pi';
  import { resolveCustomUI } from './customUI/registry';

  let {
    extensionRequest,
    sessionLabel,
    pendingCount,
    onRespond
  }: {
    extensionRequest: ExtensionUiRequest;
    sessionLabel: string;
    pendingCount: number;
    onRespond: (response: Record<string, any>) => void;
  } = $props();

  let editorValue = $state('');

  $effect(() => {
    if (extensionRequest.request.method === 'editor') {
      editorValue =
        typeof extensionRequest.request.value === 'string'
          ? extensionRequest.request.value
          : typeof extensionRequest.request.initial === 'string'
            ? extensionRequest.request.initial
            : '';
    }
  });

  function submitEditor() {
    onRespond({ value: editorValue });
  }

  function customComponentName(): string | undefined {
    const details = extensionRequest.request.details;
    if (details && typeof details === 'object' && 'component' in details) {
      const name = (details as { component: unknown }).component;
      return typeof name === 'string' ? name : undefined;
    }
    return undefined;
  }

  function customComponentProps(): Record<string, unknown> {
    const details = extensionRequest.request.details;
    if (details && typeof details === 'object' && 'props' in details) {
      const props = (details as { props: unknown }).props;
      return props && typeof props === 'object' ? (props as Record<string, unknown>) : {};
    }
    return {};
  }
</script>

<section class="global-dialog panel" aria-label="Pi request">
  <div class="panel-head">
    <div class="dialog-head-stack">
      <span>{extensionRequest.request.title ?? extensionRequest.request.method}</span>
      <small class="dialog-source">
        {sessionLabel}
        {#if pendingCount > 1}
          · {pendingCount - 1} more pending
        {/if}
      </small>
    </div>
    <button onclick={() => onRespond({ cancelled: true })}>Cancel</button>
  </div>
  {#if extensionRequest.request.method === 'confirm'}
    <p>{extensionRequest.request.message}</p>
    <div class="dialog-actions">
      <button onclick={() => onRespond({ confirmed: false })}>No</button>
      <button class="primary-action" onclick={() => onRespond({ confirmed: true })}>Yes</button>
    </div>
  {:else if extensionRequest.request.method === 'select'}
    <div class="choice-grid compact">
      {#each extensionRequest.request.options ?? [] as option}
        <button onclick={() => onRespond({ value: option.value ?? option.id ?? option })}
          ><strong>{option.label ?? option.name ?? option.value ?? option}</strong></button
        >
      {/each}
    </div>
  {:else if extensionRequest.request.method === 'input'}
    <input
      class="model-search"
      placeholder={extensionRequest.request.placeholder ?? ''}
      onkeydown={(event) =>
        event.key === 'Enter' && onRespond({ value: (event.currentTarget as HTMLInputElement).value })}
    />
  {:else if extensionRequest.request.method === 'editor'}
    <textarea
      class="editor-area"
      bind:value={editorValue}
      placeholder={extensionRequest.request.placeholder ?? ''}
      rows={12}
    ></textarea>
    <div class="dialog-actions">
      <button onclick={() => onRespond({ cancelled: true })}>Cancel</button>
      <button class="primary-action" onclick={submitEditor}>Save</button>
    </div>
  {:else if extensionRequest.request.method === 'custom'}
    {#if customComponentName() && resolveCustomUI(customComponentName()!)}
      {@const Comp = resolveCustomUI(customComponentName()!)!}
      <Comp props={customComponentProps()} {onRespond} />
    {:else}
      <p class="empty-note">
        Unknown custom UI component: <code>{customComponentName() ?? 'unspecified'}</code>
      </p>
      <pre>{JSON.stringify(extensionRequest.request.details, null, 2)}</pre>
      <div class="dialog-actions">
        <button class="primary-action" onclick={() => onRespond({ cancelled: true })}>Close</button>
      </div>
    {/if}
  {:else}
    <p>{JSON.stringify(extensionRequest.request)}</p>
  {/if}
</section>
