<script lang="ts">
  import './app.css';
  import { onMount } from 'svelte';
  import { getVaults, getDashboard, findRelated, askAi } from './lib/api';
  import type { DashboardState, NoteCard, VaultInfo } from './lib/types';

  let loading = $state(true);
  let errorMsg = $state<string | null>(null);
  let vaultList = $state<VaultInfo[]>([]);
  let selectedVault = $state('');
  let dashboard = $state<DashboardState | null>(null);
  let searchQuery = $state('');
  let selectedZone = $state<string | null>(null);
  let activeTags = $state(new Set<string>());
  let expandedCard = $state<string | null>(null);
  let relatedByPath = $state<Record<string, NoteCard[]>>({});
  let aiByPath = $state<Record<string, string>>({});
  let busyKey = $state<string | null>(null);

  const notes = $derived.by(() => {
    if (!dashboard) return [];
    let list = dashboard.notes;
    if (activeTags.size > 0)
      list = list.filter(n => n.tags.some(t => activeTags.has(t)));
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      list = list.filter(n =>
        n.title.toLowerCase().includes(q) ||
        n.path.toLowerCase().includes(q) ||
        (n.summary ?? '').toLowerCase().includes(q) ||
        n.tags.some(t => t.toLowerCase().includes(q))
      );
    }
    if (selectedZone)
      list = list.filter(n => zoneKey(n.zone as any) === selectedZone);
    return list;
  });

  const zoneCounts = $derived.by(() => {
    const c: Record<string, number> = { inbox:0, projects:0, areas:0, resources:0, zettelkasten:0, archives:0 };
    for (const n of dashboard?.notes ?? [])
      c[zoneKey(n.zone as any)] = (c[zoneKey(n.zone as any)] ?? 0) + 1;
    return c;
  });

  function zoneKey(zone: any): string {
    if (zone === 'inbox') return 'inbox';
    if (zone === 'resources') return 'resources';
    if (zone === 'zettelkasten') return 'zettelkasten';
    if (zone === 'archives') return 'archives';
    if (typeof zone === 'object' && zone !== null) {
      if ('projects' in zone) return 'projects';
      if ('areas' in zone) return 'areas';
    }
    return 'archives';
  }

  function zoneLabel(zone: any): string {
    if (zone === 'inbox') return 'Inbox';
    if (zone === 'resources') return 'Resources';
    if (zone === 'zettelkasten') return 'ZK';
    if (typeof zone === 'object' && zone !== null) {
      if ('projects' in zone) return zone.projects;
      if ('areas' in zone) return zone.areas;
    }
    return 'Archives';
  }

  function formatDate(iso: string): string {
    const diff = Math.floor((Date.now() - new Date(iso).getTime()) / 86400000);
    if (diff === 0) return 'today';
    if (diff === 1) return 'yesterday';
    if (diff < 7) return `${diff}d ago`;
    if (diff < 30) return `${Math.floor(diff/7)}w ago`;
    return new Date(iso).toLocaleDateString('ko-KR', { month:'short', day:'numeric' });
  }

  async function loadRelated(path: string) {
    busyKey = path + '|related';
    try { relatedByPath[path] = await findRelated(path); }
    catch { relatedByPath[path] = []; }
    finally { busyKey = null; }
  }

  async function loadAi(path: string) {
    busyKey = path + '|ai';
    try { aiByPath[path] = await askAi(path); }
    catch (e: any) { aiByPath[path] = '오류: ' + String(e?.message ?? e); }
    finally { busyKey = null; }
  }

  async function load(name?: string) {
    loading = true;
    errorMsg = null;
    try {
      if (!name) {
        const v = await getVaults();
        vaultList = v;
        name = (v.find(x => x.enabled) ?? v[0])?.name;
        if (name) selectedVault = name;
      }
      if (name) {
        const d = await getDashboard(name);
        dashboard = d;
      }
    } catch (e: any) {
      errorMsg = String(e?.message ?? e);
    } finally {
      loading = false;
    }
  }

  onMount(() => { load(); });
</script>

