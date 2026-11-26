# Architecture

KMAruda LAPS is a Tauri 2 desktop application: a React/TypeScript interface in
WebView2 and a Rust backend in the same process. The backend talks to Active
Directory over LDAP and to Windows LAPS through native Win32 APIs. Nothing is
delegated to PowerShell, `cmd.exe`, RSAT or the `ActiveDirectory` module.

This document describes the boundaries, the data flows and the decisions that
shape them. It complements the [README](README.md); operational details live
in [ADMIN](ADMIN.md), [BUILD](BUILD.md) and [TROUBLESHOOTING](TROUBLESHOOTING.md).

## Process model

```
┌───────────────────────────── WebView2 ─────────────────────────────┐
│ React 19 + TypeScript                                              │
│   screens → contexts → hooks → lib/ipc.ts (typed invoke)           │
└───────────────────────────────┬────────────────────────────────────┘
                                │ Tauri IPC (JSON, camelCase)
┌───────────────────────────────▼────────────────────────────────────┐
│ Rust: commands/*  — validation, session gate, audit, error mapping  │
│        services/ldap/*  — ldap3 transport (Tokio)                   │
│        services/laps/*  — query, parse, decrypt, expiration, time   │
│        services/config/* — secret-free model, validation, store     │
│        services/audit.rs — journal, rotation, pagination            │
└───────────────────────────────┬────────────────────────────────────┘
                                │ LDAPS / StartTLS        │ Win32
                        ┌───────▼────────┐        ┌───────▼─────────┐
                        │ Domain         │        │ lapsutil.dll    │
                        │ controller(s)  │        │ dpapi-ng / KDS  │
                        └────────────────┘        └─────────────────┘
```

One process, no sidecars and no local HTTP service. The only network protocol
is LDAP; OS integration is Win32 (LDAP TLS through `ldap3`, DPAPI-NG through
`lapsutil.dll`/CNG, the registry for the WebView2 preflight, the clipboard
through `arboard`).

## Layers and boundaries

### Commands (`src-tauri/src/commands/`)

Every IPC command is thin: it validates inputs, obtains the session, calls one
service function, writes the audit event and maps failures to `AppError`
(`{ code, message }`). Domain logic never lives here, and the frontend never
talks to LDAP directly.

`AdSession` is the gate:

- `AdSession::gated` returns `NOT_AUTHENTICATED` when no sign-in happened;
  search, rotation and copy-notification use it.
- `AdSession::ungated` is for pre-login operations (configuration, connection
  test, environment, audit reading, opening the journal folder). It borrows the
  live session when one exists and otherwise runs anonymously or with the
  current Windows identity, depending on the command.

Credentials are copied from the in-memory session per operation; the session
itself is never persisted and never crosses the IPC boundary.

### LDAP transport (`services/ldap/`)

- `ldap3` over Tokio, one connection per operation (bind, search, modify,
  unbind). No connection pool: an SSPI context is not safely reusable across
  connections without caching credentials.
- `connect.rs` enforces encryption before any credential is sent: `ldaps://`,
  or `ldap://` with StartTLS (forced automatically even if the stored flag is
  off). Certificate verification is on unless the user explicitly disabled it.
- Timeouts: 15 s to connect, 30 s per operation.
- `bind` supports SASL GSSAPI (SSPI, current Windows session) and simple bind;
  anonymous bind is the fallback when no identity is configured.
- `mod.rs` normalizes entries: attribute names are lower-cased and `text()`,
  `binary()` and `has()` look in both `attrs` and `bin_attrs`, because `ldap3`
  moves accidentally-UTF-8 binary values into the string map.
- `escape.rs` escapes RFC 4515 metacharacters. The query layer also restricts
  input to a safe character set, so escaping is the second line of defence.
- `errors.rs` classifies transport errors and AD result codes (`52e` bad
  credentials, locked account, TLS failures, timeouts) into stable codes.

### LAPS domain (`services/laps/`)

- `query.rs` classifies the user input (NetBIOS name, FQDN, IP), performs
  reverse DNS through `GetNameInfoW` for IPs, builds a filter restricted to
  `objectCategory=computer`/`objectClass=computer`, and escapes every value.
- `parse.rs` reads the three storage flavours. `msLAPS-Password` is JSON
  (`n`/`t`/`p`, with long-name aliases); `t` is a hexadecimal FILETIME.
  `msLAPS-EncryptedPassword` is the MS-ADA2 2.66 envelope; the parser returns
  several candidate DPAPI-NG blocks instead of failing on the first layout
  mismatch, and diagnostics never include a password.
