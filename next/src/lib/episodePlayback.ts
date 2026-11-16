// Which episode to play, and what state each episode is in, for the series
// page, the sidebar Now Playing card and the Now Playing page. Pure, so all
// three agree on "next".
import type { FileIndexEntry, RecentHistoryEntry } from './api';

export type EpisodeState = 'watched' | 'playing' | 'next' | 'unwatched' | 'missing';

export interface EpisodeTile {
  season: number;
  episode: number;
  state: EpisodeState;
  /** Null for an episode with no file on disk. */
  file: FileIndexEntry | null;
}

export interface SeasonTiles {
  season: number;
  tiles: EpisodeTile[];
}

export type ContinueKind = 'rewatch' | 'continue' | 'restart';

export interface ContinueTarget {
  episode: number;
  file: FileIndexEntry;
  kind: ContinueKind;
}

export function seasonOf(path: string): number {
  const m = path.match(/[\\/ ._-]S(\d{1,2})E\d{1,3}/i) ?? path.match(/[\\/ ]Season\s*(\d{1,2})[\\/ ]/i);
  return m?.[1] ? parseInt(m[1], 10) : 1;
}

/** The file name without its folders, for display. */
export function fileName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

/** Files with a known episode number, sorted by (season, episode). */
export function sortEpisodeFiles(files: FileIndexEntry[]): FileIndexEntry[] {
  return files
    .filter((f) => f.episode != null)
    .sort((a, b) => {
      const sa = seasonOf(a.file_path);
      const sb = seasonOf(b.file_path);
      if (sa !== sb) return sa - sb;
      return (a.episode ?? 0) - (b.episode ?? 0);
    });
}

/** The first file for an episode number (lowest season first). */
export function fileForEpisode(files: FileIndexEntry[], episode: number): FileIndexEntry | null {
  return sortEpisodeFiles(files).find((f) => f.episode === episode) ?? null;
}

/**
 * The episode a Continue button should start.
 *
 * - `rewatch`: the latest watch is below recorded progress, so you are partway
 *   through a rewatch; carry on from the episode after it.
 * - `continue`: the first file after recorded progress.
 * - `restart`: the show is finished (progress reached the episode count) and
 *   nothing is left after progress; start again from the first file.
 *
 * Null when there is nothing to play, including when you are caught up on a
 * show that is still airing (or whose episode count is unknown): the next
 * episode just isn't on disk yet, so don't send you back to episode 1.
 */
export function continueTarget(
  files: FileIndexEntry[],
  watched: number,
  history: RecentHistoryEntry[] = [],
  episodeCount: number | null = null,
): ContinueTarget | null {
  const sorted = sortEpisodeFiles(files);
  if (sorted.length === 0) return null;

  // History arrives newest first.
  const last = history[0]?.episode;
  if (last != null && last < watched) {
    const f = sorted.find((x) => x.episode === last + 1);
    if (f) return { episode: last + 1, file: f, kind: 'rewatch' };
  }

  const after = sorted.find((x) => (x.episode ?? 0) > watched);
  if (after) return { episode: after.episode!, file: after, kind: 'continue' };

  const finished = episodeCount != null && episodeCount > 0 && watched >= episodeCount;
  if (!finished) return null;

  const first = sorted[0]!;
  return { episode: first.episode!, file: first, kind: 'restart' };
}

export function continueLabel(t: ContinueTarget): string {
  switch (t.kind) {
    case 'rewatch': return 'Continue rewatch';
    case 'continue': return 'Continue watching';
    case 'restart': return 'Watch again from the start';
  }
}

export interface TileOptions {
  watched: number;
  episodeCount: number | null;
  /** The episode on screen now, if any. */
  playingEpisode?: number | null;
  /** The episode a "next" marker goes on. */
  nextEpisode?: number | null;
}

