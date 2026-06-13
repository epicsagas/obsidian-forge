<script lang="ts">
  const { s, zoneCounts } = $props();
</script>

<aside class="sidebar">
  <div class="sidebar-section">
    <h3>Zones</h3>
    {#each [['inbox','Inbox'],['projects','Projects'],['areas','Areas'],['resources','Resources'],['zettelkasten','Zettelkasten']] as [key, label]}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="sidebar-item" class:active={s.selectedZone === key}
        onclick={() => s.selectedZone = s.selectedZone === key ? null : key}
        role="button" tabindex="0">
        <span>{label}</span>
        <span class="count">{zoneCounts[key]}</span>
      </div>
    {/each}
  </div>

  <div class="sidebar-divider"></div>

  <div class="sidebar-section">
    <h3>Tags</h3>
    {#each s.dashboard?.tags.slice(0, 20) ?? [] as t}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="sidebar-item" class:active={s.selectedTags.has(t.tag)}
        onclick={() => { if (s.selectedTags.has(t.tag)) s.selectedTags.delete(t.tag); else s.selectedTags.add(t.tag); }}
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
      onclick={() => { s.selectedTags = new Set(); s.selectedZone = null; s.searchQuery = ''; }}
      role="button" tabindex="0">
      <span>Clear all</span>
    </div>
  </div>
</aside>
