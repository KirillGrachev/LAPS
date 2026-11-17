# Third-party notices

## Montserrat

- **Files:** `public/fonts/montserrat-400.ttf`, `montserrat-500.ttf`,
  `montserrat-600.ttf`, `montserrat-700.ttf`
- **Copyright:** Copyright 2024 The Montserrat.Git Project Authors
  (<https://github.com/JulietaUla/Montserrat>)
- **License:** SIL Open Font License, Version 1.1
- **Full license text:** [`public/fonts/OFL.txt`](../public/fonts/OFL.txt)

The fonts are shipped locally with the application, not fetched from a network,
and are distributed under the OFL 1.1. They provide the KMAruda brand
typography, as in KMAruda PhoneBook.

## Rust and npm dependencies

Versions are pinned by `src-tauri/Cargo.lock` and `package-lock.json`. License
metadata is not stored in the lockfiles; it comes from each package's manifest.
Use `cargo about` (or `cargo license`) and `npx license-checker --production`
to generate a complete inventory before redistributing a build, and include the
license texts of the packages you ship.

Transport and platform crates include `tauri` (MIT/Apache-2.0), `ldap3`
(MIT/Apache-2.0), `tokio` (MIT), `serde` (MIT/Apache-2.0), `chrono`
(MIT/Apache-2.0), `flate2` (MIT/Apache-2.0), `zeroize` (MIT/Apache-2.0),
`tracing` (MIT), `image` (MIT/Apache-2.0) and the `windows`/`windows-core`
bindings (MIT/Apache-2.0). The React side includes React, React DOM, i18next,
react-i18next, Radix UI Dialog, lucide-react, motion, sonner, date-fns,
tailwind-merge and clsx (MIT), plus Tailwind CSS (MIT).

## Microsoft WebView2

The NSIS installer embeds the Microsoft Edge WebView2 Evergreen bootstrapper.
WebView2 is distributed under Microsoft's terms; it is not part of this
repository. The application requires the WebView2 Runtime to render its
interface.

## NSIS

The installer is produced by NSIS through Tauri. NSIS is distributed under the
zlib/libpng license; its license text is included by the NSIS distribution used
at build time.
