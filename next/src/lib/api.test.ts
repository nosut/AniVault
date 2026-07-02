import { describe, expect, it, vi } from 'vitest';
import {
  confirmIdentification,
  connectSonarr,
  deleteSetting,
  disconnectAniList,
  disconnectSonarr,
  drainEngineEvents,
  fetchAnimeDetail,
  getEngineStatus,
  getLaunchOnStartup,
  getLibraryStats,
  getSessionState,
  getSetting,
  getSonarrAvailability,
  getSonarrStatus,
  getSyncStatus,
  getTrackingStatus,
  identifyFile,
  importAniListLibrary,
  importSonarrSeries,
  listKnownFiles,
  listRecentHistory,
  markEpisodeWatched,
  previewMigrationReport,
  remapSonarr,
  searchLibrary,
  setLaunchOnStartup,
  setSetting,
  startTracking,
  stopTracking,
  storeAniListToken,
  togglePauseTracking,
  updateListEntry,
} from './api';

describe('api wrappers', () => {
  it('gets engine status through invoke', async () => {
    const status = {
      ok: true,
      database: 'ready',
      database_path: 'C:/Users/example/AppData/Roaming/AniVault/anivault.db',
      migration_count: 1,
    };
    const invoke = vi.fn().mockResolvedValue(status);

    await expect(getEngineStatus(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('get_engine_status');
  });

  it('previews migration report through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue({ imported_anime: 0, skipped_records: 0, warnings: [] });
    await expect(previewMigrationReport(invoke)).resolves.toEqual({ imported_anime: 0, skipped_records: 0, warnings: [] });
    expect(invoke).toHaveBeenCalledWith('preview_migration_report');
  });

  it('gets setting through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    await expect(getSetting<boolean>('tracking.enabled', invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('get_setting', { key: 'tracking.enabled' });
  });

  it('sets setting through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(setSetting('tracking.enabled', true, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('set_setting', { key: 'tracking.enabled', value: true });
  });

  it('deletes setting through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    await expect(deleteSetting('tracking.enabled', invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('delete_setting', { key: 'tracking.enabled' });
  });

  it('drains engine events through invoke', async () => {
    const events = [{ SyncQueued: { service: 'anilist', anime_id: 1 } }];
    const invoke = vi.fn().mockResolvedValue(events);
    await expect(drainEngineEvents(invoke)).resolves.toEqual(events);
    expect(invoke).toHaveBeenCalledWith('drain_engine_events');
  });

  it('drains engine events with PlaybackDetected shape', async () => {
    const events = [
      {
        PlaybackDetected: {
          player_name: 'mpv.exe',
          file_path: 'C:/anime/ep01.mkv',
          window_title: null,
          episode_guess: 1,
          candidates: [{ anime_id: 1, title: 'Test Anime', confidence: 45, match_source: 'library' }],
          detected_at_unix: 1719000000,
        },
      },
    ];
    const invoke = vi.fn().mockResolvedValue(events);
    const result = await drainEngineEvents(invoke);
    expect(result).toHaveLength(1);
    const event = result[0]!;
    if ('PlaybackDetected' in event) {
      expect(event.PlaybackDetected.player_name).toBe('mpv.exe');
      expect(event.PlaybackDetected.episode_guess).toBe(1);
      expect(event.PlaybackDetected.candidates).toHaveLength(1);
      expect(event.PlaybackDetected.candidates).toEqual([
        { anime_id: 1, title: 'Test Anime', confidence: 45, match_source: 'library' },
      ]);
    } else {
      throw new Error('expected PlaybackDetected event');
    }
  });

  it('gets tracking status', async () => {
    const status = { active: false, watching: null };
    const invoke = vi.fn().mockResolvedValue(status);
    await expect(getTrackingStatus(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('get_tracking_status');
  });

  it('starts tracking', async () => {
    const status = { active: true, watching: null };
    const invoke = vi.fn().mockResolvedValue(status);
    await expect(startTracking(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('start_tracking');
  });

  it('stops tracking', async () => {
    const status = { active: false, watching: null };
    const invoke = vi.fn().mockResolvedValue(status);
    await expect(stopTracking(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('stop_tracking');
  });

  it('marks episode watched', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(markEpisodeWatched(1, 5, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('mark_episode_watched', { anime_id: 1, episode: 5 });
  });

  it('lists recent history', async () => {
    const history = [{ id: 1, anime_id: 1, episode: 5, file_path: null, player: 'manual', watched_at: 1782769008 }];
    const invoke = vi.fn().mockResolvedValue(history);
    await expect(listRecentHistory(10, invoke)).resolves.toEqual(history);
    expect(invoke).toHaveBeenCalledWith('list_recent_history', { limit: 10 });
  });

  it('identifies file through invoke', async () => {
    const result = { known_file: false, parsed: null, candidates: [] };
    const invoke = vi.fn().mockResolvedValue(result);
    await expect(identifyFile('test.mkv', null, invoke)).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledWith('identify_file', { file_path: 'test.mkv', window_title: null });
  });

  it('confirms identification', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(confirmIdentification('test.mkv', 1, 5, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('confirm_identification', { file_path: 'test.mkv', anime_id: 1, episode: 5 });
  });

  it('lists known files', async () => {
    const entries = [{ file_path: 'test.mkv', anime_id: 1, episode: 1, confidence: 100, indexed_at: 1782769008 }];
    const invoke = vi.fn().mockResolvedValue(entries);
    await expect(listKnownFiles(10, invoke)).resolves.toEqual(entries);
    expect(invoke).toHaveBeenCalledWith('list_known_files', { limit: 10 });
  });

  it('stores anilist token', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(storeAniListToken('tok', invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('store_anilist_token', { token: 'tok' });
  });

  it('disconnects anilist', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(disconnectAniList(invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('disconnect_anilist');
  });

  it('imports anilist library', async () => {
    const report = { imported: 1, merged: 1, skipped: 0 };
    const invoke = vi.fn().mockResolvedValue(report);
    await expect(importAniListLibrary(invoke)).resolves.toEqual(report);
    expect(invoke).toHaveBeenCalledWith('import_anilist_library');
  });

  it('gets sync status', async () => {
    const status = { pending: 1, failed: 0, blocked: 0, last_sync_at: null };
    const invoke = vi.fn().mockResolvedValue(status);
    await expect(getSyncStatus(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('get_sync_status');
  });

  it('searches library through invoke', async () => {
    const entries = [
      { anime_id: 1, title: 'Cowboy Bebop', status: 'watching', watched_episodes: 5, episode_count: 26, score: 8, image_url: null },
    ];
    const invoke = vi.fn().mockResolvedValue(entries);
    await expect(searchLibrary('bebop', 'watching', 10, 0, invoke)).resolves.toEqual(entries);
    expect(invoke).toHaveBeenCalledWith('search_library', { query: 'bebop', statusFilter: 'watching', limit: 10, offset: 0 });
  });

  it('gets library stats through invoke', async () => {
    const stats = { total: 10, watching: 3, completed: 4, on_hold: 1, dropped: 1, plan_to_watch: 1 };
    const invoke = vi.fn().mockResolvedValue(stats);
    await expect(getLibraryStats(invoke)).resolves.toEqual(stats);
    expect(invoke).toHaveBeenCalledWith('get_library_stats');
  });

  it('fetches anime detail through invoke', async () => {
    const detail = {
      anime_id: 1, titles_json: '{}', episode_count: 26, image_url: null, synopsis: '...', anime_status: 'finished',
      last_modified: 0, list_status: 'watching', watched_episodes: 12, score: null, notes: null,
      local_updated: null, remote_updated: null, tracker_id: null, recent_history: [],
    };
    const invoke = vi.fn().mockResolvedValue(detail);
    await expect(fetchAnimeDetail(1, invoke)).resolves.toEqual(detail);
    expect(invoke).toHaveBeenCalledWith('fetch_anime_detail', { anime_id: 1 });
  });

  it('updates list entry through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(updateListEntry(1, { watched_episodes: 7 }, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('update_list_entry', { anime_id: 1, status: null, watched_episodes: 7, score: null });
  });

  it('gets session state through invoke', async () => {
    const state = { paused: false };
    const invoke = vi.fn().mockResolvedValue(state);
    await expect(getSessionState(invoke)).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith('get_session_state');
  });

  it('toggles pause tracking through invoke', async () => {
    const state = { paused: true };
    const invoke = vi.fn().mockResolvedValue(state);
    await expect(togglePauseTracking(invoke)).resolves.toEqual(state);
    expect(invoke).toHaveBeenCalledWith('toggle_pause_tracking');
  });

  it('gets launch on startup through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    await expect(getLaunchOnStartup(invoke)).resolves.toBe(true);
    expect(invoke).toHaveBeenCalledWith('get_launch_on_startup');
  });

  it('sets launch on startup through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(setLaunchOnStartup(true, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('set_launch_on_startup', { enabled: true });
  });

  it('connects sonarr through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(connectSonarr('http://localhost:8989', 'key123', invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('connect_sonarr', { url: 'http://localhost:8989', apiKey: 'key123' });
  });

  it('disconnects sonarr through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(disconnectSonarr(invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('disconnect_sonarr');
  });

  it('gets sonarr status through invoke', async () => {
    const status = { connected: true, series_count: 10, mapped_count: 8, last_sync_at: null };
    const invoke = vi.fn().mockResolvedValue(status);
    await expect(getSonarrStatus(invoke)).resolves.toEqual(status);
    expect(invoke).toHaveBeenCalledWith('get_sonarr_status');
  });

  it('imports sonarr series through invoke', async () => {
    const report = { imported: 10, auto_mapped: 8, unmapped: 2 };
    const invoke = vi.fn().mockResolvedValue(report);
    await expect(importSonarrSeries(invoke)).resolves.toEqual(report);
    expect(invoke).toHaveBeenCalledWith('import_sonarr_series');
  });

  it('gets sonarr availability through invoke', async () => {
    const avail = {
      sonarr_id: 1, sonarr_title: 'Test', monitored: true,
      episode_count: 12, episode_file_count: 8, next_airing: null,
      path: '/media', season_count: 1, sonarr_status: 'continuing',
    };
    const invoke = vi.fn().mockResolvedValue(avail);
    await expect(getSonarrAvailability(42, invoke)).resolves.toEqual(avail);
    expect(invoke).toHaveBeenCalledWith('get_sonarr_availability', { anime_id: 42 });
  });

  it('remaps sonarr through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    await expect(remapSonarr(5, 42, invoke)).resolves.toBeUndefined();
    expect(invoke).toHaveBeenCalledWith('remap_sonarr', { sonarr_id: 5, anime_id: 42 });
  });
});
