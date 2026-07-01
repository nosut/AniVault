import { describe, expect, it, vi } from 'vitest';
import {
  deleteSetting,
  drainEngineEvents,
  getEngineStatus,
  getSetting,
  getTrackingStatus,
  listRecentHistory,
  markEpisodeWatched,
  previewMigrationReport,
  setSetting,
  startTracking,
  stopTracking,
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
});
