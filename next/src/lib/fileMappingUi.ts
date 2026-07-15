import type { FileMappingConflict, KnownFileEntry, MappingSource } from './api';

export function mappingSourceLabel(source: MappingSource): string {
  return {
    automatic: 'Automatic',
    inherited: 'Inherited',
    manual: 'Manual',
    legacy: 'Legacy',
  }[source];
}

export function knownFileMappingLabel(entry: KnownFileEntry): string {
  if (entry.ignored) return 'Ignored';
  if (entry.anime_id == null) return 'Unmapped';
  const title = entry.anime_title?.trim() || `Anime #${entry.anime_id}`;
  return `${title} (#${entry.anime_id}) - Ep ${entry.episode ?? '?'} - ${entry.confidence}% - ${mappingSourceLabel(entry.mapping_source)}`;
}

export function partitionMappingConflicts(conflicts: FileMappingConflict[]): {
  repairable: FileMappingConflict[];
  protected: FileMappingConflict[];
} {
  return {
    repairable: conflicts.filter((conflict) => conflict.repairable),
    protected: conflicts.filter((conflict) => !conflict.repairable),
  };
}
