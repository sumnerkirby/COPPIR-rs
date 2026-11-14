# Releasing

A release is a version tag. The `release` workflow builds the installers on
GitHub's macOS, Windows and Linux machines and attaches them to a draft
release.

## Steps

1. Set `version` in `Cargo.toml`, and in `web/package.json` to match.
2. Run the checks: `cargo test`, `cargo clippy --all-targets`, and in `web/`
   `npm run check && npm test`.
3. Commit, then tag and push:

   ```bash
   git tag v0.2.0
   git push origin main v0.2.0
   ```

4. Wait for **Actions → release** to finish, about 15 minutes. It fails if the
   tag does not match the version in `Cargo.toml`.
5. Open the draft under **Releases**, check the notes, and publish it.

To build the installers without releasing, run the workflow by hand
(**Actions → release → Run workflow**). They appear as artifacts on the run.

## What is built

| Platform | Installer | Built on |
|---|---|---|
| macOS 11+ | Universal `.dmg` (Apple Silicon and Intel in one app) | `macos-latest` |
| Windows 10/11 | NSIS `-setup.exe`, per user, fetches WebView2 if missing | `windows-latest` |
| Linux | `.deb` (needs WebKitGTK 4.1) and an `.AppImage` that bundles it | `ubuntu-22.04` |

[cargo-packager](https://github.com/crabnebula-dev/cargo-packager) makes all
of them from `[package.metadata.packager]` in `Cargo.toml`. The installers are
small because the app uses the system's webview: no browser engine ships
inside.

## Building an installer locally

```bash
cargo install cargo-packager --locked
(cd web && npm ci && npm run build)
cargo build --release
cargo packager --release --formats dmg        # or nsis, or deb,appimage
```

The installer lands in `target/release/`.

## Icons

`packaging/icons/` holds the rendered icons, committed. To change the icon,
edit `packaging/icons/app.svg` (macOS, with Apple's margin) and
`assets/icon.svg` (Windows), then run `python3 packaging/icons/generate.py`.
That needs `resvg` (`cargo install resvg`).

## Signing

Releases are not signed with a certificate. The macOS app is signed ad hoc,
with no identity, so its bundle is sealed: macOS lets a user open it after one
**Open Anyway**, rather than calling it damaged. `docs/release-notes.md`, which
heads every release, tells users what to click on each system.

To sign properly later:

- **macOS**: set `signingIdentity` under `[package.metadata.packager.macos]`
  to a Developer ID Application identity. Import the certificate into the
  runner's keychain. cargo-packager also notarizes when `APPLE_ID`,
  `APPLE_PASSWORD` and `APPLE_TEAM_ID` (or an App Store Connect API key) are
  set.
- **Windows**: set `certificateThumbprint` (or a `signCommand`, for example
  Azure Trusted Signing) under `[package.metadata.packager.windows]`.
