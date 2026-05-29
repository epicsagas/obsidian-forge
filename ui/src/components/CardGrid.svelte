<script lang="ts">
  import type { NoteCard } from '../lib/types';
  import { openInObsidian } from '../lib/api';

  const { notes, s } = $props<{ notes: NoteCard[]; s: any }>();

  function zoneLabel(zone: any): string {
    if ('inbox' in zone) return 'Inbox';
    if ('projects' in zone) return zone.projects;
    if ('areas' in zone) return zone.areas;
    if ('resources' in zone) return 'Resources';
    if ('zettelkasten' in zone) return 'ZK';
    return 'Archives';
  }

  function formatDate(iso: string): string {
    const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 86400000);
    if (diff === 0) return 'today';
    if (diff === 1) return 'yesterday';
    if (diff < 7) return `${diff}d ago`;
    if (diff < 30) return `${Math.floor(diff / 7)}w ago`;
    return new Date(iso).toLocaleDateString('ko-KR', { month: 'short', day: 'numeric' });
  }
</script>

<div class="card-grid">
  {#each notes as note}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="note-card" class:expanded={s.expandedCard === note.path}
      onclick={() => s.expandedCard = s.expandedCard === note.path ? null : note.path}>

      <div class="note-card-header">
        <div class="vitality">
          {#each Array(5) as _, i}
            <div class="vitality-dot" class:filled={i < note.vitality}></div>
          {/each}
        </div>
        <span class="zone-badge">{zoneLabel(note.zone)}</span>
        <span class="note-card-title">{note.title}</span>
      </div>

      {#if note.summary}
        <div class="note-card-summary">{note.summary}</div>
      {/if}

      <div class="note-card-tags">
        {#each note.tags.slice(0, 3) as tag}
          <span class="tag-chip">{tag}</span>
        {/each}
      </div>

      {#if s.expandedCard === note.path}
        <div class="note-card-meta">
          <span>{note.word_count} words</span>
          <span>{note.incoming_links}↙ {note.outgoing_links}↗</span>
          <span>{formatDate(note.modified_at)}</span>
          {#if note.layer}<span>{note.layer}</span>{/if}
          {#if note.has_mermaid}<span>mermaid</span>{/if}
        </div>

        <div class="note-card-tags" style="margin-top:8px;">
          {#each note.tags as tag}
            <span class="tag-chip">{tag}</span>
          {/each}
        </div>

        <div class="note-card-actions">
          <button class="action-btn primary"
            onclick={(e) => { e.stopPropagation(); openInObsidian(note.path); }}>
            OPEN
          </button>
          <button class="action-btn">FIND RELATED</button>
          <button class="action-btn">ASK AI</button>
        </div>
      {/if}
    </div>
  {/each}
</div>
