<script lang="ts">
  import type { ExtensionUiRequest } from '../types/pi';

  let {
    extensionRequest,
    onRespond
  }: {
    extensionRequest: ExtensionUiRequest;
    onRespond: (response: Record<string, any>) => void;
  } = $props();
</script>

<section class="global-dialog panel" aria-label="Pi request">
  <div class="panel-head">
    <span>{extensionRequest.request.title ?? extensionRequest.request.method}</span>
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
  {:else}
    <p>{JSON.stringify(extensionRequest.request)}</p>
  {/if}
</section>
