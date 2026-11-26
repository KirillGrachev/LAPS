# Contributing

Thanks for improving KMAruda LAPS. This guide covers setup, conventions and the
checks a pull request must pass.

## Before you start

- Read [README.md](README.md) and [ARCHITECTURE.md](ARCHITECTURE.md).
- For a security issue, follow [SECURITY.md](SECURITY.md), not a public issue.
- By participating you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).
- The project is a native Windows client: PowerShell, RSAT and the
  `ActiveDirectory` module are not acceptable dependencies.

## Development setup

Requirements:

- Windows 10/11 or Server 2019+;
- Node.js 20+ and npm;
- Rust stable with the MSVC toolchain, plus `rustfmt` and `clippy`;
- Visual Studio Build Tools with the C++ workload and the Windows 10/11 SDK;
- WebView2 Runtime.

```powershell
git clone https://github.com/KirillGrachev/LAPS.git
cd LAPS
npm ci
npm run tauri dev
```

The browser preview (`npm run dev`) uses the mock IPC layer and needs no domain.
See [BUILD.md](BUILD.md) for details and packaging.

## Project layout

| Path | Responsibility |
| --- | --- |
| `src/components` | Screens and UI primitives |
| `src/context`, `src/hooks` | Session, configuration, theme, search state |
| `src/lib/ipc.ts` | The only `invoke` adapter; mock routing lives in `mockIpc.ts` |
| `src/i18n` | `ru`/`en` dictionaries, key parity test |
| `src-tauri/src/commands` | Thin IPC commands, session gate and audit |
| `src-tauri/src/services` | LDAP, LAPS domain, configuration, audit |
| `docs` | Architecture, administration, build, testing, release |

## Checks

```powershell
npm run format:check
npm run lint
npm run typecheck
npm test
npm run build

cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --locked -- -D warnings
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

`npm run format` rewrites the frontend with Prettier. `cargo fmt` rewrites Rust.
CI runs the same checks on Linux and Windows; a pull request with a red job is
not merged.

## Conventions

- **Commits:** [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(scope): ...`, `fix(scope): ...`, `docs: ...`, `ci: ...`, `deps: ...`.
  One logical change per commit; the subject is imperative and lower-case; the
  body explains why, not what. Commits must be signed — GitHub marks commits
  merged through a pull request as Verified.
- **Frontend:** four spaces, 120 columns, single quotes, trailing commas
  (Prettier). No `any` unless justified. Do not bypass `lib/ipc.ts`.
- **Rust:** `rustfmt` defaults, `clippy -D warnings`, no `unwrap`/`expect` in
  production paths (tests may use them), errors flow through `AppError`.
- **i18n:** every user-visible string goes to both `src/i18n/ru.ts` and
  `src/i18n/en.ts`; `locales.test.ts` enforces key parity.
- **IPC:** a new command updates the Rust serde model, `src/types.ts`,
  `src/lib/mockIpc.ts`, `lib.rs`'s handler list and the audit path in the same
  change.
- **Versions:** `package.json`, `src-tauri/Cargo.toml` and
  `src-tauri/tauri.conf.json` stay identical; a test enforces it.

## Security rules

Never write a LAPS password, sign-in password, token or real domain name to a
log, the audit journal, a fixture, a screenshot, an error message or a commit.
Keep AD transport encrypted and do not add a plain-LDAP fallback. Do not spawn
PowerShell or `cmd.exe`; use typed native APIs. Prefer failing closed.

The CI `Policy` job fails if `powershell.exe`, `Get-LapsADPassword`,
`-EncodedCommand`, `tauri-plugin-shell`, a shell-allow capability or a
`Command::new("powershell")` appears in the sources, Tauri configuration or
installer.

## Pull requests

1. Branch from `main`.
2. Keep the diff focused; unrelated formatting belongs in its own commit.
3. Fill the pull request template, including the verification checklist.
4. Add a regression test when the change fixes a defect.
5. Wait for the required checks; the maintainer merges with a linear history.

UI changes: attach light and dark screenshots and check the minimum window size
(900×680).
