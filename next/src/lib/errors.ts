// Turns a raw error into something a user can act on. Every error shown in
// the app goes through explainError: the screen supplies a plain sentence
// saying what didn't work, this module adds what to do about it based on the
// likely cause, and the raw text is kept for ErrorNotice's "Show details".

export type ErrorCause =
  | 'offline'
  | 'sonarr-unreachable'
  | 'sonarr-not-connected'
  | 'sonarr-rejected'
  | 'sonarr-import-list'
  | 'sonarr-not-linked'
  | 'anilist-login'
  | 'anilist-busy'
  | 'oauth-timeout'
  | 'port-in-use'
  | 'service-down'
  | 'file-missing'
  | 'file-locked'
  | 'no-permission'
  | 'disk-full'
  | 'database-busy'
  | 'not-a-backup'
  | 'no-old-data'
  | 'drive-root'
  | 'startup-setting'
  | 'damaged-setting'
  | 'unknown';

export interface ExplainedError {
  /** Plain words for the user: what failed, then what to do. */
  message: string;
  /** The raw error, for "Show details" and bug reports. */
  details: string;
}

/** The raw text of a thrown value: Error message, string, or JSON. */
export function errorDetail(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === 'string') return e;
  try { return JSON.stringify(e); } catch { return String(e); }
}

const SONARR_HINT = /sonarr|\/api\/v3\//i;
const HTTP_STATUS = /\bHTTP (\d{3})\b|\bstatus(?: code)?:? (\d{3})\b|\b(\d{3}) (?:Unauthorized|Forbidden|Too Many Requests|Internal Server Error|Bad Gateway|Service Unavailable)\b/i;

function httpStatus(raw: string): number | null {
  const m = raw.match(HTTP_STATUS);
  const code = m?.[1] ?? m?.[2] ?? m?.[3];
  return code ? Number(code) : null;
}

/** The most likely cause behind a raw error, from its wording. */
export function errorCause(raw: string): ErrorCause {
  const status = httpStatus(raw);
  const sonarr = SONARR_HINT.test(raw);

  // Messages AniVault's backend writes itself.
  if (/rejected the saved login|not connected to AniList|invalid token|token (?:has )?expired/i.test(raw)) return 'anilist-login';
  if (/OAuth timed out/i.test(raw)) return 'oauth-timeout';
  if (/Port \d+ is in use/i.test(raw)) return 'port-in-use';
  if (/Sonarr (?:is )?not connected|^not connected$/i.test(raw)) return 'sonarr-not-connected';
  if (/AniList import list in Sonarr/i.test(raw)) return 'sonarr-import-list';
  if (/not linked to a Sonarr series/i.test(raw)) return 'sonarr-not-linked';
  if (/does not look like a SQLite database|too small to be a SQLite database/i.test(raw)) return 'not-a-backup';
  if (/No v1 data found/i.test(raw)) return 'no-old-data';
  if (/Refusing to scan a filesystem root/i.test(raw)) return 'drive-root';
  if (/registry key/i.test(raw)) return 'startup-setting';
  if (/saved .* is damaged/i.test(raw)) return 'damaged-setting';

  // HTTP responses.
  if (status === 401 || status === 403) return sonarr ? 'sonarr-rejected' : 'anilist-login';
  if (status === 429 || /too many requests|rate limit/i.test(raw)) return 'anilist-busy';
  if (status !== null && status >= 500) return 'service-down';

  // Network failures.
  if (/error sending request|connection refused|os error 10061|os error 10060|timed out|dns error|failed to lookup address|no such host|network is unreachable|os error 1003|connect error/i.test(raw)) {
    return sonarr ? 'sonarr-unreachable' : 'offline';
  }

  // Files and storage.
  if (/os error 32\b|being used by another process/i.test(raw)) return 'file-locked';
  if (/os error 5\b|access is denied|permission denied/i.test(raw)) return 'no-permission';
  if (/os error 112\b|not enough space|disk (?:is )?full|database or disk is full/i.test(raw)) return 'disk-full';
  if (/database is locked|database is busy/i.test(raw)) return 'database-busy';
  if (/os error [23]\b|cannot find the (?:file|path)|Folder does not exist|Folder not found|Backup file not found|No such file or directory/i.test(raw)) return 'file-missing';

  return 'unknown';
}