/**
 * One tile per episode, grouped by season. For a single-season show, episodes
 * with no file are filled in as `missing` up to the episode count, so gaps are
 * visible. Multi-season numbering is too uncertain to fill.
 */
export function buildEpisodeTiles(files: FileIndexEntry[], opts: TileOptions): SeasonTiles[] {
  const sorted = sortEpisodeFiles(files);
  const bySeason = new Map<number, Map<number, FileIndexEntry>>();
  for (const f of sorted) {
    const s = seasonOf(f.file_path);
    let eps = bySeason.get(s);
    if (!eps) bySeason.set(s, (eps = new Map()));
    // Keep the first file for a duplicated episode number.
    if (!eps.has(f.episode!)) eps.set(f.episode!, f);
  }

  const single = bySeason.size <= 1;
  if (single) {
    const season = [...bySeason.keys()][0] ?? 1;
    const eps = bySeason.get(season) ?? new Map<number, FileIndexEntry>();
    const maxFile = Math.max(0, ...eps.keys());
    const last = Math.max(maxFile, opts.episodeCount ?? 0);
    if (last === 0) return [];
    const tiles: EpisodeTile[] = [];
    for (let e = 1; e <= last; e++) {
      const file = eps.get(e) ?? null;
      tiles.push({ season, episode: e, file, state: stateFor(e, file, opts) });
    }
    return [{ season, tiles }];
  }

  return [...bySeason.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([season, eps]) => ({
      season,
      tiles: [...eps.entries()].map(([episode, file]) => ({
        season,
        episode,
        file,
        state: stateFor(episode, file, opts),
      })),
    }));
}

function stateFor(episode: number, file: FileIndexEntry | null, opts: TileOptions): EpisodeState {
  if (opts.playingEpisode != null && episode === opts.playingEpisode) return 'playing';
  if (!file) return 'missing';
  if (opts.nextEpisode != null && episode === opts.nextEpisode) return 'next';
  if (episode <= opts.watched) return 'watched';
  return 'unwatched';
}

export function stateLabel(state: EpisodeState): string {
  switch (state) {
    case 'watched': return 'Watched';
    case 'playing': return 'Playing';
    case 'next': return 'Up next';
    case 'unwatched': return '';
    case 'missing': return 'No file';
  }
}

/**
 * The playable neighbours of an episode: the closest file before and after it.
 * Gaps are skipped, so Next from episode 14 with no 15 lands on 16.
 */
export function neighbours(files: FileIndexEntry[], episode: number): { prev: FileIndexEntry | null; next: FileIndexEntry | null } {
  const sorted = sortEpisodeFiles(files);
  let prev: FileIndexEntry | null = null;
  let next: FileIndexEntry | null = null;
  for (const f of sorted) {
    const e = f.episode!;
    if (e < episode) prev = f;
    else if (e > episode && !next) next = f;
  }
  return { prev, next };
}

export interface PlaybackControls {
  /** The episode on screen, or the one watched last when nothing plays. */
  current: number | null;
  currentFile: FileIndexEntry | null;
  prev: FileIndexEntry | null;
  next: FileIndexEntry | null;
}

/**
 * Previous / Replay / Next targets. While an episode plays they are its
 * neighbours; otherwise Next is the Continue target and Replay is the episode
 * watched last.
 */
export function playbackControls(
  files: FileIndexEntry[],
  watched: number,
  history: RecentHistoryEntry[],
  playing: number | null,
  episodeCount: number | null = null,
): PlaybackControls {
  if (playing != null) {
    const { prev, next } = neighbours(files, playing);
    return { current: playing, currentFile: fileForEpisode(files, playing), prev, next };
  }
  const last = history[0]?.episode ?? null;
  const target = continueTarget(files, watched, history, episodeCount);
  const currentFile = last != null ? fileForEpisode(files, last) : null;
  return {
    current: last,
    currentFile,
    prev: last != null ? neighbours(files, last).prev : null,
    next: target?.file ?? null,
  };
}
