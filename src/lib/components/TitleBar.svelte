<script lang="ts">
  import { getCurrentWindow } from '@tauri-apps/api/window';

  let { onMenuClick, onSettingsClick }: {
    onMenuClick?: () => void;
    onSettingsClick?: () => void;
  } = $props();

  const appWindow = getCurrentWindow();

  function stopTitlebarDrag(event: MouseEvent | PointerEvent) {
    event.stopPropagation();
  }

  function handleTitlebarMouseDown(event: MouseEvent) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement | null)?.closest('button, a, input, textarea, select')) return;
    void appWindow.startDragging().catch(console.error);
  }

  function runWindowAction(action: () => Promise<void>) {
    void action().catch(console.error);
  }

  function handleMenuClick(event: MouseEvent) {
    event.stopPropagation();
    onMenuClick?.();
  }

  function handleSettingsClick(event: MouseEvent) {
    event.stopPropagation();
    onSettingsClick?.();
  }
</script>

<div class="title-bar" role="toolbar" aria-label="Window controls" tabindex="-1" onmousedown={handleTitlebarMouseDown}>
  <div class="title-bar-left">
    <button class="title-bar-btn title-bar-menu" onpointerdown={stopTitlebarDrag} onclick={handleMenuClick}>
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
        <path d="M2 4h12M2 8h12M2 12h12" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
      </svg>
      <span>Menu</span>
    </button>
    <button class="title-bar-btn" onpointerdown={stopTitlebarDrag} onclick={handleSettingsClick} title="Settings">
      <svg width="14" height="14" viewBox="0 0 16 16" fill="none">
        <circle cx="8" cy="8" r="2.5" stroke="currentColor" stroke-width="1.3"/>
        <path d="M8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.05 3.05l1.41 1.41M11.54 11.54l1.41 1.41M3.05 12.95l1.41-1.41M11.54 4.46l1.41-1.41" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
      </svg>
    </button>
  </div>

  <div class="title-bar-right">
    <button class="title-bar-btn window-ctrl" onpointerdown={stopTitlebarDrag} onclick={(event) => { event.stopPropagation(); runWindowAction(() => appWindow.minimize()); }} title="Minimize">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M2 6h8" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
      </svg>
    </button>
    <button class="title-bar-btn window-ctrl" onpointerdown={stopTitlebarDrag} onclick={(event) => { event.stopPropagation(); runWindowAction(() => appWindow.toggleMaximize()); }} title="Maximize">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <rect x="2" y="2" width="8" height="8" rx="1.5" stroke="currentColor" stroke-width="1.3"/>
      </svg>
    </button>
    <button class="title-bar-btn window-ctrl window-close" onpointerdown={stopTitlebarDrag} onclick={(event) => { event.stopPropagation(); runWindowAction(() => appWindow.close()); }} title="Close">
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
      </svg>
    </button>
  </div>
</div>
