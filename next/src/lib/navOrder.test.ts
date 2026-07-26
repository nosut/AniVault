// @vitest-environment jsdom
import { describe, expect, it, beforeEach } from 'vitest';
import {
  DEFAULT_NAV_ITEMS,
  clearNavOrder,
  loadNavOrder,
  moveNavItem,
  reconcile,
  saveNavOrder,
  type NavId,
} from './navOrder';

const DEFAULT_ORDER = DEFAULT_NAV_ITEMS.map((i) => i.id);

describe('sidebar nav order', () => {
  beforeEach(() => localStorage.clear());

  it('defaults to the shipped order when nothing is stored', () => {
    expect(loadNavOrder()).toEqual(DEFAULT_ORDER);
    expect(DEFAULT_ORDER[0]).toBe('dashboard');
    expect(DEFAULT_ORDER).toHaveLength(9);
  });

  it('round-trips a saved order', () => {
    const custom: NavId[] = ['settings', 'library', 'dashboard', 'collection',
      'season', 'search', 'calendar', 'history', 'stats'];
    saveNavOrder(custom);
    expect(loadNavOrder()).toEqual(custom);
  });

  it('drops ids that are not real nav items', () => {
    expect(reconcile(['library', 'detail', 'garbage', 'dashboard']).slice(0, 2))
      .toEqual(['library', 'dashboard']);
    expect(reconcile(['library', 'detail'])).not.toContain('detail');
  });

  it('appends nav items missing from the stored order, in default order', () => {
    // Simulates a stored order written before new items existed.
    const result = reconcile(['stats', 'library']);
    expect(result.slice(0, 2)).toEqual(['stats', 'library']);
    expect(result).toHaveLength(DEFAULT_ORDER.length);
    for (const id of DEFAULT_ORDER) expect(result).toContain(id);
    // The appended tail keeps default relative order.
    expect(result.slice(2)).toEqual(
      DEFAULT_ORDER.filter((id) => id !== 'stats' && id !== 'library'),
    );
  });

  it('collapses duplicate ids, keeping the first occurrence', () => {
    const result = reconcile(['library', 'library', 'dashboard']);
    expect(result.filter((id) => id === 'library')).toHaveLength(1);
    expect(result.slice(0, 2)).toEqual(['library', 'dashboard']);
  });

  it('falls back to defaults for corrupt or non-array values', () => {
    localStorage.setItem('anivault-nav-order', 'not json{');
    expect(loadNavOrder()).toEqual(DEFAULT_ORDER);
    expect(reconcile('library')).toEqual(DEFAULT_ORDER);
    expect(reconcile(null)).toEqual(DEFAULT_ORDER);
    expect(reconcile({ id: 'library' })).toEqual(DEFAULT_ORDER);
  });

  it('clears back to the default order', () => {
    saveNavOrder(['settings', 'dashboard']);
    clearNavOrder();
    expect(loadNavOrder()).toEqual(DEFAULT_ORDER);
  });

  it('moves an item down to the given landing index', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection', 'season'];
    expect(moveNavItem(order, 0, 2))
      .toEqual(['library', 'collection', 'dashboard', 'season']);
  });

  it('moves an item up to the given landing index', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection', 'season'];
    expect(moveNavItem(order, 3, 1))
      .toEqual(['dashboard', 'season', 'library', 'collection']);
  });

  it('clamps out-of-range targets and ignores an invalid source', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection'];
    expect(moveNavItem(order, 0, 99))
      .toEqual(['library', 'collection', 'dashboard']);
    expect(moveNavItem(order, 2, -5))
      .toEqual(['collection', 'dashboard', 'library']);
    expect(moveNavItem(order, 7, 0)).toEqual(order);
  });

  it('is a no-op when from equals to, and never mutates its input', () => {
    const order: NavId[] = ['dashboard', 'library', 'collection'];
    expect(moveNavItem(order, 1, 1)).toEqual(order);
    moveNavItem(order, 0, 2);
    expect(order).toEqual(['dashboard', 'library', 'collection']);
  });
});
