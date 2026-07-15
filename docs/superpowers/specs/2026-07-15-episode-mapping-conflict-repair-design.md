# Episode Mapping Conflict Repair - Design

Date: 2026-07-15
Status: Approved

## Problem

The File Management view groups files from their parsed filename and season marker, but
the library detail view lists files by their persisted AniList ID. A group can therefore
look like a single season while its rows are mapped to different anime. In the reported
case, episode 1 was mapped to Skeleton Knight Season 2 (`185542`) and episode 2 was
mapped at 100% confidence to the base series (`132474`). The Season 2 detail query
correctly returned only episode 1.

The existing scanner change only helps files that are unmatched when evaluated. Targeted
rescan skips every existing row with confidence greater than zero, so it cannot repair an
already-confident incorrect mapping. Confidence is also insufficient as a protection
boundary because both an exact automatic match and an explicit manual mapping can have
confidence 100.

## Goals

- Preserve ordinary rescan as a non-destructive operation.
- Detect plausible conflicting mappings beside files already mapped to the selected anime.
- Require explicit confirmation before repairing any existing mapping.
- Never overwrite a new explicit manual mapping through the repair operation.
- Repair eligible legacy records so existing installations benefit from the patch.
- Show the real mapped anime title and mapping origin in File Management.
- Surface rescan and repair failures instead of silently discarding them.
- Ship the backward-compatible fix as version `1.0.2` with a new NSIS installer.

## Non-Goals

- Automatically remap an entire recursively nested show directory during rescan.
- Override protected manual mappings; Map Folder remains the explicit broad override.
- Merge AniList entries or infer relationships between separate anime records.
- Redesign File Management or the Episode Files section beyond conflict clarity and repair.

## Data Model

Add migration `0007_file_index_mapping_source.sql` with a non-null
`mapping_source` text column. Existing rows are backfilled by the column default as
`legacy`. Accepted application values are:

- `automatic`: title-based library scanning or automatic playback recognition.
- `inherited`: a match inherited from unanimous direct siblings in one directory.
- `manual`: File Management mapping, Map Folder, or user-confirmed playback identity.
- `legacy`: a row created before mapping provenance was recorded.

Represent these values in Rust with a `MappingSource` enum that owns serialization and
database string conversion. `FileIndexRow` exposes the source to commands and the
frontend. Unknown database values fail closed as `legacy` so they remain confirmation-
gated and are never treated as manual or silently repaired.

Replace the tuple `FileMatch` with a struct containing `anime_id`, `episode`,
`confidence`, and `mapping_source`. Every file-index write path must provide a source;
there is no implicit default in the Rust storage API. Batch manual mapping always writes
`manual`.

The Re-match unmapped command processes only rows whose `anime_id` is null. It may not
rewrite an existing automatic, inherited, legacy, or manual mapping; existing mapping
changes go through confirmed repair or an explicit Map Folder action.

## Conflict Detection

Normal full scans, watcher scans, and targeted rescans keep their current add, skip, and
prune semantics. Targeted `rescan_anime_dirs(storage, anime_id)` gains a read-only conflict
detection phase after indexing and pruning:

1. Derive distinct parent directories from real files already mapped to `anime_id`.
2. Inspect only video files directly inside each derived directory. Nested files are not
   candidates merely because a parent directory was scanned.
3. Load each existing non-ignored row mapped to another anime.
4. Build the same filename, parent, and grandparent title queries used by matching. When
   the parser produced a usable filename title, require that title itself to score against
   the selected anime at the normal match threshold. Only a generic filename with no
   usable title may fall back to parent and grandparent directory queries.
5. Report the row when that filename-first plausibility rule succeeds.

Each reported conflict contains the file path, parsed episode, current anime ID, current
anime title, mapping source, and whether it is repairable. Sources `automatic`,
`inherited`, and `legacy` are repairable after confirmation. Source `manual` is reported
as protected and is never eligible for this repair command.

`LibraryScanReport` gains `mapping_conflicts: Vec<FileMappingConflict>`. This field is
empty for scan modes that do not perform selected-anime conflict detection. Existing scan
counts retain their current meanings; detection does not increment `indexed`.

## Confirmed Repair

Add backend command `repair_anime_file_mappings(anime_id)`. The frontend sends only the
selected anime ID, not candidate paths or replacement IDs. The command reruns conflict
detection against current database and filesystem state to prevent stale UI data from
authorizing an obsolete change.

