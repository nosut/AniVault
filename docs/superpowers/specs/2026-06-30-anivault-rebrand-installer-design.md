# AniVault Rebrand and Installer Design

Date: 2026-06-30

## Goal

Rebrand the clean-room Taiga Next Phase 0 application as **AniVault** and produce a Windows installer that can be installed beside the user's existing Taiga installation for review.

## Decisions

- Product name: `AniVault`.
- Install identity: separate app from existing Taiga.
- Tauri identifier: `app.anivault.desktop`.
- Existing Taiga install must not be replaced or modified.
- Use supplied root assets:
  - `Icon.png` as the application, window, and installer icon source.
  - `Banner.png` as the home-screen brand banner.
- Keep Phase 0 engine behavior unchanged.
- Produce a local test installer from Tauri's bundler.

## Scope

### In scope

- Rename visible UI text from `Taiga Next` to `AniVault`.
- Update Tauri product/window metadata.
- Enable bundling for local Windows installer output.
- Generate any icon formats Tauri requires from `Icon.png`.
- Copy brand assets into the `next/` application tree.
- Display `Banner.png` on the home view.
- Run verification before and after packaging.
- Provide the installer path to the user.

### Out of scope

- Changing the existing C++ Taiga application.
- Migrating user data.
- Replacing the user's installed Taiga.
- Adding tracking/sync/library functionality beyond Phase 0.
- Publishing or signing the installer.

## Implementation Notes

- Prefer Tauri's native bundle output over a custom NSIS script.
- If Tauri requires icon files such as `.ico` or PNG sizes, generate them from `Icon.png` and keep generated source assets under `next/src-tauri/icons/`.
- Place app-visible images under `next/src/assets/`.
- Keep installer output under Tauri's target directory, normally `next/src-tauri/target/release/bundle/`.
- The installer is for local review only and may be unsigned.

## Verification

- `npm run verify` must pass after rebrand changes.
- `npm run build` must pass.
- Tauri bundle command must produce at least one Windows installer artifact.
- The generated artifact path must be reported.
- Git status must not include unintended generated folders such as `node_modules`, `dist`, `src-tauri/target`, or `src-tauri/gen`.

## Risks

- Tauri bundling may require WiX/NSIS tooling that is not installed. If so, install the required bundler tool or use the bundle target available in this environment.
- Generated icons may be invalid if `Icon.png` cannot be converted correctly. Verify by building the bundle.
- Unsigned installer may trigger Windows SmartScreen; acceptable for local testing.
