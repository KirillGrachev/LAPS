# Troubleshooting

The UI shows a stable error code and a message. The same code is written to
`audit.log` for domain operations. Start with the code, then use the checklist.

## Error codes

| Code | Meaning | What to do |
| --- | --- | --- |
| `CONFIG_ERROR` | No LDAP URL is configured | Settings → Active Directory → enter `ldaps://dc.company.ru:636` |
| `LDAP_UNREACHABLE` | TCP connection failed | Check the address, port, VPN and firewall |
| `LDAP_TIMEOUT` | No response: 15 s to connect, 30 s per operation | Check the network path to the controller |
| `LDAP_AUTH` | Bind rejected | Check the account, password, lockout and expiry |
| `LDAP_TLS` | TLS handshake or certificate failure | Check the certificate chain, the internal CA and port 636 |
| `LDAP_INSECURE` | Defensive guard: the channel would be cleartext | Use `ldaps://`; `ldap://` is upgraded to StartTLS automatically |
| `LDAP_PROTOCOL` | Unexpected LDAP response | Check the Base DN and the controller's health |
| `COMPUTER_NOT_FOUND` | The filter matched nothing | Check the name/FQDN, the Base DN and the object type |
| `COMPUTER_AMBIGUOUS` | Several computers matched | Search by FQDN |
| `NO_LAPS_DATA` | No LAPS attribute is visible | Check the LAPS policy and the delegated read right |
| `PERMISSION_DENIED` | The write was rejected | Delegate "Reset msLAPS password expiration" |
| `NOT_AUTHENTICATED` | No sign-in in this process | Sign in again (after logout or restart) |
| `VALIDATION` | Input rejected | Correct the URL, Base DN or date |
| `INTERNAL` | Unexpected failure | Check the application log and report the code |

## Encrypted LAPS diagnostics

The card shows the exact native error when the password cannot be decrypted:

| Diagnostic | Likely cause |
| --- | --- |
| `модуль Windows LAPS не найден` | No `lapsutil.dll` on this Windows build |
| `DecryptNormalMode: 0x8009002C` | No KDS decrypt right for this account |
| `DecryptNormalMode: 0x80070005` | Access denied to the KDS key or the attribute |
| `DecryptNormalMode: 0x8007052E` | Wrong password in `LogonWithCredentials` |
| `расшифрованный блок не является JSON LAPS` | Unexpected payload; report the header diagnostics |
| `NCryptUnprotectSecret: HRESULT 0x80090016` | The key is not available to this process; sign in explicitly |

The header diagnostic (`len`, `timestamp`, `size8`, `size12`, `sizeBE`,
`payload`, candidate count, first 24 bytes in hex) never contains a password;
include it in a bug report.

## Checklist

1. `nltest /dsgetdc:company.ru` from the workstation resolves a controller.
2. `Test-NetConnection dc1.company.ru -Port 636` succeeds (or StartTLS on 389).
3. The certificate chain is trusted, or "Allow invalid certificate" is on.
4. The account can bind: try the same credentials with `ldp.exe`.
5. The account has the attribute read right; `ldp.exe` shows the LAPS
   attributes on the computer object.
6. For encrypted storage, the account has the KDS rights and a route to the
   controller that holds the KDS root key.
7. The LAPS policy is applied to the computer and the agent has written a
   password at least once.

## Logs

- Application log: run the app from a console (`KMAruda LAPS.exe`) or check the
  output captured by your deployment tool. The default filter is `info`; set
  `RUST_LOG=app_lib=debug` for details. Passwords are never logged.
- Audit journal: Settings → Security → "Log folder", or
  `%APPDATA%\KMAruda LAPS\audit.log`.

## Build failures

See [BUILD.md](BUILD.md). The most common causes are a missing MSVC C++
workload, a missing Windows SDK, or a stale `src-tauri/target` after a toolchain
upgrade (`cargo clean`).
