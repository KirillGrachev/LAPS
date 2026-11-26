# Frozen archive: the 1.x PowerShell design

The files in this directory describe the **old 1.x implementation** and are kept
only as design history:

- `laps-instruction-v1-powershell.html` — the architecture write-up with
  PowerShell, RSAT and the `src-tauri/scripts/*.ps1` layer;
- `laps-architecture-v1-powershell.png` — the diagram of that variant.

Since 2.1 the application does not use PowerShell, RSAT, the `ActiveDirectory`
module, `powershell.exe` or child processes for AD access. Nothing here reflects
the current code or the current security model; do not reuse these files as
documentation for a deployment.

Current documentation:

- [`README.md`](../README.md) — overview, install, checks;
- [`../ARCHITECTURE.md`](../ARCHITECTURE.md) — the native client;
- [`../ADMIN.md`](../ADMIN.md) — AD and KDS permissions.