- `decrypt.rs` (Windows) loads `lapsutil.dll` by full path from
  `%WindowsDir%\System32\WindowsPowerShell\v1.0\Modules\LAPS\`, resolves
  `DecryptNormalMode` and `LogonWithCredentials`, creates a network token for an
  explicit sign-in (and a full `LogonUser(INTERACTIVE)` token only if the first
  one is not accepted), decrypts, parses the UTF-16LE JSON, zeroizes the output
  buffer and frees it with `LocalFree`. Each candidate is tried with
  `flags=0` and `0x20000000` because the export is undocumented. A short account
  name gets its domain from the Base DN or LDAP URL before the Windows session.
  SSO falls back to `NCryptUnprotectSecret`; an explicit sign-in never falls
  back, so the password cannot be decrypted under an identity the operator did
  not choose.
- `expiration.rs` builds LDAP `Replace` modifications. "Now" writes the current
  FILETIME (the same effective behaviour as
  `Set-LapsADPasswordExpirationTime` without `-WhenEffective`); an explicit date
  is validated to 2000..now+10 years. Mixed domains get both expiration
  attributes.
- `time.rs` converts FILETIME (100 ns since 1601) to local RFC 3339 and back.
  Local times are resolved by round-tripping the offset; nonexistent and
  ambiguous DST moments are rejected instead of writing a wrong deadline.

### Configuration (`services/config/`)

`AppConfig` contains theme, language, LDAP URL, base DN, TLS flags, audit
switch, clipboard timeout and the demo-mode flag. There are no secrets: the
password is entered at every launch and lives only in the session.

`ConfigStore::save` validates and normalizes input, then writes
`config.json` atomically (temp file + rename). A corrupt file is moved to
`config.json.corrupt` and defaults are used. `ldap://` URLs are normalized to a
lower-case scheme with StartTLS forced. The configuration, the audit journal and
`window.json` live in `%APPDATA%\KMAruda LAPS`; data from the older
`com.kmaruda.laps` directory is migrated once (rename, or copy into a staging
directory and rename when the directories are on different volumes). A save
with an unchanged configuration does not append a duplicate audit record.

Window size, position and maximized state are stored by `window_state.rs` and
restored before the window is shown. A position that no longer intersects any
available monitor is ignored and the window is centered.

### Audit (`services/audit.rs`)

The Rust commands write the journal, not the UI: a search, a failed search, a
rotation, a rotation failure, a copy marker, sign-in, sign-out, a saved
configuration and a connection test each append a fixed-width line to
`audit.log` and `sync_data()` it. Passwords never enter the record; details
carry the operation mode, an error code or a bounded diagnostic string.

The file is rotated like `latest.log`: the previous run is gzipped at startup,
an oversized live file (10 MiB) is rotated mid-run, and at most 14 archives are
kept. Reading merges the live file and all archives, newest first, with
offset pagination for the UI. This is a local operational journal, not a
tamper-evident compliance log: a local administrator can edit it, audit can be
switched off, and write failures are logged but do not fail the operation.

### Frontend (`src/`)

React 19 with contexts for session, configuration, theme, search and toasts.
`lib/ipc.ts` is the only place that calls `invoke`; it also routes commands to
`lib/mockIpc.ts` in the browser demo and in test mode. `lib/mockIpc.ts` mirrors
the Rust contract, including audit events, and is kept in sync manually —
changes to `types.ts` must update both sides.

Screens are `LoginScreen`, `SearchTab`, `AuditTab` and `SettingsTab`, sharing
the UI primitives in `components/ui`. Localization is `ru`/`en` with key parity
enforced by a test. Backend messages and decrypt diagnostics stay Russian by
design; the English UI translates stable error codes and shows raw technical
text as diagnostics.

## Data flows

### Search

1. The operator submits a name, FQDN or IP.
2. `search_computer` gates the session, parses the query (reverse DNS for IPs),
   opens LDAP, resolves the base DN (configured or rootDSE), and searches with
   the computer-only filter.
3. Exactly one entry is normalized and turned into `LapsInfo`. For encrypted
   storage, parsing and decryption run on a blocking thread; a failure produces
   a card without a password plus `decryptError`, not an error result.
4. The command writes `search`/`search_failed` to the audit journal and returns
   the card; the UI masks the password until the operator reveals it.

### Rotation

1. The operator chooses "now" or a date/time.
2. The command validates the DN (must start with `CN=`, no control characters),
   reads the object, builds `Replace` modifications for the expiration
   attributes present, modifies the object, re-reads it and returns the fresh
   card.
3. The audit event records the mode and result; a rejected write becomes
   `PERMISSION_DENIED` with an administrator hint.

### Sign-in

1. `login` loads the configuration, creates credentials, opens LDAP and binds
   (GSSAPI for SSO, simple bind otherwise).
2. On success the session (actor, account, `Zeroizing` password) is stored in
   memory and the `login` event is written.
3. `logout` drops the session, zeroizes the password and writes `logout`.
   There is no session TTL: the session ends at logout or process exit.

## Failure model

`AppError` is serialized as `{ code, message }`. The code is stable and used as
the i18n key; the message is a user-facing Russian explanation that may contain
technical details. Transport failures, TLS failures, authentication failures,
missing objects, missing LAPS data, insufficient rights and missing sign-in are
distinct codes. A computer with encrypted LAPS that cannot be decrypted is
**not** an error: the card is returned with `password: null`, `decrypted:
false` and the diagnostic, so expiration management remains available.

## Security boundaries

- Untrusted input: search queries, Base DN, LDAP URL, DN for rotation, IPC
  payloads, LDAP attribute values, the encrypted blob, configuration files.
- Trusted: the Rust command layer, the session store, `lapsutil.dll` loaded
  from the Windows directory (full path, no search path).
- Out of scope: a compromised local administrator, a compromised domain
  controller, memory dumps and pagefile contents, and the clipboard/UI after
  the password has been revealed.

The design deliberately avoids a generic shell, dynamic command execution and
remote content in the WebView (strict CSP, no DevTools in release, browser
accelerators and context menus disabled).

## Non-goals

- DC Locator and automatic failover: the controller is configured explicitly.
- Recovery mode, local KDS and password history decryption.
- Entra ID / Intune LAPS.
- Multi-tenant or remote (non-domain) operation.
- A tamper-evident audit trail or SIEM forwarding.
