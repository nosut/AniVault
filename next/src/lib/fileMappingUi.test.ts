import { describe, expect, it } from 'vitest';
import { knownFileMappingLabel, mappingSourceLabel, partitionMappingConflicts } from './fileMappingUi';

describe('file mapping UI helpers', () => {
  it('shows the actual mapped title rather than the filename group', () => {
    expect(
      knownFileMappingLabel({
        file_path: 'D:/Skeleton Knight/Season 2/Skeleton Knight - S02E02.mkv',
        anime_id: 132474,
        anime_title: 'Skeleton Knight in Another World',
        episode: 2,
        confidence: 100,
        indexed_at: 1,
        ignored: false,
        mapping_source: 'legacy',
      }),
    ).toBe('Skeleton Knight in Another World (#132474) - Ep 2 - 100% - Legacy');
  });

  it('partitions repairable and protected conflicts', () => {
    const conflicts = [
      {
        file_path: 'ep2.mkv',
        episode: 2,
        current_anime_id: 1,
        current_anime_title: 'Base',
        mapping_source: 'legacy' as const,
        target_confidence: 80,
        repairable: true,
      },
      {
        file_path: 'special.mkv',
        episode: 1,
        current_anime_id: 2,
        current_anime_title: 'Special',
        mapping_source: 'manual' as const,
        target_confidence: 80,
        repairable: false,
      },
    ];
    expect(partitionMappingConflicts(conflicts)).toEqual({
      repairable: [conflicts[0]],
      protected: [conflicts[1]],
    });
    expect(mappingSourceLabel('inherited')).toBe('Inherited');
  });

  it('returns no repairable conflicts for protected-only input', () => {
    const manual = {
      file_path: 'ep2.mkv',
      episode: 2,
      current_anime_id: 1,
      current_anime_title: 'Base',
      mapping_source: 'manual' as const,
      target_confidence: 80,
      repairable: false,
    };
    expect(partitionMappingConflicts([manual])).toEqual({ repairable: [], protected: [manual] });
  });
});
