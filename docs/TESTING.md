# Testing

## Automated

| Layer | Scope | Command |
| --- | --- | --- |
| Rust domain | FILETIME conversion, attribute parsing, envelope variants, query building, escaping, expiration rules, audit parsing and rotation, error codes | `cargo test --manifest-path src-tauri/Cargo.toml --locked` |
| Frontend | IPC routing, validation, formatting, locale parity, version consistency | `npm test` |
| Static | rustfmt, clippy `-D warnings`, ESLint, `tsc --noEmit`, Prettier | see below |
| Policy | no PowerShell, no shell plugin, no shell-allow capability | CI `Policy` job |
| Supply chain | CodeQL, dependency review, OSV, RustSec audit, `npm audit` | CI workflows |

```powershell
npm run format:check
npm run lint
npm run typecheck
npm test

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

Rust tests are inline `#[cfg(test)]` modules next to the code; there is no
separate integration-test crate. One test loads `lapsutil.dll` when the
component is installed and is a no-op otherwise. CI runs the suite on Linux and
Windows; the Linux run exercises the platform-independent paths and the
`cfg(not(windows))` fallbacks.

The frontend uses Vitest with the Node environment. There is no component test
runner (no `@testing-library/react`); UI behaviour is covered by types, the
mock IPC layer and manual checks.

## Manual domain checklist

Live AD is not part of CI. Before a release, run this in a test domain with a
delegated account:

1. Plain Windows LAPS: search a machine, reveal the password, compare it with
   `Get-LapsADPassword` (run by an administrator on a management host).
2. Legacy LAPS: the same for `ms-Mcs-AdmPwd`.
3. Encrypted Windows LAPS, explicit sign-in: the password decrypts under the
   entered account; the card reports the `dpapi-ng` method.
4. Encrypted Windows LAPS, SSO: decryption works under the current session; if
   `lapsutil.dll` is blocked, the NCrypt fallback or a clear diagnostic appears.
5. Negative: an account without KDS rights gets `0x8009002C`, not a crash.
6. Rotation: "now" changes the expiration; a future date persists; a rejected
   write yields `PERMISSION_DENIED`.
7. TLS: LDAPS with a trusted certificate; StartTLS on `ldap://`; an untrusted
   certificate is rejected unless the bypass is enabled.
8. Audit: search, failure, rotation and copy produce rows; the journal survives
   a restart and rotates to gzip.
9. Sign-out: the card and clipboard are cleared; a command without a session
   returns `NOT_AUTHENTICATED`.
10. Installer: install per-user and per-machine, upgrade over a running
    instance, uninstall, and verify no PowerShell payload is present.

## What unit tests do not cover

- Real KDS decryption and `lapsutil.dll` ABI behaviour on every Windows build.
- LDAP server quirks (referrals, paging, size limits across controllers).
- WebView2 rendering, DPI, clipboard permissions and screen readers.
- A compromised host, memory dumps and the pagefile.

Record the domain, Windows build and controller in the pull request when a
change touches decryption, TLS or the expiration logic.
