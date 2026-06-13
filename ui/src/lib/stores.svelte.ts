import type { DashboardState, NoteCard, TimeFilter, VaultInfo } from './types';

// Single reactive state object — accessed DIRECTLY from .svelte components
// Svelte 5 tracks property reads on $state objects in .svelte files
const state = $state({
  vaults: [] as VaultInfo[],
  selectedVault: '',
  dashboard: null as DashboardState | null,
  timeFilter: 'all' as TimeFilter,
  selectedTags: new Set<string>(),
  searchQuery: '',
  expandedCard: null as string | null,
  selectedZone: null as string | null,
});

// Export the state object itself — components read properties directly
// This ensures Svelte 5's reactivity system tracks the property access
export function getState() { return state; }
