import { describe, it, expect } from 'vitest';
import { errorCause, errorDetail, explainError, explainFailures, explainScanErrors, causeAdvice, type ErrorCause } from './errors';

describe('errorCause', () => {
  const cases: [string, ErrorCause][] = [
    ['error sending request for url (https://graphql.anilist.co/): dns error: failed to lookup address', 'offline'],
    ['Sonarr series HTTP 0: error sending request for url (http://192.168.1.5:8989/api/v3/series): connection refused', 'sonarr-unreachable'],
    ['error sending request for url (http://nas:8989/api/v3/system/status): tcp connect error: No connection could be made (os error 10061)', 'sonarr-unreachable'],
    ['Sonarr returned HTTP 401', 'sonarr-rejected'],
    ['Sonarr lookup HTTP 403: Forbidden', 'sonarr-rejected'],
    ['Sonarr is not connected', 'sonarr-not-connected'],
    ['not connected', 'sonarr-not-connected'],
    ['No AniList import list in Sonarr to copy settings from', 'sonarr-import-list'],
    ['The AniList import list in Sonarr has no root folder', 'sonarr-import-list'],
    ['This show is not linked to a Sonarr series', 'sonarr-not-linked'],
    ['AniList rejected the saved login. Reconnect AniList in Settings.', 'anilist-login'],
    ['not connected to AniList', 'anilist-login'],
    ['AniList error: HTTP 429 Too Many Requests', 'anilist-busy'],
    ['AniList error: HTTP 502: Bad Gateway', 'service-down'],
    ['OAuth timed out after 2 minutes', 'oauth-timeout'],
    ['Port 47123 is in use. Close other AniVault instances or change OAUTH_PORT. Error: x', 'port-in-use'],
    ['The process cannot access the file because it is being used by another process. (os error 32)', 'file-locked'],
    ['Access is denied. (os error 5)', 'no-permission'],
    ['There is not enough space on the disk. (os error 112)', 'disk-full'],
    ['error returned from database: (code: 5) database is locked', 'database-busy'],
    ['The system cannot find the file specified. (os error 2)', 'file-missing'],
    ['Folder does not exist: Y:\\Anime', 'file-missing'],
    ['Folder not found: Y:\\Anime', 'folder-offline'],
    ['C:\\x.json does not look like a SQLite database file', 'not-a-backup'],
    ['No v1 data found. Cannot run migration.', 'no-old-data'],
    ['Refusing to scan a filesystem root: C:\\', 'drive-root'],
    ['Failed to write registry key', 'startup-setting'],
    ['The saved library folder list is damaged: expected value at line 1 column 1', 'damaged-setting'],
    ["The saved login couldn't be read on this Windows account: Key not valid for use in specified state. (0x8009000B)", 'saved-login-unreadable'],
    ['pool timed out while waiting for an open connection', 'database-busy'],
    ['error returned from database: (code: 8) attempt to write a readonly database', 'database-unavailable'],
    ['error returned from database: (code: 14) unable to open database file', 'database-unavailable'],
    ['error returned from database: (code: 13) database or disk is full', 'disk-full'],
    ['Invalid AniList token. Make sure you copied the Access Token. Error: AniList HTTP 400: {}', 'anilist-login'],
    ['Invalid AniList token. Make sure you copied the Access Token. Error: error sending request for url (https://graphql.anilist.co/)', 'offline'],
    ['AniList HTTP 401: {"errors":[{"message":"Invalid token","status":401}]}', 'anilist-login'],
    ['AniList HTTP 500: ', 'service-down'],
    ['Sonarr returned HTTP 401 Unauthorized', 'sonarr-rejected'],
    ['error sending request for url (http://localhost:8989/api/v3/system/status)', 'sonarr-unreachable'],
    ['This show is not Watching or Planning', 'not-active'],
    ['Episode 4 not found in Sonarr', 'sonarr-episode-missing'],
    ['Sonarr could not find TVDB id 12345', 'sonarr-show-unknown'],
    [`"Frieren" isn't a TVDB ID or thetvdb.com link`, 'tvdb-input'],
    ['TheTVDB has no series at that link (no-such-show)', 'tvdb-input'],
    ["TheTVDB page for some-show didn't show a series ID", 'tvdb-input'],
    [`Sonarr add series HTTP 400 Bad Request: [{"errorMessage":"'Path' is already configured for an existing series"}]`, 'sonarr-already-has'],
    ['Could not write the startup entry: Access is denied. (os error 5)', 'startup-setting'],
    ["Your web browser couldn't be opened to sign in to AniList (x; y)", 'browser-not-opened'],
    ["Windows couldn't show the notification: x", 'notifications-off'],
    ['(os error 5)', 'no-permission'],
    ['(os error 32)', 'file-locked'],
    ['(os error 2)', 'file-missing'],
    ['something nobody planned for', 'unknown'],
  ];

  it.each(cases)('%s -> %s', (raw, cause) => {
    expect(errorCause(raw)).toBe(cause);
  });

  it('does not mistake "os error 32" for a missing file or "os error 5" inside a longer code', () => {
    expect(errorCause('(os error 52)')).toBe('unknown');
  });
});

