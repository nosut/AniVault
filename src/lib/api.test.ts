import { describe, expect, it, vi } from 'vitest';
import { getEngineStatus, previewMigrationReport } from './api';

describe('api wrappers', () => {
  it('gets engine status through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue({ ok: true, database: 'ready' });
    await expect(getEngineStatus(invoke)).resolves.toEqual({ ok: true, database: 'ready' });
    expect(invoke).toHaveBeenCalledWith('get_engine_status');
  });

  it('previews migration report through invoke', async () => {
    const invoke = vi.fn().mockResolvedValue({ imported_anime: 0, skipped_records: 0, warnings: [] });
    await expect(previewMigrationReport(invoke)).resolves.toEqual({ imported_anime: 0, skipped_records: 0, warnings: [] });
    expect(invoke).toHaveBeenCalledWith('preview_migration_report');
  });
});
