<script lang="ts">
  const { current, onchange } = $props<{ current: string; onchange: (f: string) => void }>();
  const filters = [['today','Today'],['this_week','This Week'],['this_month','This Month'],['all','All']];
</script>

<div class="time-filter">
  {#each filters as [key, label]}
    <button class="time-btn" class:active={current === key} onclick={() => onchange(key)}>
      {label}
    </button>
  {/each}
</div>
