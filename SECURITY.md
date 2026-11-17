# Security policy

KMAruda LAPS reads and rotates local administrator passwords stored in Active
Directory. Treat every search query, LDAP attribute, configuration file, IPC
payload and decrypted blob as sensitive: a defect can expose a machine password
or a domain credential.

## Supported versions

| Version | Supported |
| --- | --- |
| 2.1.x | Yes |
| 1.x (PowerShell) | No |

Security fixes land on `main` and in the next patch release. Include the commit
or release tag in a report if you are unsure whether a version is affected.

## Reporting a vulnerability

Do not open a public issue with a password, a credential, a domain name, an
internal computer name or a working exploit.

Use GitHub private vulnerability reporting:
<https://github.com/KirillGrachev/LAPS/security/advisories/new>

Please include:

- affected version, commit or installer;
- Windows build and architecture;
- the component: UI, Tauri command, LDAP client, decryption, audit, installer;
- impact and the access required (domain user, delegated support account,
  local user, local administrator);
- exact reproduction steps and the error/decrypt diagnostic;
- logs and screenshots with passwords, domain names and personal paths removed.

There is no bug bounty and no formal SLA; reports are handled on a best-effort
basis.

## Scope

In scope:

- LDAP filter or DN injection, or reading attributes of non-computer objects;
- TLS downgrade, certificate-check bypass without the explicit setting, or
  credentials sent over an unencrypted channel;
- disclosure of a LAPS password or sign-in credential through logs, the audit
  journal, error messages, IPC payloads, crash output, the clipboard or the UI;
- bypass of the session gate (`NOT_AUTHENTICATED`) or of the audit event for a
  domain operation;
- unsafe FFI in the decryption path, including DLL search-order issues;
- WebView2/CSP/capability misconfiguration that lets non-local content reach
  IPC commands;
- path traversal or unsafe deserialization of the configuration;
- privilege escalation in the NSIS installer or its hooks.

Out of scope:

- a compromised local administrator, domain controller or KDS root key;
- memory dumps, the pagefile or a debugger attached to the process;
- the clipboard after the password has been revealed, and screen capture;
- denial of service from a hostile LDAP server that already controls the
  directory response;
- test mode, which is visibly marked, accepts any credentials and writes no
  backend audit.

## Security model

- **Credentials are not persisted by the application.** The sign-in password
  lives in `Zeroizing<String>` in process memory, is copied into a UTF-16 buffer
  for `LogonWithCredentials` and wiped immediately after the call. The OS may
  page or dump memory; the app does not lock pages or block debuggers.
- **AD transport is encrypted.** `ldaps://`, or StartTLS forced for `ldap://`;
  plain LDAP is rejected before a bind. Certificate verification is on by
  default. Disabling it is an explicit, warned setting and is recorded in the
  audit event as `verify_cert=false`.
- **Decryption uses the native Windows LAPS API.** `lapsutil!DecryptNormalMode`
  runs under a token created for the account entered at sign-in, or under the
  current Windows account for SSO. For SSO, `NCryptUnprotectSecret` is a
  fallback; an explicit sign-in never falls back, so a password is not decrypted
  under an identity the operator did not choose. The API output buffer is
  zeroized before it is freed.
- **Audit is written by Rust commands**, not by the UI, for search, failures,
  rotation, copy, sign-in, sign-out, configuration and connection tests. It can
  be switched off in settings; turning it off is not itself audited. Write
  failures are logged as warnings and do not fail the operation. The journal is
  local plain text and is not tamper-evident.
- **No PowerShell or shell is executed.** `lapsutil.dll` is loaded by full path
  from the Windows directory; `open_external` uses an allow-list and the OS
  handler without a shell.

## Known limitations

- Windows LAPS does not document `DecryptNormalMode`/`LogonWithCredentials` as a
  public API. A Windows update, WDAC/AppLocker policy or security product may
  remove or block them; encrypted LAPS then cannot be decrypted and the card
  shows the exact diagnostic. The DLL is loaded by path, but its Authenticode
  signature is not verified by the application.
- The plaintext password is copied from the native buffer into a Rust `String`,
  crosses IPC, lives in React state and may sit in the clipboard. The
  application zeroizes the native buffer only.
- The audit journal is editable and deletable by a local administrator; audit
  can be disabled. Forward it to a SIEM if audit is a compliance requirement.
- The binary is not code-signed by default; verify the source/build and use your
  own certificate before enterprise distribution.
- Test mode is available in release builds, accepts any credentials, returns
  demo passwords and writes no backend audit. It is clearly marked in the UI.
- There is no re-authentication prompt before revealing a password, and no
  OS-level screen-capture protection.
- A live-domain penetration test and an independent review have not been
  performed for this rewrite.
