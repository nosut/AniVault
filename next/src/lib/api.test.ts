import { describe, expect, it, vi } from 'vitest';
import {
  deleteSetting,
  drainEngineEvents,
  getEngineStatus,
  getSetting,
  previewMigrationReport,
  setSetting,
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
});
