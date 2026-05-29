<script lang="ts">
  import type { DashboardState } from '../lib/types';
  const { dashboard } = $props<{ dashboard: DashboardState | null }>();
</script>

{#if dashboard?.needs_attention}
  {@const a = dashboard.needs_attention}
  {#if a.orphans.length > 0 || a.stale.length > 0 || a.untagged.length > 0 || a.inbox_count > 0}
    <div class="attention-panel">
      {#if a.orphans.length > 0}
        <div class="attention-item"><span class="attention-dot red"></span>{a.orphans.length} orphans</div>
      {/if}
      {#if a.stale.length > 0}
        <div class="attention-item"><span class="attention-dot yellow"></span>{a.stale.length} stale</div>
      {/if}
      {#if a.untagged.length > 0}
        <div class="attention-item"><span class="attention-dot gray"></span>{a.untagged.length} untagged</div>
      {/if}
      {#if a.inbox_count > 0}
        <div class="attention-item"><span class="attention-dot yellow"></span>{a.inbox_count} in inbox</div>
      {/if}
    </div>
  {/if}
{/if}
