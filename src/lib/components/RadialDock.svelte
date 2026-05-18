<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { animate, stagger } from 'animejs';
  import type { PiSession } from '../types/pi';

  let {
    groupedSessions,
    activeSessionId,
    onOpenSession,
    onCloseSession
  }: {
    groupedSessions: [string, PiSession[]][];
    activeSessionId: string;
    onOpenSession: (id: string) => void;
    onCloseSession: (id: string) => void;
  } = $props();

  let active = $state(false);
  let center = $state({ x: 0, y: 0 });
  let cursor = $state({ x: 0, y: 0 });
  let hoveredFolder = $state<string | null>(null);
  let hoveredSession = $state<string | null>(null);
  let closeZone = $state(false);
  let layoutScale = $state(1);

  const INNER_RADIUS = 90;
  const OUTER_RADIUS = 170;
  const CLOSE_THRESHOLD = 230;
  const SCREEN_MARGIN = 16;
  const MAX_NODE_WIDTH = 152;
  const MAX_NODE_HEIGHT = 48;
  const NODE_ANCHOR_OFFSET = 18;
  const HOVER_SHADOW = 20;

  function folderLabel(path: string) {
    return path.split('/').filter(Boolean).at(-1) ?? path;
  }

  const FOLDER_COLORS = ['#c6f36d', '#ff8a3d', '#7dd3fc', '#c084fc', '#fb7185', '#fbbf24'];

  function folderColor(index: number) {
    return FOLDER_COLORS[index % FOLDER_COLORS.length];
  }

  function clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function layoutBounds() {
    return {
      left: OUTER_RADIUS + NODE_ANCHOR_OFFSET + HOVER_SHADOW,
      right: OUTER_RADIUS - NODE_ANCHOR_OFFSET + MAX_NODE_WIDTH + HOVER_SHADOW,
      top: OUTER_RADIUS + NODE_ANCHOR_OFFSET + HOVER_SHADOW,
      bottom: OUTER_RADIUS - NODE_ANCHOR_OFFSET + MAX_NODE_HEIGHT + HOVER_SHADOW
    };
  }

  function fitActivationPoint(x: number, y: number) {
    const bounds = layoutBounds();
    const availableWidth = Math.max(1, window.innerWidth - SCREEN_MARGIN * 2);
    const availableHeight = Math.max(1, window.innerHeight - SCREEN_MARGIN * 2);
    const scale = Math.max(
      0.05,
      Math.min(1, availableWidth / (bounds.left + bounds.right), availableHeight / (bounds.top + bounds.bottom))
    );
    const minX = Math.min(bounds.left * scale + SCREEN_MARGIN, window.innerWidth / 2);
    const maxX = Math.max(window.innerWidth - bounds.right * scale - SCREEN_MARGIN, window.innerWidth / 2);
    const minY = Math.min(bounds.top * scale + SCREEN_MARGIN, window.innerHeight / 2);
    const maxY = Math.max(window.innerHeight - bounds.bottom * scale - SCREEN_MARGIN, window.innerHeight / 2);

    return {
      center: { x: clamp(x, minX, maxX), y: clamp(y, minY, maxY) },
      scale
    };
  }

  function localPointer(clientX: number, clientY: number) {
    const scale = layoutScale || 1;
    const dx = (clientX - center.x) / scale;
    const dy = (clientY - center.y) / scale;
    return { dx, dy, dist: Math.hypot(dx, dy) };
  }

  function activateAt(x: number, y: number) {
    const fitted = fitActivationPoint(x, y);
    center = fitted.center;
    layoutScale = fitted.scale;
    active = true;
    hoveredFolder = null;
    hoveredSession = null;
    closeZone = false;

    requestAnimationFrame(() => {
      const nodes = document.querySelectorAll('.radial-node');
      if (nodes.length) {
        animate(Array.from(nodes), {
          opacity: [0, 1],
          scale: [0.3, 1],
          delay: stagger(25),
          duration: 180,
          ease: 'out(3)'
        });
      }
    });
  }

  function getFolderPositions() {
    const count = groupedSessions.length;
    return groupedSessions.map(([path], i) => {
      const angle = (2 * Math.PI * i) / Math.max(count, 1) - Math.PI / 2;
      return {
        path,
        x: Math.cos(angle) * INNER_RADIUS,
        y: Math.sin(angle) * INNER_RADIUS,
        angle
      };
    });
  }

  function getSessionPositions(folderPath: string) {
    const folderIndex = groupedSessions.findIndex(([p]) => p === folderPath);
    if (folderIndex < 0) return [];
    const [, sessions] = groupedSessions[folderIndex];
    const baseAngle = (2 * Math.PI * folderIndex) / Math.max(groupedSessions.length, 1) - Math.PI / 2;
    const spread = Math.PI / 3;
    const count = sessions.length;

    return sessions.map((session, i) => {
      const offset = count === 1 ? 0 : ((i / (count - 1)) - 0.5) * spread;
      const angle = baseAngle + offset;
      return {
        session,
        x: Math.cos(angle) * OUTER_RADIUS,
        y: Math.sin(angle) * OUTER_RADIUS,
        angle
      };
    });
  }

  function handleKeyDown(e: KeyboardEvent) {
    if ((e.metaKey || e.ctrlKey) && e.key === 'e') {
      e.preventDefault();
      if (active) {
        dismiss();
        return;
      }
      activateAt(cursor.x || window.innerWidth / 2, cursor.y || window.innerHeight / 2);
    }

    if (e.key === 'Escape' && active) {
      dismiss();
    }
  }

  function handleClick(e: MouseEvent) {
    if (!active) return;
    const { dist } = localPointer(e.clientX, e.clientY);
    if (closeZone && hoveredSession) {
      onCloseSession(hoveredSession);
      dismiss();
    } else if (hoveredSession) {
      onOpenSession(hoveredSession);
      dismiss();
    } else if (dist < 30) {
      dismiss();
    }
  }

  function handleMouseMove(e: MouseEvent) {
    cursor = { x: e.clientX, y: e.clientY };
    if (!active) return;

    const { dx, dy, dist } = localPointer(e.clientX, e.clientY);
    closeZone = dist > CLOSE_THRESHOLD && hoveredSession !== null;

    if (dist > INNER_RADIUS * 0.5 && dist < (INNER_RADIUS + OUTER_RADIUS) / 2) {
      let closest: string | null = null;
      let closestDist = Infinity;
      for (const fp of getFolderPositions()) {
        const d = Math.hypot(dx - fp.x, dy - fp.y);
        if (d < closestDist && d < 60) {
          closestDist = d;
          closest = fp.path;
        }
      }
      if (closest !== hoveredFolder) {
        hoveredFolder = closest;
        hoveredSession = null;
      }
    }

    if (hoveredFolder && dist > (INNER_RADIUS + OUTER_RADIUS) / 2 - 20) {
      let closest: string | null = null;
      let closestDist = Infinity;
      for (const sp of getSessionPositions(hoveredFolder)) {
        const d = Math.hypot(dx - sp.x, dy - sp.y);
        if (d < closestDist && d < 50) {
          closestDist = d;
          closest = sp.session.id;
        }
      }
      hoveredSession = closest;
    }
  }

  function dismiss() {
    const nodes = document.querySelectorAll('.radial-node');
    if (nodes.length) {
      animate(Array.from(nodes), {
        opacity: [1, 0],
        scale: [1, 0.5],
        duration: 120,
        ease: 'in(2)',
        onComplete: () => { active = false; }
      });
    } else {
      active = false;
    }
    hoveredFolder = null;
    hoveredSession = null;
    closeZone = false;
  }

  function handleResize() {
    if (!active) return;
    const fitted = fitActivationPoint(center.x, center.y);
    center = fitted.center;
    layoutScale = fitted.scale;
  }

  onMount(() => {
    window.addEventListener('keydown', handleKeyDown);
    window.addEventListener('mousemove', handleMouseMove);
    window.addEventListener('click', handleClick);
    window.addEventListener('resize', handleResize);
  });

  onDestroy(() => {
    window.removeEventListener('keydown', handleKeyDown);
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('click', handleClick);
    window.removeEventListener('resize', handleResize);
  });
