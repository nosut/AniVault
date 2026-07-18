import { invoke as tauriInvoke } from '@tauri-apps/api/core';

export type InvokeFn = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

export interface EngineStatus {
  ok: boolean;
  database: 'ready';
  database_path: string;
  migration_count: number;
}

export interface V1DataPaths {
  sqlite_path: string | null;
  history_xml_path: string | null;
  anime_xml_path: string | null;
  list_xml_path: string | null;
  data_dir: string | null;
  found: boolean;
}

export interface MigrationWarning {
  source: string;
  source_id: string;
  message: string;
}

export interface MigrationReport {
  imported_anime: number;
  imported_entries: number;
  imported_history: number;
  skipped_anime: number;
  skipped_entries: number;
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

export type MappingSource = 'automatic' | 'inherited' | 'manual' | 'legacy';

export interface FileIndexEntry {
  file_path: string;
  anime_id: number | null;
  episode: number | null;
  confidence: number;
  indexed_at: number;
  ignored: boolean;
  mapping_source: MappingSource;
}

export interface KnownFileEntry extends FileIndexEntry {
  anime_title: string | null;
}

export interface FileMappingConflict {
  file_path: string;
  episode: number | null;
  current_anime_id: number;
  current_anime_title: string;
  mapping_source: MappingSource;
  target_confidence: number;
  repairable: boolean;
}

export interface LibraryScanReport {
  found: number;
  indexed: number;
  skipped: number;
  removed: number;
  errors: string[];
  mapping_conflicts: FileMappingConflict[];
}

export interface FileMappingRepairReport {
  repaired: number;
  skipped: number;
  protected: number;
}

export interface AniListSyncStatus {
  pending: number;
  failed: number;
  blocked: number;
  last_sync_at: number | null;
}

export interface SyncResult {
  processed: number;
  failed: number;
}

export interface ImportReport {
  imported: number;
  merged: number;
  skipped: number;
}

export interface SessionState {
  paused: boolean;
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

export interface LibraryUpdatedEvent {
  LibraryUpdated: {
    indexed: number;
    removed: number;
  };
}

export type EngineEvent =
  | MediaDetectedEvent
  | PlaybackDetectedEvent
  | AnimeIdentifiedEvent
  | ProgressAdvancedEvent
  | SyncQueuedEvent
  | SyncFailedEvent
  | LibraryUpdatedEvent;

export interface LibraryEntry {
  anime_id: number;
  title: string;
  status: string;
  watched_episodes: number;
  episode_count: number | null;
  score: number | null;
  image_url: string | null;
  season: string | null;
  season_year: number | null;
  airing_status: string | null;
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
  recent_history: RecentHistoryEntry[];
}

export interface SonarrStatus {
  connected: boolean;
  series_count: number;
  mapped_count: number;
  last_sync_at: number | null;
}

export interface SonarrImportReport {
  imported: number;
  auto_mapped: number;
  unmapped: number;
}

export interface SonarrAvailability {
  sonarr_id: number;
  sonarr_title: string;
  monitored: boolean;
  episode_count: number;
  episode_file_count: number;
  next_airing: number | null;
  path: string | null;
  season_count: number;
  sonarr_status: string | null;
}

export function getEngineStatus(invokeFn: InvokeFn = tauriInvoke): Promise<EngineStatus> {
  return invokeFn<EngineStatus>('get_engine_status');
}

export function previewMigration(invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('preview_migration');
}

export function discoverV1Data(invokeFn: InvokeFn = tauriInvoke): Promise<V1DataPaths> {
  return invokeFn<V1DataPaths>('discover_v1_data');
}

export function runMigration(strategy: 'Skip' | 'Merge', invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('run_migration', { strategy });
}

export function backupDatabase(invokeFn: InvokeFn = tauriInvoke): Promise<string> {
  return invokeFn<string>('backup_database');
}

// The backend restarts the app immediately after a successful restore
// (see commands.rs's restore_database), so this promise's success branch
// is never actually observed by the caller in practice — only rejection
// (a validation or file-system error before the restart) is. Don't add
// .then() logic here expecting to run after a successful restore.
export function restoreDatabase(backupPath: string, invokeFn: InvokeFn = tauriInvoke): Promise<string> {
  return invokeFn<string>('restore_database', { backupPath });
}

export function exportDatabase(invokeFn: InvokeFn = tauriInvoke): Promise<string> {
  return invokeFn<string>('export_database');
}

export function importDatabase(json: string, invokeFn: InvokeFn = tauriInvoke): Promise<MigrationReport> {
  return invokeFn<MigrationReport>('import_database', { json });
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
  return invokeFn<void>('mark_episode_watched', { animeId: anime_id, episode });
}

export function listRecentHistory(limit: number, invokeFn: InvokeFn = tauriInvoke): Promise<RecentHistoryEntry[]> {
  return invokeFn<RecentHistoryEntry[]>('list_recent_history', { limit });
}

export function identifyFile(filePath: string, windowTitle: string | null, invokeFn: InvokeFn = tauriInvoke): Promise<RecognitionResult> {
  return invokeFn<RecognitionResult>('identify_file', { filePath: filePath, windowTitle: windowTitle });
}

export function confirmIdentification(filePath: string, animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('confirm_identification', { filePath: filePath, animeId: animeId, episode });
}

export function listKnownFiles(limit: number, invokeFn: InvokeFn = tauriInvoke): Promise<KnownFileEntry[]> {
  return invokeFn<KnownFileEntry[]>('list_known_files', { limit });
}

export function rematchUnmappedFiles(invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('rematch_unmapped_files');
}

export function setKnownFileIgnored(filePath: string, ignored: boolean, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_known_file_ignored', { filePath, ignored });
}

export function deleteKnownFile(filePath: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('delete_known_file', { filePath });
}

export function setKnownFileMapping(filePath: string, animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_known_file_mapping', { filePath, animeId, episode });
}

export interface FileMappingInput {
  file_path: string;
  anime_id: number;
  episode: number;
}

export function setKnownFileMappings(mappings: FileMappingInput[], invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('set_known_file_mappings', { mappings });
}

export function setKnownFilesIgnored(filePaths: string[], ignored: boolean, invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('set_known_files_ignored', { filePaths, ignored });
}

export function deleteKnownFiles(filePaths: string[], invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('delete_known_files', { filePaths });
}

export function unmapKnownFiles(filePaths: string[], invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('unmap_known_files', { filePaths });
}

export function importAnilistAnime(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('import_anilist_anime', { animeId });
}

export interface DeepMatchReport {
  groups_total: number;
  groups_matched: number;
  files_mapped: number;
  unmatched: string[];
}

export function deepMatchViaAnilist(invokeFn: InvokeFn = tauriInvoke): Promise<DeepMatchReport> {
  return invokeFn<DeepMatchReport>('deep_match_via_anilist');
}

export function connectAniListOauth(clientId: string, clientSecret: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('connect_anilist_oauth', { clientId, clientSecret });
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

export function getAniListConnectionStatus(invokeFn: InvokeFn = tauriInvoke): Promise<boolean> {
  return invokeFn<boolean>('get_anilist_connection_status');
}

export function getSessionState(invokeFn: InvokeFn = tauriInvoke): Promise<SessionState> {
  return invokeFn<SessionState>('get_session_state');
}

export function togglePauseTracking(invokeFn: InvokeFn = tauriInvoke): Promise<SessionState> {
  return invokeFn<SessionState>('toggle_pause_tracking');
}

export function getLaunchOnStartup(invokeFn: InvokeFn = tauriInvoke): Promise<boolean> {
  return invokeFn<boolean>('get_launch_on_startup');
}

export function setLaunchOnStartup(enabled: boolean, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_launch_on_startup', { enabled });
}

export function getStartInTray(invokeFn: InvokeFn = tauriInvoke): Promise<boolean> {
  return invokeFn<boolean>('get_start_in_tray');
}

export function setStartInTray(enabled: boolean, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_start_in_tray', { enabled });
}

export function connectSonarr(
  url: string,
  apiKey: string,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<void> {
  return invokeFn<void>('connect_sonarr', { url, apiKey });
}

export function disconnectSonarr(invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('disconnect_sonarr');
}

export function getSonarrStatus(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrStatus> {
  return invokeFn<SonarrStatus>('get_sonarr_status');
}

export function importSonarrSeries(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrImportReport> {
  return invokeFn<SonarrImportReport>('import_sonarr_series');
}

export interface SonarrSeriesListRow {
  sonarr_id: number;
  title: string;
  poster_url: string | null;
  episode_count: number;
  anime_id: number | null;
  confidence: number | null;
  anime_title: string | null;
}

export function listSonarrSeries(invokeFn: InvokeFn = tauriInvoke): Promise<SonarrSeriesListRow[]> {
  return invokeFn<SonarrSeriesListRow[]>('list_sonarr_series');
}

export function testSonarrConnection(url: string, apiKey: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('test_sonarr_connection', { url, apiKey });
}

export function getSonarrAvailability(
  animeId: number,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<SonarrAvailability | null> {
  return invokeFn<SonarrAvailability | null>('get_sonarr_availability', { animeId: animeId });
}

export function remapSonarr(
  sonarrId: number,
  animeId: number | null,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<void> {
  return invokeFn<void>('remap_sonarr', { sonarrId: sonarrId, animeId: animeId });
}

export interface CalendarEntry {
  anime_id: number;
  title: string;
  image_url: string | null;
  episode_count: number | null;
  progress: number | null;
  next_episode: number | null;
  airing_at: number | null;
  time_until_airing: number | null;
  has_file: boolean;
}

export interface WatchHistoryEntry {
  id: number;
  anime_id: number;
  anime_title: string;
  episode: number;
  file_path: string | null;
  player: string | null;
  watched_at: number;
  source: string;
}

export function queueAniListSync(animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('queue_anilist_sync', { animeId, episode });
}

export function getWatchHistory(query?: string, limit?: number, offset?: number, invokeFn: InvokeFn = tauriInvoke): Promise<WatchHistoryEntry[]> {
  return invokeFn<WatchHistoryEntry[]>('get_watch_history', { query: query ?? null, limit: limit ?? 100, offset: offset ?? 0 });
}

export function getCalendar(invokeFn: InvokeFn = tauriInvoke): Promise<CalendarEntry[]> {
  return invokeFn<CalendarEntry[]>('get_calendar');
}

export interface ContinueWatchingEntry {
  anime_id: number;
  anime_title: string;
  image_url: string | null;
  watched_episodes: number;
  episode_count: number | null;
  last_watched_at: number;
}

export function searchSonarrEpisode(animeId: number, episode: number, invokeFn: InvokeFn = tauriInvoke): Promise<string> {
  return invokeFn<string>('search_sonarr_episode', { animeId, episode });
}

export interface UpdateInfo {
  current: string;
  latest: string;
  url: string;
  update_available: boolean;
}

export function checkForUpdate(invokeFn: InvokeFn = tauriInvoke): Promise<UpdateInfo> {
  return invokeFn<UpdateInfo>('check_for_update');
}

export interface ReadyToWatchEntry {
  anime_id: number;
  title: string;
  image_url: string | null;
  next_episode: number;
  ready_count: number;
  watched_episodes: number;
  episode_count: number | null;
}

export function getReadyToWatch(invokeFn: InvokeFn = tauriInvoke): Promise<ReadyToWatchEntry[]> {
  return invokeFn<ReadyToWatchEntry[]>('get_ready_to_watch');
}

export function getContinueWatching(invokeFn: InvokeFn = tauriInvoke): Promise<ContinueWatchingEntry[]> {
  return invokeFn<ContinueWatchingEntry[]>('continue_watching');
}

export interface ScoreBucket {
  range: string;
  count: number;
}

export interface AnimeStats {
  score_distribution: ScoreBucket[];
  total_anime: number;
  total_episodes_watched: number;
  total_rewatches: number;
  avg_score: number;
  episodes_today: number;
  episodes_this_week: number;
}

export function getStatistics(invokeFn: InvokeFn = tauriInvoke): Promise<AnimeStats> {
  return invokeFn<AnimeStats>('get_statistics');
}

export interface SeasonAnimeEntry {
  id: number;
  title: string;
  image_url: string | null;
  episodes: number | null;
  status: string | null;
  format: string | null;
  average_score: number | null;
  popularity: number | null;
}

export function searchAnime(query: string, invokeFn: InvokeFn = tauriInvoke): Promise<SeasonAnimeEntry[]> {
  return invokeFn<SeasonAnimeEntry[]>('search_anime', { query });
}

export function getSeasonAnime(season: string, year: number, genre?: string, invokeFn: InvokeFn = tauriInvoke): Promise<SeasonAnimeEntry[]> {
  return invokeFn<SeasonAnimeEntry[]>('get_season_anime', { season, year, genre: genre ?? null });
}

export interface FutureAnimeEntry extends SeasonAnimeEntry {
  season: string | null;
  season_year: number | null;
  start_year: number | null;
}

export function getFutureAnime(genre?: string, invokeFn: InvokeFn = tauriInvoke): Promise<FutureAnimeEntry[]> {
  return invokeFn<FutureAnimeEntry[]>('get_future_anime', { genre: genre ?? null });
}

export function triggerSync(invokeFn: InvokeFn = tauriInvoke): Promise<SyncResult> {
  return invokeFn<SyncResult>('trigger_sync');
}

export function getSyncStatus(invokeFn: InvokeFn = tauriInvoke): Promise<AniListSyncStatus> {
  return invokeFn<AniListSyncStatus>('get_sync_status');
}

export function searchLibrary(
  query: string,
  statusFilter?: string | null,
  limit?: number,
  offset?: number,
  invokeFn: InvokeFn = tauriInvoke,
): Promise<LibraryEntry[]> {
  return invokeFn<LibraryEntry[]>('search_library', {
    query,
    statusFilter: statusFilter ?? null,
    limit: limit ?? 50,
    offset: offset ?? 0,
  });
}

export function getLibraryStats(invokeFn: InvokeFn = tauriInvoke): Promise<LibraryStats> {
  return invokeFn<LibraryStats>('get_library_stats');
}

// Full set of anime ids that have a list entry (any status) — unpaginated,
// for "is this anime in my library?" membership checks (season/search grids).
export function getLibraryIds(invokeFn: InvokeFn = tauriInvoke): Promise<number[]> {
  return invokeFn<number[]>('get_library_ids');
}

export function fetchAnimeDetail(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<AnimeDetail> {
  return invokeFn<AnimeDetail>('fetch_anime_detail', { animeId: animeId });
}

export interface NextAiring {
  episode: number;
  airing_at: number;
  time_until_airing: number;
}

export function getNextAiring(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<NextAiring | null> {
  return invokeFn<NextAiring | null>('get_next_airing', { animeId });
}

export interface RelationEntry {
  id: number;
  title: string;
  relation_type: string;
  format: string | null;
  status: string | null;
  image_url: string | null;
}

export function getAnimeRelations(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<RelationEntry[]> {
  return invokeFn<RelationEntry[]>('get_anime_relations', { animeId });
}

export function deleteAnime(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('delete_anime', { animeId });
}

export function updateListEntry(
  animeId: number,
  updates: { status?: string | null; watched_episodes?: number | null; score?: number | null },
  invokeFn: InvokeFn = tauriInvoke,
): Promise<void> {
  return invokeFn<void>('update_list_entry', {
    animeId: animeId,
    status: updates.status ?? null,
    watchedEpisodes: updates.watched_episodes ?? null,
    score: updates.score ?? null,
  });
}

export function getLibraryFolders(invokeFn: InvokeFn = tauriInvoke): Promise<string[]> {
  return invokeFn<string[]>('get_library_folders');
}

export function setLibraryFolders(folders: string[], invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('set_library_folders', { folders });
}

export function scanLibraryFolders(invokeFn: InvokeFn = tauriInvoke): Promise<LibraryScanReport> {
  return invokeFn<LibraryScanReport>('scan_library_folders');
}

export function rescanAnimeFiles(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<LibraryScanReport> {
  return invokeFn<LibraryScanReport>('rescan_anime_files', { animeId });
}

export function getEpisodeFiles(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<FileIndexEntry[]> {
  return invokeFn<FileIndexEntry[]>('get_episode_files', { animeId });
}

export function repairAnimeFileMappings(animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<FileMappingRepairReport> {
  return invokeFn<FileMappingRepairReport>('repair_anime_file_mappings', { animeId });
}

export function openEpisodeFile(path: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('open_episode_file', { path });
}

export function openContainingFolder(path: string, invokeFn: InvokeFn = tauriInvoke): Promise<void> {
  return invokeFn<void>('open_containing_folder', { path });
}

export function pickFolder(invokeFn: InvokeFn = tauriInvoke): Promise<string | null> {
  return invokeFn<string | null>('pick_folder');
}

export function mapFolderToAnime(folder: string, animeId: number, invokeFn: InvokeFn = tauriInvoke): Promise<number> {
  return invokeFn<number>('map_folder_to_anime', { folder, animeId });
}
