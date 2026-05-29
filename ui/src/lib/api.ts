import { invoke } from '@tauri-apps/api/core';
import type { DashboardState, VaultInfo } from './types';

export async function getVaults(): Promise<VaultInfo[]> {
  return invoke('get_vaults');
}

export async function getDashboard(vaultName: string): Promise<DashboardState> {
  return invoke('get_dashboard', { vaultName });
}

export async function openInObsidian(path: string): Promise<void> {
  return invoke('open_in_obsidian', { path });
}