</script>

{#if active}
  <div class="radial-overlay" class:close-zone={closeZone}>
    <div class="radial-center" style="left: {center.x}px; top: {center.y}px; --radial-scale: {layoutScale}">
      <!-- Inner ring: folders -->
      {#each getFolderPositions() as fp, i}
        <div
          class="radial-node folder-node"
          class:hovered={hoveredFolder === fp.path}
          style="transform: translate({fp.x}px, {fp.y}px); --node-color: {folderColor(i)}"
        >
          <span class="radial-folder-letter">{folderLabel(fp.path).charAt(0).toUpperCase()}</span>
          <span class="radial-folder-name">{folderLabel(fp.path)}</span>
        </div>
      {/each}

      <!-- Outer ring: sessions for hovered folder -->
      {#if hoveredFolder}
        {#each getSessionPositions(hoveredFolder) as sp}
          <div
            class="radial-node session-node"
            class:hovered={hoveredSession === sp.session.id}
            class:is-active={sp.session.id === activeSessionId}
            class:will-close={closeZone && hoveredSession === sp.session.id}
            style="transform: translate({sp.x}px, {sp.y}px)"
          >
            <span class="radial-session-dot"></span>
            <span class="radial-session-name">{sp.session.name}</span>
          </div>
        {/each}
      {/if}

      <!-- Center indicator -->
      <div class="radial-hub">
        {#if closeZone && hoveredSession}
          <span class="hub-label danger" style="--hub-chars: 5">close</span>
        {:else if hoveredSession}
          <span class="hub-label" style="--hub-chars: 4">open</span>
        {:else}
          <span class="hub-label dim" style="--hub-chars: 8">sessions</span>
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .radial-overlay {
    position: fixed;
    inset: 0;
    z-index: 9999;
    backdrop-filter: blur(6px);
    background: rgba(20, 20, 18, 0.5);
    overflow: hidden;
  }

  .radial-overlay.close-zone {
    background: rgba(20, 20, 18, 0.6);
  }

  .radial-center {
    position: absolute;
    transform: translate(-50%, -50%) scale(var(--radial-scale));
    transform-origin: center;
  }

  .radial-hub {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--base-2);
    border: 1.5px solid var(--line);
    display: grid;
    place-items: center;
  }

  .hub-label {
    font-size: calc(48px / var(--hub-chars, 5));
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--accent);
    font-weight: 600;
    white-space: nowrap;
  }

  .hub-label.dim { color: var(--faint); }
  .hub-label.danger { color: var(--danger); }

  .radial-node {
    position: absolute;
    top: 50%;
    left: 50%;
    margin-top: -18px;
    margin-left: -18px;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border-radius: 20px;
    background: var(--panel);
    border: 1.5px solid var(--line);
    white-space: nowrap;
    pointer-events: none;
    transition: transform 80ms ease, border-color 100ms, box-shadow 100ms, background 100ms;
  }

  .folder-node {
    padding: 8px 14px;
    border-color: color-mix(in srgb, var(--node-color) 25%, var(--line));
  }

  .folder-node.hovered {
    border-color: var(--node-color);
    box-shadow: 0 0 16px color-mix(in srgb, var(--node-color) 30%, transparent);
    background: color-mix(in srgb, var(--node-color) 8%, var(--panel));
  }

  .radial-folder-letter {
    width: 22px;
    height: 22px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--node-color) 15%, var(--base-3));
    color: var(--node-color);
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 11px;
  }

  .radial-folder-name {
    font-size: 11px;
    color: var(--paper);
    font-weight: 500;
    max-width: 80px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .session-node.hovered {
    border-color: var(--accent);
    box-shadow: 0 0 14px var(--glow);
    background: color-mix(in srgb, var(--accent) 6%, var(--panel));
  }

  .session-node.is-active {
    border-color: var(--accent);
  }

  .session-node.will-close {
    border-color: var(--danger);
    box-shadow: 0 0 14px rgba(255, 123, 114, 0.25);
    background: rgba(255, 123, 114, 0.08);
  }

  .radial-session-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--faint);
    flex-shrink: 0;
  }

  .session-node.is-active .radial-session-dot { background: var(--accent); }
  .session-node.will-close .radial-session-dot { background: var(--danger); }

  .radial-session-name {
    font-size: 11px;
    color: var(--paper);
    max-width: 90px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
