import type { EngineEvent } from './api';

export interface PromptKey {
  anime_id: number;
  episode: number;
}

/** The most recent ProgressAdvanced in a polled event batch, if any. */
export function latestProgressAdvance(
  events: EngineEvent[],
): { anime_id: number; new_episode: number } | null {
  for (let i = events.length - 1; i >= 0; i--) {
    const ev = events[i];
    if (ev && 'ProgressAdvanced' in ev) {
      return { anime_id: ev.ProgressAdvanced.anime_id, new_episode: ev.ProgressAdvanced.new_episode };
    }
  }
  return null;
}

/** Same anime + episode — used to avoid re-prompting for a prompt already shown. */
export function samePrompt(a: PromptKey | null, b: PromptKey | null): boolean {
  if (!a || !b) return false;
  return a.anime_id === b.anime_id && a.episode === b.episode;
}
