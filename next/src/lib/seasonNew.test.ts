import { describe, expect, it } from 'vitest';
import { partitionNew } from './seasonNew';

const entry = (id: number) => ({ id, title: `Show ${id}` });

describe('partitionNew', () => {
  it('splits entries into new and the rest', () => {
    const entries = [entry(1), entry(2), entry(3)];
    const { fresh, rest } = partitionNew(entries, new Set([2]));
    expect(fresh.map((e) => e.id)).toEqual([2]);
    expect(rest.map((e) => e.id)).toEqual([1, 3]);
  });

  it('preserves the source order within each partition', () => {
    // AniList returns by popularity; the band and the grid must both keep it.
    const entries = [entry(5), entry(4), entry(3), entry(2), entry(1)];
    const { fresh, rest } = partitionNew(entries, new Set([4, 2]));
    expect(fresh.map((e) => e.id)).toEqual([4, 2]);
    expect(rest.map((e) => e.id)).toEqual([5, 3, 1]);
  });

  it('puts everything in rest when nothing is new', () => {
    const entries = [entry(1), entry(2)];
    const { fresh, rest } = partitionNew(entries, new Set());
    expect(fresh).toEqual([]);
    expect(rest.map((e) => e.id)).toEqual([1, 2]);
  });

  it('ignores ids that are not in the listing', () => {
    // A show flagged new can vanish from the next listing; it must not
    // conjure a phantom entry.
    const entries = [entry(1)];
    const { fresh, rest } = partitionNew(entries, new Set([99]));
    expect(fresh).toEqual([]);
    expect(rest.map((e) => e.id)).toEqual([1]);
  });

  it('handles an empty listing', () => {
    const { fresh, rest } = partitionNew([], new Set([1]));
    expect(fresh).toEqual([]);
    expect(rest).toEqual([]);
  });
});
