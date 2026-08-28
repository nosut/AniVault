# AniVault

Windows-only Tauri desktop app for managing a local anime library. The active
codebase lives in `next/` (Svelte frontend in `next/src`, Rust backend in
`next/src-tauri`).

## Releasing

**Every user-facing change ships with a version bump. There is no reason not to.**
**But never build or release unprompted: always ask the user before building the
installer / pushing / creating the release.** The user often makes several
changes in a row, one pass at a time, and decides when a build cuts. The order,
once they say go, is: build → git push → GitHub release.

When a change (feature, fix, UI tweak) is finished and the user has confirmed
they want a build, bump the patch version in the same piece of work:

1. Bump the version in all four places (they must match):
   - `next/package.json` and `next/package-lock.json` (`npm version <x.y.z> --no-git-tag-version` in `next/`)
   - `next/src-tauri/Cargo.toml`
   - `next/src-tauri/tauri.conf.json`
   (`next/src-tauri/Cargo.lock` refreshes on the next build — commit it too.)
2. Build the installer: `npm run bundle` in `next/` (or run
   `next/scripts/bundle.ps1` directly if `pwsh` is not on PATH). Output lands in
   `next/src-tauri/target/release/bundle/nsis/AniVault_<version>_x64-setup.exe`.
3. Commit feature work first, then a `chore: release <version>` commit with the
   version files, tag it `v<version>`, and push branch + tag.
4. Create the GitHub release: `gh release create v<version>` with the installer
   attached, titled `AniVault v<version>`, body starting with
   `> ⚠️ AI-generated project. Windows-only. Install over any previous version.`
   followed by Added/Fixed sections.

## Verification

`npm run verify` in `next/` runs the typecheck, vitest suite, and
`cargo check --tests`. Run the Rust suite with `cargo test` in `next/src-tauri`.
