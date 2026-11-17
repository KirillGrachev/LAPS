# Release process

## Version

One version for the whole product, in three places:

| File | Field |
| --- | --- |
| `package.json` | `version` |
| `src-tauri/Cargo.toml` | `[package] version` |
| `src-tauri/tauri.conf.json` | `version` |

A Vitest test fails when they diverge. Update `CHANGELOG.md` in the same commit.

The Cargo lockfile records the crate version too; run
`cargo check --manifest-path src-tauri/Cargo.toml` after the bump so
`Cargo.lock` is refreshed.

## Checklist

1. Land all changes on `main` with green CI.
2. Bump the version in the three files and update `CHANGELOG.md`.
3. Run the manual domain checklist in [TESTING.md](TESTING.md).
4. Build locally and smoke-test the installer.
5. Tag and push:

   ```powershell
   git tag -s v2.1.0 -m "KMAruda LAPS 2.1.0"
   git push origin v2.1.0
   ```

6. The `Release` workflow builds the NSIS installer on `windows-latest`,
   computes its SHA-256, creates the GitHub release and attaches both files.
7. Verify the release assets, the checksum and the release notes.

## Signing

The workflow publishes an unsigned installer by default. To sign, add the
certificate to the runner (for example with Azure Trusted Signing or a PFX in
secrets) and configure `bundle.windows.certificateThumbprint` or the
`TAURI_SIGNING_PRIVATE_KEY` variables. Never commit a certificate or a private
key, and never echo secrets in workflow logs.

## Release notes

Use `CHANGELOG.md` as the source of truth. Group by Added, Changed, Fixed,
Removed and Security; mention breaking changes explicitly (the 1.x → 2.x
configuration reset, the dropped PowerShell fallback). Do not include internal
host names, domain names or account names.

## Rollback

Keep the previous installer and its SHA-256. The application stores no secrets,
so a downgrade only requires reinstalling the older build; the configuration
format is additive within 2.x. If a release is broken, mark it as such on
GitHub and publish a fixed patch version rather than moving the tag.
