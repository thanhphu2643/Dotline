# Dotline

macOS menu bar app that lets any web app print raw labels (ZPL, EZPL, TSPL…)
to local or network label printers, with no QZ Tray, Java or certificates.
The page sends jobs to `http://127.0.0.1:17820`. Dotline delivers them to a CUPS
queue (`lp -o raw`) or straight to a network printer (`tcp://host:9100`), keeps
a history, and previews every label from the exact bytes sent.

- `core/`: Rust staticlib with the HTTP server, job ledger, CUPS/TCP delivery and the EZPL/ZPL bitmap decoder. C ABI in `core/include/dotline.h`.
- `app/`: SwiftUI app (dashboard window + `MenuBarExtra`) calling Rust via module `CDotline`. UI in English, with Vietnamese following the macOS language (`Resources/*.lproj`).
- `app/Icon/`: `make-icon.swift` draws the icon in code; `make icon` re-renders `AppIcon.icns`.

- `windows/`: Windows tray app (Rust, `tray-icon`) on the same core. Prints through the Windows spooler (RAW datatype); data in `%APPDATA%\Dotline`.

```sh
make test   # cargo test
make run    # build Rust + Swift, bundle build/Dotline.app, open it
make dmg    # dist/Dotline-<version>-macos-arm64.dmg
make exe    # dist/Dotline-<version>-windows-x64.exe (needs: brew install mingw-w64; rustup target add x86_64-pc-windows-gnu)
make dist   # both
```

Builds are not signed for distribution yet. The macOS app is ad-hoc signed: first launch needs
right-click → Open, or Developer ID signing plus notarization. The Windows exe is unsigned, so
SmartScreen shows a warning.

Data lives in `~/Library/Application Support/Dotline/`: `config.json` (port + token, mode 0600),
`jobs.json` (last 500 jobs) and `payloads/` (bytes for previews).

## HTTP API

Every route except `/health` requires `Authorization: Bearer <token>` (copy it from the app).
CORS and Chrome Private Network Access are enabled, so HTTPS pages can call localhost.

| Route | Result |
|---|---|
| `GET /health` | `{app: "dotline", version}`: detect whether Dotline is installed |
| `GET /printers` | `{printers: [{name, state, isDefault}]}` |
| `POST /jobs` `{id, printer, payloadBase64, payloadHash?, title?}` | `Job` (200 sent / 502 failed) |
| `GET /jobs/:id` | `Job` `{id, printer, payloadHash, bytes, state, detail, requestId, createdAt}` |

Errors are `{error: code}`: `unauthorized` (401), `printerNotFound` (404), `jobNotFound` (404),
`invalidJob` / `invalidPayload` / `payloadHashMismatch` (400), `jobIdConflict` (409),
`payloadTooLarge` (413), `printersUnavailable` (503).

- **Idempotent by `id`:** resending a job ID with the same bytes returns the first result and never prints twice. The same ID with different bytes returns 409. Only `failed` jobs are retried.
- **Exact printer names:** no fuzzy matching, so a label can never land on a similarly named queue.
- **In order:** requests are handled one at a time, so labels print in the order they were sent.

```js
await fetch("http://127.0.0.1:17820/jobs", {
  method: "POST",
  headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
  body: JSON.stringify({ id: "order-1042-label-1", printer: "Zebra_ZD421", payloadBase64: btoa(zpl) }),
});
```
