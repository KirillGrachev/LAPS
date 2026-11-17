# Build and packaging

## Prerequisites

| Tool | Version | Notes |
| --- | --- | --- |
| Windows | 10/11 or Server 2019+ | The app targets WebView2; packaging is Windows-only |
| Node.js | 20 or newer (22 in CI) | npm ships with it |
| Rust | 1.90 or newer, MSVC toolchain | `rustup default stable-x86_64-pc-windows-msvc` |
| Visual Studio Build Tools | 2022, "Desktop development with C++" | Includes the Windows 10/11 SDK and MSVC v143 |
| WebView2 Runtime | Evergreen | Present on current Windows; the installer also bundles a bootstrapper |
| NSIS | fetched by Tauri | No manual install; Tauri downloads the tooling on the first bundle |

The Rust toolchain needs `rustfmt` and `clippy` for the checks:

```powershell
rustup component add rustfmt clippy
```

## Development

```powershell
npm ci
npm run tauri dev
```

`npm run tauri dev` starts Vite on port 1420 and the Tauri shell. The browser
preview (`npm run dev`) uses the mock IPC layer and needs no domain, no LDAP and
no WebView2 host — useful for UI work.

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

The same commands run in CI on Linux and Windows. The Linux job installs
`libwebkit2gtk-4.1-dev`, GTK 3, appindicator, librsvg, OpenSSL and keyutils
headers; Tauri's `generate_context!` embeds `dist`, so the frontend is built
before Cargo runs.

## Release installer

```powershell
npm run tauri build
```

Output:

```
src-tauri/target/release/bundle/nsis/KMAruda LAPS_2.1.0_x64-setup.exe
```

The NSIS package uses the branded header/sidebar images from
`src-tauri/installer/`, offers per-user and per-machine installs, includes the
WebView2 bootstrapper and runs `installer/hooks.nsh` to close a running instance
before an upgrade. There is no PowerShell in the payload.

## Code signing

Unsigned builds trigger SmartScreen. To sign, set the standard Tauri variables
before `npm run tauri build`:

```powershell
$env:TAURI_SIGNING_PRIVATE_KEY = "..."
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = "..."
```

Alternatively configure `bundle.windows.certificateThumbprint` and
`bundle.windows.digestAlgorithm` in `src-tauri/tauri.conf.json` for a
certificate installed in the user store. Keep the private key out of the
repository and CI logs. The release workflow publishes the unsigned installer
and its SHA-256; add signing there once a certificate is available.

## CI artifacts

The `Rust backend (Windows)` job builds the NSIS installer on every push and
pull request and uploads it as the `kmaruda-laps-nsis` artifact. The tag-driven
`Release` workflow attaches the installer and a `.sha256` file to the GitHub
release.

## Repository hygiene

`dist/`, `node_modules/`, `src-tauri/target/` and `src-tauri/gen/` are ignored.
`.gitattributes` normalizes text to LF and marks binary assets; `.editorconfig`
and Prettier define four-space, 120-column formatting for the frontend.
