# Privacy

KMAruda LAPS is a local desktop application. It has no telemetry, no analytics
and no remote service other than the Active Directory controller you configure.

## What is stored locally

| Data | Location | Contains secrets |
| --- | --- | --- |
| Configuration | `%APPDATA%\com.kmaruda.laps\config.json` | No |
| Audit journal | `%APPDATA%\com.kmaruda.laps\audit.log` | No password fields |
| Archived journals | `%APPDATA%\com.kmaruda.laps\audit-*.log.gz` | No password fields |

The configuration holds the LDAP URL, Base DN, theme, language, TLS and audit
preferences. The audit journal holds the timestamp, action, result code,
computer name, LAPS flavour, actor (`DOMAIN\user@WORKSTATION` or the sign-in
account) and a bounded diagnostic string. LAPS passwords, sign-in passwords and
tokens are never written there.

## What leaves the machine

Only LDAP traffic to the configured controller. The application does not call
home, does not download updates and does not send crash reports. External links
open in the default browser through a scheme allow-list (`http`, `https`,
`mailto`).

## Memory and the clipboard

The sign-in password is kept in `Zeroizing<String>` and wiped at logout and
exit. The decrypted LAPS password exists in Rust memory, in the IPC payload, in
React state and in the clipboard while the operator uses it. The OS may page or
dump process memory; the app does not attempt to lock pages or block debuggers.
The clipboard is cleared after the configured timeout when the WebView can still
access it; the failure mode is reported in the UI.

## Test mode

Test mode replaces domain commands with demo data, accepts any credentials,
keeps its mock journal in browser `localStorage` and writes nothing to the
backend audit. It is visibly marked in the UI and should not be enabled on a
production workstation.

## Deletion

Remove `%APPDATA%\com.kmaruda.laps` to delete the configuration and the whole
audit history. Uninstalling the application removes the program files; the
data directory is intentionally left in place so an upgrade or reinstall keeps
the settings.
