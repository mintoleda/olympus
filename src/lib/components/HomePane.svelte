<script lang="ts">
  import type { PiSession, PiSessionMeta } from '../types/pi';
  import { piImportPreview, relativeTime } from '../utils/pi';

  type HomeStat = { label: string; value: string; note: string };

  let {
    activeSession,
    homeStats,
    visiblePiImports,
    piImportBusy,
    piImportsLoaded,
    sessionCount,
    onResumeSession,
    onOpenFolder,
    onNewSession,
    onImportPiSession
  }: {
    activeSession: PiSession | undefined;
    homeStats: HomeStat[];
    visiblePiImports: PiSessionMeta[];
    piImportBusy: string;
    piImportsLoaded: boolean;
    sessionCount: number;
    onResumeSession: () => void;
    onOpenFolder: () => void;
    onNewSession: () => void;
    onImportPiSession: (meta: PiSessionMeta) => void;
  } = $props();
</script>

<div class="home-body">
  <section class="launch-card">
    <p class="eyebrow">Local command center</p>
    <h1>Pick up the thread.</h1>
    <p>Mount a project folder, resume a Pi context, or start a clean session.</p>
    <div class="home-actions" aria-label="Home quick actions">
      <button class="primary-action" onclick={onResumeSession} disabled={!activeSession}>Resume session</button>
      <button onclick={onOpenFolder}>Open folder</button>
      <button onclick={onNewSession}>New session</button>
    </div>
  </section>
  <section class="home-metrics" aria-label="Workspace overview">
    {#each homeStats as stat}
      <article>
        <span>{stat.label}</span>
        <strong>{stat.value}</strong>
        <small>{stat.note}</small>
      </article>
    {/each}
  </section>
  <section class="pi-imports" aria-label="Resume from pi">
    <div class="panel-head">
      <span class="eyebrow">Resume from pi</span>
      <small>{visiblePiImports.length} found</small>
    </div>
    {#if visiblePiImports.length}
      <div class="pi-import-list">
        {#each visiblePiImports as meta (meta.session_file)}
          <button
            class="pi-import-row"
            class:busy={piImportBusy === meta.session_file}
            disabled={!!piImportBusy}
            onclick={() => onImportPiSession(meta)}
          >
            <div class="pi-import-row__head">
              <strong>{meta.project_path.split('/').filter(Boolean).at(-1) ?? meta.project_path}</strong>
              <small>{relativeTime(meta.last_activity_ms)}</small>
            </div>
            <small class="pi-import-row__path">{meta.project_path}</small>
            <p class="pi-import-row__preview">{piImportPreview(meta)}</p>
            <div class="pi-import-row__meta">
              <span>{meta.message_count} msg{meta.message_count === 1 ? '' : 's'}</span>
              {#if meta.model_id}<span>{meta.model_id}</span>{/if}
              {#if meta.provider}<span>{meta.provider}</span>{/if}
            </div>
          </button>
        {/each}
      </div>
    {:else if piImportsLoaded && sessionCount === 0}
      <p class="empty-note">No pi conversations found in <code>~/.pi/agent/sessions/</code>.</p>
    {:else if !piImportsLoaded}
      <p class="empty-note">Scanning pi sessions…</p>
    {/if}
  </section>
</div>