describe('errorDetail', () => {
  it('reads Errors, strings and other values', () => {
    expect(errorDetail(new Error('boom'))).toBe('boom');
    expect(errorDetail('plain')).toBe('plain');
    expect(errorDetail({ code: 5 })).toBe('{"code":5}');
    expect(errorDetail(undefined)).toBe('No error details were given.');
    expect(errorDetail(null)).toBe('No error details were given.');
  });
});

describe('explainError', () => {
  it('puts the summary and the advice in the message, and the raw error only in details', () => {
    const raw = 'error returned from database: (code: 5) database is locked';
    const out = explainError("Your library couldn't be loaded.", new Error(raw));
    expect(out.message).toBe(`Your library couldn't be loaded. ${causeAdvice('database-busy')}`);
    expect(out.message).not.toContain('code: 5');
    expect(out.details).toBe(raw);
  });

  it('never leaks technical words into advice', () => {
    const causes: ErrorCause[] = [
      'offline', 'sonarr-unreachable', 'sonarr-not-connected', 'sonarr-rejected', 'sonarr-import-list',
      'sonarr-not-linked', 'anilist-login', 'anilist-busy', 'oauth-timeout', 'port-in-use', 'service-down',
      'file-missing', 'file-locked', 'no-permission', 'disk-full', 'database-busy', 'not-a-backup',
      'no-old-data', 'drive-root', 'startup-setting', 'damaged-setting', 'saved-login-unreadable', 'database-unavailable',
      'not-active', 'sonarr-episode-missing', 'sonarr-show-unknown', 'tvdb-input', 'sonarr-already-has', 'folder-offline', 'browser-not-opened', 'notifications-off', 'unknown',
    ];
    for (const cause of causes) {
      expect(causeAdvice(cause)).not.toMatch(/os error|HTTP|\d{3}|sqlite|oauth|registry|exception|null/i);
    }
  });
});

describe('explainFailures', () => {
  it('lists what failed in plain words and advises on the most common cause', () => {
    const out = explainFailures("Some of the clean-up didn't finish.", [
      { what: "Sonarr couldn't unmonitor Frieren.", error: 'Sonarr series HTTP 0: error sending request for url (http://nas:8989/api/v3/series)' },
      { what: "Sonarr couldn't unmonitor Dandadan.", error: 'error sending request for url (http://nas:8989/api/v3/series/2)' },
      { what: "“ep1.mkv” couldn't be deleted.", error: 'Access is denied. (os error 5)' },
    ]);
    expect(out.message).toBe(`Some of the clean-up didn't finish. ${causeAdvice('sonarr-unreachable')}`);
    expect(out.items).toEqual([
      "Sonarr couldn't unmonitor Frieren.",
      "Sonarr couldn't unmonitor Dandadan.",
      "“ep1.mkv” couldn't be deleted.",
    ]);
    expect(out.details).toContain('os error 5');
    expect(out.message).not.toContain('os error');
  });
});

describe('explainScanErrors', () => {
  it('turns raw scan errors into plain items', () => {
    const out = explainScanErrors([
      'Folder not found: Y:\Anime',
      'Cannot read Y:\Anime\Locked: Access is denied. (os error 5)',
      'Max scan depth (12) exceeded at Y:\Deep',
      'something else',
    ]);
    expect(out.items).toEqual([
      'The folder "Y:\Anime" couldn\'t be found.',
      '"Y:\Anime\Locked" couldn\'t be read.',
      '"Y:\Deep" is nested too deeply, so it was skipped.',
      'Part of a folder couldn\'t be scanned.',
    ]);
    expect(out.message).toMatch(/^Some folders couldn't be scanned\. /);
    expect(out.details).toContain('os error 5');
  });
});
