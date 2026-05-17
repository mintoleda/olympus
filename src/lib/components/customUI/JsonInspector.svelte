<script lang="ts">
  let {
    props,
    onRespond
  }: {
    props: Record<string, unknown>;
    onRespond: (response: Record<string, any>) => void;
  } = $props();
</script>

<div class="custom-json-inspector">
  <pre>{JSON.stringify(props, null, 2)}</pre>
  <div class="dialog-actions">
    <button onclick={() => onRespond({ cancelled: true })}>Close</button>
    <button class="primary-action" onclick={() => onRespond({ acknowledged: true })}>OK</button>
  </div>
</div>
