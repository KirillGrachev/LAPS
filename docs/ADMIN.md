# Active Directory administration

KMAruda LAPS needs delegated rights, never domain administrator. Give the
support group only what each operation requires and keep the rights on the
computer objects, not on the domain root.

## Rights by operation

| Operation | Attribute / right | Scope |
| --- | --- | --- |
| Read legacy password | Read `ms-Mcs-AdmPwd` | Managed computers (or the OU tree) |
| Read plain Windows LAPS | Read `msLAPS-Password` | Managed computers |
| Read encrypted Windows LAPS | Read `msLAPS-EncryptedPassword` + **Read password** KDS right | Managed computers + KDS root key |
| Decrypt encrypted LAPS | **Decrypt** KDS right, route to a DC | KDS root key |
| Reset expiration | **Reset msLAPS password expiration** (or Write on `ms-Mcs-AdmPwdExpirationTime`) | Managed computers |
| Connection test | Read rootDSE (anonymous or authenticated) | Domain controller |

`msLAPS-EncryptedPassword` is unreadable without both the attribute ACL and the
KDS right: the attribute holds a DPAPI-NG envelope whose key is derived from the
KDS root key. Granting only the attribute read right produces
`DecryptNormalMode: 0x8009002C`.

## Delegation

### Legacy LAPS

Use the LAPS UI or ADSI Edit to delegate **Read `ms-Mcs-AdmPwd`** and **Read
`ms-Mcs-AdmPwdExpirationTime`** on the computer objects, or grant the group
`Read All Properties` on the managed OU. For resets, delegate **Write
`ms-Mcs-AdmPwdExpirationTime`**.

### Windows LAPS

On the OU that contains the managed computers, use **Delegate Control** and
grant:

- **Read `msLAPS-Password`** and **Read `msLAPS-PasswordExpirationTime`** for
  plain storage;
- **Read `msLAPS-EncryptedPassword`** for encrypted storage;
- **Reset msLAPS password expiration** for rotation.

The KDS rights are separate: on the `msKds-ProvRootKey` object (or via the
Windows LAPS delegation wizard) grant the support group **Read password** and
**Decrypt**. The group also needs a network route to a domain controller that
holds the KDS root key; the app does not implement DC Locator, so configure a
reachable controller explicitly.

### Least-privilege layout

```
GG-LAPS-Support            global group, support engineers
  ├─ read LAPS attributes  on OU=Workstations,OU=Servers
  ├─ reset expiration      on the same OUs
  └─ KDS read/decrypt      on the domain KDS root key
```

Do not add the group to Domain Admins, and do not grant rights on the domain
root. Keep the sign-in account separate from the engineer's day-to-day account.

## Transport and certificates

- Prefer `ldaps://dc1.company.ru:636`. `ldap://` is upgraded to StartTLS
  automatically and plain LDAP is rejected.
- Certificate verification is on by default. For an internal CA whose root is
  not in the machine store, either install the root certificate or enable
  **Allow invalid certificate** in Settings → Active Directory. That choice is
  written to the audit event as `verify_cert=false`.
- Open LDAP 389 is only used for the StartTLS handshake, never for credentials.

## Configuration and data locations

| Item | Path |
| --- | --- |
| Configuration | `%APPDATA%\com.kmaruda.laps\config.json` |
| Audit journal | `%APPDATA%\com.kmaruda.laps\audit.log` |
| Archived journals | `%APPDATA%\com.kmaruda.laps\audit-*.log.gz` (14 kept) |

The configuration contains no secrets. The audit journal contains computer
names, account identifiers and operation results; protect the profile folder
accordingly.

## Upgrading from 1.x

1.x stored a DPAPI-encrypted `config.json` and its journal in
`%APPDATA%\KMArudaLAPS`. 2.x uses the Tauri data directory and a secret-free
format, so the connection settings must be entered once. The 1.x journal is not
migrated and is not deleted.

## Test mode

Test mode replaces domain commands with the in-browser demo data set. It is
visibly marked in the sidebar and in the audit tab, accepts any credentials,
writes no backend audit and must not be used on a production workstation to
"try" the app against a real domain.
