import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export interface EngineStatus {
  ok: boolean;
  database: 'ready' | 'uninitialized';
}

export interface MigrationWarning {
  source: string;
  source_id: string;
  message: string;
}

export interface MigrationReport {
  imported_anime: number;
  skipped_records: number;
  warnings: MigrationWarning[];
}

export interface TrackingStatus {
  is_running: boolean;
  current_anime: string | null;
  current_anime_id: number | null;
  current_episode: number | null;
}

export interface OAuthStatus {
  authenticated: boolean;
  username: string | null;
}

export function getEngineStatus(invokeFn: InvokeFn = tauriInvoke): Promise<EngineStatus> {
  return invokeFn<EngineStatus>('get_engine_status');
}

export function previewMigrationReport(invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('preview_migration_report');
}

export function getTrackingStatus(invokeFn: InvokeFn = tauriInvoke): Promise<TrackingStatus> {
  return invokeFn<TrackingStatus>('get_tracking_status');
}

export function startOAuth(invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('start_oauth');
}

export function completeOAuth(invokeFn: InvokeFn = tauriInvoke): Promise<OAuthStatus> {
  return invokeFn<OAuthStatus>('complete_oauth');
}

export function getOAuthStatus(invokeFn: InvokeFn = tauriInvoke): Promise<OAuthStatus> {
  return invokeFn<OAuthStatus>('get_oauth_status');
}

export interface SyncStatus {
  pending: number;
  failed: number;
}

export function getSyncStatus(invokeFn: InvokeFn = tauriInvoke): Promise<SyncStatus> {
  return invokeFn<SyncStatus>('get_sync_status');
}

export function setWatchedEpisodes(animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_watched_episodes', { animeId, episode });
}

export interface LibraryEntry {
  id: number;
  title: string;
  status: string;
  watched_episodes: number;
  episode_count: number | null;
}

export function getLibraryAnime(invokeFn: InvokeFn = tauriInvoke): Promise<LibraryEntry[]> {
  return invokeFn<LibraryEntry[]>('get_library_anime');
}

export interface SonarrConfig {
  url: string;
  api_key: string;
}

export function getSonarrConfig(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrConfig> {
  return invokeFn<SonarrConfig>('get_sonarr_config');
}

export function setSonarrConfig(url: string, apiKey: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_sonarr_config', { url, apiKey });
}

export function testSonarrConnection(url: string, apiKey: string, invokeFn: InvokeFn = tauriInvoke): Promise<string> {
  return invokeFn<string>('test_sonarr_connection', { url, apiKey });
}

export interface SonarrMapping {
  anime_id: number;
  sonarr_series_id: number;
  sonarr_title: string;
  monitored: boolean;
}

export function getSonarrMappings(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrMapping[]> {
  return invokeFn<SonarrMapping[]>('get_sonarr_mappings');
}

export function mapSonarrSeries(animeId: number, sonarrSeriesId: number, sonarrTitle: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('map_sonarr_series', { animeId, sonarrSeriesId, sonarrTitle });
}