For every still-repairable conflict, the command:

- reparses the episode number from the current filename, preserving the existing episode
  only when parsing yields no positive episode;
- writes the selected anime ID;
- records confidence using the selected anime's current title score;
- records mapping source `manual`, because the user explicitly confirmed this repair;
- leaves ignored, missing, weak-match, nested, and protected manual rows unchanged.

The command returns a `FileMappingRepairReport` with `repaired`, `skipped`, and
`protected` counts. It is idempotent: running it again after a successful repair reports
zero repaired rows.

Map Folder remains the intentional override when a user wants to map an entire recursive
folder, including any protected manual conflicts.

## File Management Clarity

The filename-derived series and season heading remains useful for grouping, but it must
not imply database ownership. The known-file list response is enriched with the mapped
anime title. Each mapped row displays:

`<anime title> (#<id>) - Ep <n> - <confidence>% - <source>`

The group badge remains `Mixed` when active rows have different anime IDs. A mixed group
does not display a single mapped-title badge. Unmapped and ignored rows retain their
existing states.

## Episode Files Interaction

`DetailView.handleRescanFiles` stores the returned scan report and reloads the selected
anime's episode files. When the report contains conflicts, the section displays a compact
warning with each conflict's episode, path, current mapped title and ID, and source.

Repairable conflicts expose a `Repair mappings` action. Selecting it reveals an inline
confirmation stating that eligible files will move to the currently open anime. Confirm
calls `repairAnimeFileMappings(animeId)`, reloads episode files, and shows the actual
repair count. Cancel clears the confirmation without changing mappings.

Protected manual conflicts are labeled as protected and direct the user to Map Folder if
an override is intended. If every conflict is protected, no Repair mappings action is
shown.

Rescan and repair errors are displayed in the Episode Files section. Existing episode
rows remain visible after failure. Starting a new rescan clears the previous transient
success message but does not clear loaded episode data.

## Error Handling and Safety

- Conflict detection is read-only and cannot partially remap files.
- Repair candidates are recomputed server-side immediately before writes.
- Repair writes execute in one database transaction so eligible mappings change together.
- A missing or unreadable directory produces a skipped candidate or scan error, never a
  deletion or forced mapping.
- Manual and ignored rows are protected at the final transactional update as well as the
  detection layer.
- The direct-child guard prevents an anchor in a season folder from claiming files in a
  Specials or unrelated nested directory.
- Filename-first title similarity prevents an unrelated direct sibling from being offered
  for repair merely because it lives inside the selected anime's directory.

## Testing

### Storage and migration

- Migration 0007 preserves existing file-index rows and gives them source `legacy`.
- File-index reads serialize mapping source and mapped anime title.
- Single and batch writes persist each explicit source correctly.
- Re-match unmapped maps null rows but never rewrites an existing mapped row.
- Transactional repair rechecks source and ignored state.

### Scanner and commands

- Exact regression: episode 1 maps to `185542`; episode 2 maps at confidence 100 to
  `132474`; targeted rescan reports episode 2 without changing it; confirmed repair maps
  episode 2 to `185542`; querying `185542` returns episodes 1 and 2.
- Automatic, inherited, and legacy conflicts are repairable after confirmation.
- Manual conflicts are reported protected and remain unchanged.
- Ignored files, nested files, missing files, and unrelated weak-title files remain
  unchanged.
- Repair revalidates changed candidates and is idempotent.
- Existing prune, offline-root, folder inheritance, and confident unrelated-title tests
  continue to pass.

### Frontend

- API wrappers use the exact rescan conflict and repair report shapes.
- File Management renders actual mapped title, ID, episode, confidence, and source.
- Mixed filename groups remain visibly mixed.
- Detail View presents repairable and protected conflicts correctly.
- Confirm repair reloads episode files and displays the returned count.
- Cancel performs no mutation.
- Rescan and repair errors remain visible while existing episode rows remain rendered.

## Release and Verification

This is a backward-compatible bug fix, so SemVer increments `1.0.1` to `1.0.2` across
the npm, Cargo, and Tauri version declarations and lockfiles.

Verification order:

1. Focused migration, storage, mapping, rescan, command, API, and component tests.
2. Complete Rust test suite.
3. Complete frontend test and production build.
4. NSIS installer build.
5. Record installer path, byte size, and SHA-256 checksum.
