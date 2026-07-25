const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

/** Human-readable byte size, e.g. 12.4 GB. One decimal below 100 within a unit. */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const i = Math.min(UNITS.length - 1, Math.floor(Math.log(bytes) / Math.log(1024)));
  const val = bytes / Math.pow(1024, i);
  const text = i === 0 || val >= 100 ? String(Math.round(val)) : val.toFixed(1);
  return `${text} ${UNITS[i]}`;
}
