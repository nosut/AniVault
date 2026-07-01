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
