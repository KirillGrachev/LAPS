# Compatibility

## Application

| Platform | Supported |
| --- | --- |
| Windows 11 | Yes |
| Windows Server 2022 / 2025 | Yes |
| Windows Server 2019 | Yes, with the WebView2 Runtime |
| Windows 10 21H2/22H2 | Yes, while the OS is supported (ESU after October 2025) |
| Linux / macOS | Builds for development only; no AD, no Windows LAPS decryption |

The app needs a domain-joined Windows machine, the WebView2 Runtime, LDAPS
(636) or StartTLS to a domain controller, and delegated LAPS rights. It does not
need RSAT, the `ActiveDirectory` module or local administrator rights.

## LAPS storage

| Storage | Attribute | Read | Reset expiration |
| --- | --- | --- | --- |
| Legacy LAPS | `ms-Mcs-AdmPwd` | Plain text, attribute ACL | `ms-Mcs-AdmPwdExpirationTime` |
| Windows LAPS, plain | `msLAPS-Password` (JSON) | Plain text, attribute ACL | `msLAPS-PasswordExpirationTime` |
| Windows LAPS, encrypted | `msLAPS-EncryptedPassword` | DPAPI-NG + KDS | `msLAPS-PasswordExpirationTime` |

In a mixed domain the app prefers plain Windows LAPS, then legacy, then
encrypted. Resetting the expiration writes every expiration attribute present
on the object, so agents of both generations see a consistent value.

## Windows LAPS platform support

The Windows LAPS component (`lapsutil.dll`) ships with:

- Windows 11 23H2 and newer;
- Windows 10 21H2/22H2 with the April 11, 2023 update (ESU after October 2025);
- Windows Server 2019 and 2022 with the April 11, 2023 update;
- Windows Server 2025.

For SSO, the app can fall back to `NCryptUnprotectSecret` when `lapsutil.dll`
is absent or rejects the block. For an explicit sign-in the DLL is required:
without it the card shows a diagnostic and no password.

## Not supported

- Recovery-mode decryption (`DecryptRecoveryMode`), local KDS and password
  history.
- Entra ID / Intune LAPS and Azure AD DS.
- DC Locator, automatic site-aware failover and multi-domain forests: configure
  one reachable controller and, if needed, an explicit Base DN.
- Reading LAPS attributes of users or groups; the search is restricted to
  `objectCategory=computer`.
- Hardware-backed key storage or memory protection against a local
  administrator (pagefile, dumps, debuggers).
- A tamper-evident audit log or SIEM forwarding.

## Limits and quotas

- One LDAP connection per operation, 15 s connect timeout, 30 s operation
  timeout.
- Audit: 10 MiB live file, 14 gzip archives; older archives are deleted.
- Clipboard auto-clear: off, or 10–300 seconds (default 30).
- Window: minimum 900×680 logical pixels; size and position are restored when
  the monitor is still available.
- Search input: 255 characters; rotation date: 2000 .. now + 10 years.
