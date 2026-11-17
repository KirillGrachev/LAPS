# Release process

## Version

One version for the whole product, in three places:

| File | Field |
| --- | --- |
| `package.json` | `version` |
| `src-tauri/Cargo.toml` | `[package] version` |
| `src-tauri/tauri.conf.json` | `version` |

`npm run check:version` (and the Vitest suite) fails when they diverge. Update
`CHANGELOG.md` in the same commit.

The Cargo lockfile records the crate version too; run
`cargo check --manifest-path src-tauri/Cargo.toml` after the bump so
`Cargo.lock` is refreshed.

## Checklist

1. Land all changes on `main` with green CI.
2. Bump the version in the three files and update `CHANGELOG.md`.
3. Write `docs/release-notes/vX.Y.Z.md` from
   [RELEASE_NOTES_TEMPLATE.md](RELEASE_NOTES_TEMPLATE.md).
4. Run the manual domain checklist in [TESTING.md](TESTING.md).
5. Tag and push:

   ```powershell
   git tag v2.1.0
   git push origin v2.1.0
   ```

6. The `Release` workflow verifies the tag against the version, generates the
   CycloneDX SBOM, builds the MSI and NSIS installers, packages the portable
   build and the PDB symbols, writes `SHA256SUMS-windows-x64.txt` and publishes
   the notes from `docs/release-notes/vX.Y.Z.md`.
7. Verify the release assets, the checksum and the release notes.

## Release assets

| Asset | Contents |
| --- | --- |
| `KMAruda LAPS_X.Y.Z_x64-setup.exe` | NSIS installer, per-user or per-machine, WebView2 bootstrapper |
| `KMAruda LAPS_X.Y.Z_x64_ru-RU.msi` | MSI payload for enterprise deployment |
| `KMAruda.LAPS_X.Y.Z_windows-x64_portable.zip` | The executable without an installer |
| `KMAruda.LAPS_X.Y.Z_windows-x64_symbols.zip` | PDB symbols for crash analysis |
| `npm.cdx.json` | CycloneDX SBOM of the npm dependencies |
| `SHA256SUMS-windows-x64.txt` | SHA-256 of every Windows artifact above |

The portable build still needs the WebView2 Runtime, which current Windows
provides; it does not carry the bootstrapper.

## Signing

The workflow publishes an unsigned installer by default. To sign, add the
certificate to the runner (for example with Azure Trusted Signing or a PFX in
secrets) and configure `bundle.windows.certificateThumbprint` or the
`TAURI_SIGNING_PRIVATE_KEY` variables. Never commit a certificate or a private
key, and never echo secrets in workflow logs. Record the signature status in the
release notes until signing is enabled.

## Release notes

Start from [RELEASE_NOTES_TEMPLATE.md](RELEASE_NOTES_TEMPLATE.md). Group changes
by Features, Fixes and Internal, link every bullet to its commit, end with the
`Full Changelog` compare link and the SmartScreen note. Do not include internal
host names, domain names or account names.

## Rollback

Keep the previous installer and its SHA-256. The application stores no secrets,
so a downgrade only requires reinstalling the older build; the configuration
format is additive within 2.x. If a release is broken, mark it as such on
GitHub and publish a fixed patch version rather than moving the tag.