<div id="app-root">
  <!-- SIDEBAR -->
  <aside class="sidebar">
    <div class="sidebar-section">
      <h3>Zones</h3>
      {#each [['inbox','Inbox'],['projects','Projects'],['areas','Areas'],['resources','Resources'],['zettelkasten','Zettelkasten'],['archives','Archives']] as [key, label]}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="sidebar-item" class:active={selectedZone === key}
          onclick={() => selectedZone = selectedZone === key ? null : key}
          role="button" tabindex="0">
          <span>{label}</span>
          <span class="count">{zoneCounts[key] ?? 0}</span>
        </div>
      {/each}
    </div>

    <div class="sidebar-divider"></div>

    <div class="sidebar-section">
      <h3>Tags</h3>
      {#each dashboard?.tags.slice(0, 20) ?? [] as t}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="sidebar-item" class:active={activeTags.has(t.tag)}
          onclick={() => {
            const n = new Set(activeTags);
            n.has(t.tag) ? n.delete(t.tag) : n.add(t.tag);
            activeTags = n;
          }}
          role="button" tabindex="0">
          <span>{t.tag}</span>
          <span class="count">{t.count}</span>
        </div>
      {/each}
    </div>

    <div class="sidebar-divider"></div>

    <div class="sidebar-section">
      <h3>Filters</h3>
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="sidebar-item"
        onclick={() => { activeTags = new Set(); selectedZone = null; searchQuery = ''; }}
        role="button" tabindex="0">
        <span>Clear all</span>
      </div>
    </div>
  </aside>

  <!-- MAIN -->
  <div class="main">
    <div class="header">
      <div style="display:flex;align-items:center;gap:12px">
        <h1>Vault Dashboard</h1>
        <select class="vault-select" value={selectedVault}
          onchange={(e) => { const v = (e.target as HTMLSelectElement).value; selectedVault = v; load(v); }}>
          {#each vaultList as v}
            <option value={v.name}>{v.name}</option>
          {/each}
        </select>
        {#if dashboard}
          <span class="stats">{dashboard.total_notes} notes · {dashboard.orphan_count} orphans</span>
        {/if}
      </div>
      <div class="header-right">
        <input class="search-input" placeholder="Search..."
          value={searchQuery}
          oninput={(e) => searchQuery = (e.target as HTMLInputElement).value} />
        <button class="refresh-btn" onclick={() => load(selectedVault)}>⟳</button>
      </div>
    </div>

    <!-- Attention -->
    {#if dashboard?.needs_attention}
      {@const a = dashboard.needs_attention}
      {#if a.orphans.length || a.stale.length || a.untagged.length || a.inbox_count}
        <div class="attention-panel">
          {#if a.orphans.length}<div class="attention-item"><span class="attention-dot red"></span>{a.orphans.length} orphans</div>{/if}
          {#if a.stale.length}<div class="attention-item"><span class="attention-dot yellow"></span>{a.stale.length} stale</div>{/if}
          {#if a.untagged.length}<div class="attention-item"><span class="attention-dot gray"></span>{a.untagged.length} untagged</div>{/if}
          {#if a.inbox_count}<div class="attention-item"><span class="attention-dot yellow"></span>{a.inbox_count} inbox</div>{/if}
        </div>
      {/if}
    {/if}

    <!-- Content -->
    {#if loading}
      <div class="loading">Loading vault...</div>
    {:else if errorMsg}
      <div class="error" style="padding:40px;font-family:monospace;font-size:13px;word-break:break-all">{errorMsg}</div>
    {:else if notes.length === 0}
      <div class="empty">No notes found</div>
    {:else}
      <div class="card-grid">
        {#each notes as note}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="note-card" class:expanded={expandedCard === note.path}
            onclick={() => expandedCard = expandedCard === note.path ? null : note.path}>
            <div class="note-card-header">
              <div class="vitality">
                {#each Array(5) as _,i}
                  <div class="vitality-dot" class:filled={i < note.vitality}></div>
                {/each}
              </div>
              <span class="zone-badge">{zoneLabel(note.zone as any)}</span>
              <span class="note-card-title">{note.title}</span>
            </div>
            {#if note.summary}
              <div class="note-card-summary">{note.summary}</div>
            {/if}
            <div class="note-card-tags">
              {#each note.tags.slice(0,3) as tag}
                <span class="tag-chip">{tag}</span>
              {/each}
            </div>
            {#if expandedCard === note.path}
              <div class="note-card-meta">
                <span>{note.word_count} words</span>
                <span>{note.incoming_links}↙ {note.outgoing_links}↗</span>
                <span>{formatDate(note.modified_at)}</span>
                {#if note.layer}<span>{note.layer}</span>{/if}
              </div>
              <div class="note-card-actions">
                <button class="action-btn primary"
                  onclick={(e) => { e.stopPropagation(); import('./lib/api').then(a => a.openInObsidian(note.path)); }}>
                  OPEN
                </button>
                <button class="action-btn"
                  onclick={(e) => { e.stopPropagation(); loadRelated(note.path); }}>FIND RELATED</button>
                <button class="action-btn"
                  onclick={(e) => { e.stopPropagation(); loadAi(note.path); }}>ASK AI</button>
              </div>

              {#if busyKey === note.path + '|related'}
                <div class="card-loading">관련 노드 검색 중...</div>
              {:else if relatedByPath[note.path]}
                {@const related = relatedByPath[note.path]}
                <div class="related-section">
                  <div class="related-title">관련 노드 ({related.length})</div>
                  {#each related as r}
                    <!-- svelte-ignore a11y_click_events_have_key_events -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div class="related-item"
                      onclick={(e) => { e.stopPropagation(); searchQuery = r.title; }}
                      role="button" tabindex="0">
                      <span class="zone-badge">{zoneLabel(r.zone as any)}</span>
                      <span>{r.title}</span>
                    </div>
                  {/each}
                </div>
              {/if}

              {#if busyKey === note.path + '|ai'}
                <div class="card-loading">AI 분석 중...</div>
              {:else if aiByPath[note.path]}
                <div class="ai-section">{aiByPath[note.path]}</div>
              {/if}
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  #app-root { display:flex; height:100vh; width:100vw; }
</style>
