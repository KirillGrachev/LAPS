# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.2.0] - 2026-11-26

### Added

- Native clipboard through `arboard`: the WebView no longer asks for clipboard
  permission, and auto-clear compares the value in Rust
  (`clear_clipboard_if_equals`) instead of reading arbitrary clipboard content.
- Custom window state (`window.json`): size, position and maximized state are
  restored before the window is shown; a position on a disconnected monitor is
  ignored and the window is centered.
- Data directory moved to `%APPDATA%\KMAruda LAPS`; existing 2.1 data is
  migrated once (rename, or copy into a staging directory and rename).
- Encrypted LAPS: DER scan for definite and indefinite-length CMS, diagnostics
  for every candidate and flag combination, a second `LogonUser(INTERACTIVE)`
  token for explicit sign-in, and domain inference from the Base DN or LDAP URL
  for short account names.
- Unified `text-ui-*` typography scale, a compact settings layout, native
  dependency checks before dev/build, and smoother dialogs, menus and login
  elements.

### Changed

- `chrono` is replaced by `time`; `rustix`, `url` and `tokio` are updated, and a
  RustSec audit job plus `src-tauri/.cargo/audit.toml` document the ignored
  Linux-only advisories.
- Saving an unchanged configuration no longer appends a duplicate audit record;
  the audit folder opens even when `audit.log` does not exist yet.
- `tauri-plugin-window-state` is removed in favour of the in-process store.
- DWM window transitions are disabled; the minimum window height is 680 px.
- The StartTLS switch is removed from Settings: `ldap://` always uses StartTLS,
  `ldaps://` does not need it.

### Fixed

- Local rotation deadlines around DST are resolved by round-tripping the offset;
  nonexistent and ambiguous local times are rejected instead of writing a wrong
  deadline.
- A failed data migration no longer leaves a partial data directory that blocks
  the next attempt.
- The test-mode toggle is audited again; only exact duplicate saves are skipped.
- The UI language is applied on start-up, the settings draft is not overwritten
  by its own autosave, and late search/rotation responses cannot restore a
  cleared password card.
- The audit page de-duplicates boundary rows and ignores stale responses; the
  demo journal is marked, and failed mock operations are recorded.

### Security

- Clipboard writes are session-gated; the WebView cannot read arbitrary
  clipboard content, only clear it when it still holds the copied password.
- LDAP URL schemes are case-insensitive and normalized before a connection.

## [2.1.0] - 2026-11-17

### Added

- Native LDAP transport (`ldap3`, Tokio): binds with an AD account or the
  current Windows session (SASL GSSAPI/SSPI), searches computer objects, reads
  rootDSE and modifies expiration attributes.
- Native decryption of `msLAPS-EncryptedPassword` through
  `lapsutil.dll!DecryptNormalMode`, with a token created by
  `LogonWithCredentials` for an explicit sign-in; `NCryptUnprotectSecret`
  remains an SSO fallback.
- Tolerant parsing of the MS-ADA2 2.66 envelope: several candidate layouts,
  header diagnostics in the card, no password in diagnostics.
- Backend audit journal with gzip rotation (14 archives), cross-archive
  pagination and a fixed-width human-readable format.
- Russian/English interface, light/dark/system themes, local Montserrat fonts,
  custom date-time picker and settings hub.
- Browser demo mode and a start-up WebView2 runtime preflight.
- Rust and Vitest unit suites, a no-PowerShell policy job, CodeQL, dependency
  review, OSV and Dependabot.

### Changed

- The application reads and writes LAPS entirely in-process; AD access no
  longer depends on RSAT or the `ActiveDirectory` module.
- Connection settings are validated and normalized; `ldap://` is always
  upgraded to StartTLS and the scheme is case-insensitive.
- "Reset now" writes the current FILETIME instead of deleting the attribute,
  matching `Set-LapsADPasswordExpirationTime` without `-WhenEffective`.
- The configuration file is secret-free and written atomically; connection
  settings from 1.x must be entered again.
- The audit journal is written by Rust commands rather than the UI.

### Fixed

- Encrypted LAPS no longer fails on a size field that includes the 16-byte
  header, a big-endian size, a zero timestamp or a headerless DPAPI-NG block.
- The session, the password card and the clipboard are cleared on sign-out and
  on session loss; the clipboard wipe reports failures instead of failing
  silently.
- A late search/rotation response can no longer restore a card the operator
  already cleared.
- The saved interface language is applied on start-up.
- Rapid settings edits no longer overwrite each other; the connection test
  saves the draft before testing.
- Audit pagination de-duplicates boundary rows and ignores stale responses.

### Removed

- `src-tauri/scripts/*.ps1`, generated temporary scripts, `-EncodedCommand`,
  `powershell.exe` execution and `tauri-plugin-shell`.
- The PowerShell decryption fallback (`decrypt_module.rs`).
- Persisting credentials and the DPAPI-encrypted 1.x configuration format.

### Security

- LDAPS or StartTLS is mandatory; plain LDAP is rejected before credentials
  are sent. Certificate verification is on by default.
- The decrypted API buffer is zeroized before it is freed; the sign-in password
  is held in `Zeroizing<String>`.
- LDAP filter values are escaped per RFC 4515 and validated against a character
  set; the search is restricted to computer objects.
- External links use an allow-list and the OS handler, without a shell.
- WebView2 autofill, password save, DevTools (release), context menus, zoom and
  browser accelerators are disabled; the CSP allows only local content.

## [2.0.0] - 2026-10-05

### Added

- Initial Tauri 2 + React implementation of the internal LAPS reader for the
  KMAruda support team: sign-in, search, password display, expiration reset,
  local audit and a Russian/English interface. The AD layer used PowerShell
  scripts and the system LAPS module.

[2.2.0]: https://github.com/KirillGrachev/LAPS/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/KirillGrachev/LAPS/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/KirillGrachev/LAPS/releases/tag/v2.0.0