const ADVICE: Record<ErrorCause, string> = {
  'offline': "AniVault couldn't connect to the internet. Check your connection and try again.",
  'sonarr-unreachable': "AniVault couldn't reach Sonarr. Make sure Sonarr is running and its address in Settings is right.",
  'sonarr-not-connected': "Sonarr isn't connected yet. Connect it in Settings first.",
  'sonarr-rejected': 'Sonarr turned the request down. Check the API key in Settings.',
  'sonarr-import-list': 'This needs an AniList import list in Sonarr with a root folder and quality profile. Set one up in Sonarr, then try again.',
  'sonarr-not-linked': "This show isn't linked to a series in Sonarr yet.",
  'anilist-login': 'Your AniList login has expired or was removed. Reconnect AniList in Settings.',
  'anilist-busy': 'The service is limiting requests right now. Wait a minute, then try again.',
  'oauth-timeout': "The AniList sign-in wasn't finished in time. Start connecting again.",
  'port-in-use': 'Another copy of AniVault seems to be running. Close it, then try again.',
  'service-down': 'The service had a problem on its end. Try again in a few minutes.',
  'file-missing': "The file or folder couldn't be found. It may have been moved, renamed or deleted.",
  'file-locked': 'A file is open in another program. Close it, then try again.',
  'no-permission': "Windows didn't allow AniVault to do this. Check that the folder isn't read-only or protected.",
  'disk-full': 'The drive is full. Free up some space, then try again.',
  'database-busy': "AniVault's database was busy. Try again in a moment.",
  'not-a-backup': "That file isn't an AniVault backup. Choose a different file.",
  'no-old-data': 'No data from the old version of AniVault was found on this PC.',
  'drive-root': 'Choose a specific folder rather than a whole drive.',
  'startup-setting': "Windows didn't let AniVault change its startup setting. Try again, or check whether security software is blocking it.",
  'damaged-setting': "That saved setting is damaged. Set it again to replace it.",
  'unknown': 'Try again. If it keeps happening, Show details has the technical information to report.',
};

/** What a user should do about a cause, in plain words. */
export function causeAdvice(cause: ErrorCause): string {
  return ADVICE[cause];
}

/**
 * Pair a screen's plain summary ("Your library couldn't be loaded.") with
 * advice for the likely cause. The raw error never appears in the message.
 */
export function explainError(summary: string, e: unknown): ExplainedError {
  const details = errorDetail(e);
  return { message: `${summary} ${causeAdvice(errorCause(details))}`, details };
}

/** One step of a multi-step action that failed, described in plain words. */
export interface StepFailure {
  /** What didn't happen, e.g. "Sonarr couldn't unmonitor Frieren." */
  what: string;
  error: unknown;
}

/**
 * Several steps that failed together: a summary with advice for the most
 * common cause, the plain list of what didn't happen, and every raw error
 * in the details.
 */
export function explainFailures(summary: string, failures: StepFailure[]): ExplainedError & { items: string[] } {
  const raws = failures.map((f) => errorDetail(f.error));
  const counts = new Map<ErrorCause, number>();
  for (const raw of raws) {
    const cause = errorCause(raw);
    counts.set(cause, (counts.get(cause) ?? 0) + 1);
  }
  const [top] = [...counts.entries()].sort((a, b) => b[1] - a[1]);
  return {
    message: `${summary} ${causeAdvice(top?.[0] ?? 'unknown')}`,
    items: failures.map((f) => f.what),
    details: failures.map((f, i) => `${f.what}\n  ${raws[i]}`).join('\n'),
  };
}

/** A library scan's per-folder problems, which arrive as raw strings. */
export function explainScanErrors(errors: string[]): ExplainedError & { items: string[] } {
  const failures: StepFailure[] = errors.map((raw) => {
    let m: RegExpMatchArray | null;
    if ((m = raw.match(/^Folder not found: (.+)$/))) return { what: `The folder "${m[1]}" couldn't be found.`, error: raw };
    if ((m = raw.match(/^Cannot read (.+?): /))) return { what: `"${m[1]}" couldn't be read.`, error: raw };
    if ((m = raw.match(/exceeded at (.+)$/))) return { what: `"${m[1]}" is nested too deeply, so it was skipped.`, error: raw };
    return { what: 'Part of a folder couldn\'t be scanned.', error: raw };
  });
  return explainFailures("Some folders couldn't be scanned.", failures);
}
