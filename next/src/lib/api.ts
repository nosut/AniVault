import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export interface EngineStatus {
  ok: boolean;
  database: 'ready';
  database_path: string;
  migration_count: number;
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

export interface MediaDetectedEvent {
  MediaDetected: {
    player_name: string;
    file_path: string | null;
    window_title: string | null;
    detected_at_unix: number;
  };
}

export interface MatchCandidate {
  anime_id: number;
  title: string;
  confidence: number;
  match_source: string;
}

export interface ParsedFilename {
  cleaned_title: string;
  episode_number: number;
  release_group: string | null;
  quality: string | null;
  raw: string;
}

export interface RecognitionResult {
  known_file: boolean;
  parsed: ParsedFilename | null;
  candidates: MatchCandidate[];
}

export interface FileIndexEntry {
  file_path: string;
  anime_id: number | null;
  episode: number | null;
  confidence: number;
  indexed_at: number;
}

export interface AniListSyncStatus {
  pending: number;
  failed: number;
  blocked: number;
  last_sync_at: number | null;
}

export interface ImportReport {
  imported: number;
  merged: number;
  skipped: number;
}

export interface LibraryEntry {
  anime_id: number;
  title: string;
  status: string;
  watched_episodes: number;
  episode_count: number | null;
  score: number | null;
  image_url: string | null;
}

export interface LibraryStats {
  total: number;
  watching: number;
  completed: number;
  on_hold: number;
  dropped: number;
  plan_to_watch: number;
}

export interface AnimeDetail {
  anime_id: number;
  titles_json: string;
  episode_count: number | null;
  image_url: string | null;
  synopsis: string | null;
  anime_status: string | null;
  last_modified: number;
  list_status: string | null;
  watched_episodes: number | null;
  score: number | null;
  notes: string | null;
  local_updated: number | null;
  remote_updated: number | null;
  tracker_id: string | null;
}

export interface PlaybackDetectedEvent {
  PlaybackDetected: {
    player_name: string;
    file_path: string | null;
    window_title: string | null;
    episode_guess: number | null;
    candidates: MatchCandidate[];
    detected_at_unix: number;
  };
}

export interface AnimeIdentifiedEvent {
  AnimeIdentified: {
    anime_id: number;
    episode: number;
    confidence: number;
    evidence: string;
  };
}

export interface ProgressAdvancedEvent {
  ProgressAdvanced: {
    anime_id: number;
    old_episode: number;
    new_episode: number;
    source: string;
  };
}

export interface SyncQueuedEvent {
  SyncQueued: {
    service: string;
    anime_id: number;
  };
}

export interface SyncFailedEvent {
  SyncFailed: {
    service: string;
    anime_id: number;
    message: string;
  };
}

export type EngineEvent =
  | MediaDetectedEvent
  | PlaybackDetectedEvent
  | AnimeIdentifiedEvent
  | ProgressAdvancedEvent
  | SyncQueuedEvent
  | SyncFailedEvent;

export function getEngineStatus(invokeFn: InvokeFn = tauriInvoke): Promise<EngineStatus> {
  return invokeFn<EngineStatus>('get_engine_status');
}

export function previewMigrationReport(invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('preview_migration_report');
}

export function getSetting<T>(key: string, invokeFn: InvokeFn = tauriInvoke): Promise<T | null> {
  return invokeFn<T | null>('get_setting', { key });
}

export function setSetting(key: string, value: unknown, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_setting', { key, value });
}

export function deleteSetting(key: string, invokeFn: InvokeFn = tauriInvoke): Promise<boolean> {
  return invokeFn<boolean>('delete_setting', { key });
}

export function drainEngineEvents(invokeFn: InvokeFn = tauriInvoke): Promise<EngineEvent[]> {
  return invokeFn<EngineEvent[]>('drain_engine_events');
}

export interface ActivePlaybackInfo {
  player_name: string;
  file_path: string | null;
  window_title: string | null;
  episode_guess: number | null;
}

export interface TrackingStatus {
  active: boolean;
  watching: ActivePlaybackInfo | null;
}

export interface RecentHistoryEntry {
  id: number;
  anime_id: number;
  episode: number;
  file_path: string | null;
  player: string | null;
  watched_at: number;
}

export function getTrackingStatus(invokeFn: InvokeFn = tauriInvoke): Promise<TrackingStatus> {
  return invokeFn<TrackingStatus>('get_tracking_status');
}

export function startTracking(invokeFn: InvokeFn = tauriInvoke): Promise<TrackingStatus> {
  return invokeFn<TrackingStatus>('start_tracking');
}

export function stopTracking(invokeFn: InvokeFn = tauriInvoke): Promise<TrackingStatus> {
  return invokeFn<TrackingStatus>('stop_tracking');
}

export function markEpisodeWatched(anime_id: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('mark_episode_watched', { anime_id, episode });
}

export function listRecentHistory(limit: number, invokeFn: InvokeFn = tauriInvoke): Promise<RecentHistoryEntry[]> {
  return invokeFn<RecentHistoryEntry[]>('list_recent_history', { limit });
}

export function identifyFile(filePath: string, windowTitle: string | null, invokeFn: InvokeFn = tauriInvoke): Promise<RecognitionResult> {
  return invokeFn<RecognitionResult>('identify_file', { file_path: filePath, window_title: windowTitle });
}

export function confirmIdentification(filePath: string, animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('confirm_identification', { file_path: filePath, anime_id: animeId, episode });
}

export function listKnownFiles(limit: number, invokeFn: InvokeFn = tauriInvoke): Promise<FileIndexEntry[]> {
  return invokeFn<FileIndexEntry[]>('list_known_files', { limit });
}

export function storeAniListToken(token: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('store_anilist_token', { token });
}

export function disconnectAniList(invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('disconnect_anilist');
}

export function importAniListLibrary(invokeFn: InvokeFn = tauriInvoke): Promise<ImportReport> {
  return invokeFn<ImportReport>('import_anilist_library');
}

export function getSyncStatus(invokeFn: InvokeFn = tauriInvoke): Promise<AniListSyncStatus> {
  return invokeFn<AniListSyncStatus>('get_sync_status');
}

export function searchLibrary(query: string, statusFilter: string | null, limit: number, offset: number, invokeFn: InvokeFn = tauriInvoke): Promise<LibraryEntry[]> {
  return invokeFn<LibraryEntry[]>('search_library', { query, status_filter: statusFilter, limit, offset });
}

export function getLibraryStats(invokeFn: InvokeFn = tauriInvoke): Promise<LibraryStats> {
  return invokeFn<LibraryStats>('get_library_stats');
}

export function fetchAnimeDetail(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<AnimeDetail | null> {
  return invokeFn<AnimeDetail | null>('fetch_anime_detail', { anime_id: animeId });
}

export function updateListEntry(animeId: number, status: string | null, watchedEpisodes: number | null, score: number | null, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('update_list_entry', { anime_id: animeId, status, watched_episodes: watchedEpisodes, score });
}
