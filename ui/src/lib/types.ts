export interface NoteCard {
  path: string;
  title: string;
  summary: string | null;
  tags: string[];
  vitality: number;
  zone: Zone;
  layer: string | null;
  word_count: number;
  modified_at: string;
  incoming_links: number;
  outgoing_links: number;
  has_mermaid: boolean;
  is_orphan: boolean;
}

export type Zone =
  | { inbox: true }
  | { projects: string }
  | { areas: string }
  | { resources: true }
  | { zettelkasten: true }
  | { archives: true };

export interface TagSummary {
  tag: string;
  count: number;
}

export interface AttentionPanel {
  orphans: NoteCard[];
  stale: NoteCard[];
  untagged: NoteCard[];
  inbox_count: number;
}

export interface DashboardState {
  notes: NoteCard[];
  tags: TagSummary[];
  needs_attention: AttentionPanel;
  vault_name: string;
  total_notes: number;
  orphan_count: number;
}

export interface VaultInfo {
  name: string;
  path: string;
  enabled: boolean;
}

export type TimeFilter = 'today' | 'this_week' | 'this_month' | 'all';
