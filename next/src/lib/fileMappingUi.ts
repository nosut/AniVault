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

/**
 * The episode a file is mapped to after applying the dialog's offset. A file
 * whose episode number is unknown stays unknown (0): defaulting it to 1 would
 * map every such file onto the same episode.
 */
export function mappedEpisode(episode: number | null, offset: number): number {
  if (episode == null) return 0;
  return Math.max(0, episode + offset);
}
