# Brand Assets Refresh Design

## Summary

Refresh Kairox's public brand assets so they better match the project identity: a local-first AI agent workbench with a shared Rust core, explicit permissions, event-sourced runtime, and both TUI and desktop surfaces.

The approved direction is "Workbench Signal": restrained developer-tool branding with a compact AI accent. The logo should work as an app icon. The banner should carry the README's main visual story. The README should not stack the banner and standalone logo back to back.

## Current State

- `docs/assets/logo.svg` is a dark rounded square with a gradient K monogram.
- `docs/assets/banner.svg` repeats the logo, includes decorative grid/glow elements, and hardcodes the current release version.
- `README.md` shows `banner.svg`, badges, intro text, then `logo.svg`, which makes the first screen feel visually crowded and repetitive.
- `apps/agent-gui/src-tauri/icons/` contains PNG, ICO, ICNS, iOS, and Android icons derived from the logo.

## Goals

- Make `logo.svg` a stronger source asset for app icons at small sizes.
- Make `banner.svg` communicate Kairox's workbench concept without relying on stale version text.
- Keep the visual system recognizable across docs, app icons, and release assets.
- Reduce README top-of-page crowding by using the banner as the single primary brand moment.
- Regenerate Tauri icon assets from the refreshed logo source.

## Non-Goals

- No application UI redesign.
- No changes to Rust, Vue, or Tauri IPC behavior.
- No generated TypeScript binding changes.
- No release version bump.
- No new raster marketing screenshots.

## Visual Direction

### Logo

The logo remains a 256x256 SVG, but the monogram should be tightened for icon use:

- Dark, rounded app tile background.
- Geometric K shape with enough internal spacing to survive 16px and 32px sizes.
- Cyan, blue, and violet gradient accent used sparingly.
- No text in the logo source.
- Accessible `<title>` and `<desc>` metadata retained.

### Banner

The banner remains a 1280x640 SVG and becomes the README's main brand surface:

- Integrate the logo mark inside the banner instead of requiring a second logo image below it.
- Show restrained workbench geometry: panes, trace lines, task/tool blocks, or event-flow nodes.
- Use the same palette as the logo, but avoid a one-note blue/purple glow-heavy composition.
- Remove hardcoded release version text so the asset does not age.
- Preserve accessible `<title>` and `<desc>` metadata.

### README Layout

`README.md` should keep the top-level title, banner, badges, and introduction, then proceed into links and architecture content. The standalone logo image should be removed from this position because it repeats the banner mark too soon.

The standalone logo remains available in `docs/assets/logo.svg` for external reuse, package/release surfaces, and icon generation.

## Asset Generation

After updating `docs/assets/logo.svg`, regenerate Tauri icons under `apps/agent-gui/src-tauri/icons/` from the refreshed logo source. Prefer the existing Tauri icon generator if available. If the generator cannot consume SVG directly, render a high-resolution PNG source from the SVG first, then feed that source into the generator.

Generated icon outputs to update include:

- Root desktop assets such as `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.ico`, and `icon.icns`.
- Windows store logo PNGs.
- iOS app icon PNGs.
- Android launcher PNGs and any XML color resource if the background color changes.

## Testing And Verification

- Render or inspect `docs/assets/logo.svg` and `docs/assets/banner.svg` after editing.
- Verify README top matter no longer shows banner and logo consecutively.
- Verify regenerated icon files exist and have the expected dimensions.
- Run formatting checks for Markdown and SVG-related changes where supported by repository tooling.
- Avoid running unrelated heavy build steps unless asset generation or formatting exposes a reason.

## Risks

- The icon generator may rasterize gradients differently across PNG, ICO, and ICNS outputs. Mitigation: inspect representative generated files at small and large sizes.
- A visually detailed banner can become unreadable in GitHub README width constraints. Mitigation: keep main text large and avoid dense detail near the center-left title area.
- SVG text rendering can vary by environment. Mitigation: use standard system font stacks for banner text and keep text minimal.

## Acceptance Criteria

- `docs/assets/logo.svg` and `docs/assets/banner.svg` reflect the approved Workbench Signal direction.
- `README.md` no longer displays the standalone logo immediately after the intro.
- Tauri icon assets are regenerated from the refreshed logo.
- No generated icon or README changes introduce broken paths.
- Verification commands complete with documented results.
