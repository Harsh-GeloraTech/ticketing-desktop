# Phase 1 — Thermal Printer Prototype

A self-contained desktop prototype that validates the **ESC/POS thermal-printing
workflow** before the full ticketing platform is built. No backend, no sync, no
auth — all printer logic runs inside the Tauri app.

## What it proves

- ✅ **Printer detection** — scans OS-installed printers on startup
- ✅ **Connection status** — live 🟢 Connected / 🔴 Disconnected, auto-updates on
  unplug / power-off / reconnect (polled every 3s)
- ✅ **Thermal printing** — raw ESC/POS commands sent through the OS spooler
- ✅ **QR printing** — native printer QR (`GS ( k`, model 2, EC level H)
- ✅ **Reprint** — re-sends the last ticket with the **same** ticket number
- ✅ **Disconnect handling** — pre-print gate + friendly error messages
- ✅ **Local persistence** — `printers` + `tickets` tables in SQLite

## Architecture

All hardware access lives in the Tauri Rust process; the React UI calls it via
`invoke()`. The OS print spooler (CUPS on Linux, Windows spooler) is the
transport, so the printer's normal driver handles the USB/network link.

```
src-tauri/
  migrations/0001_printer_prototype.sql   printers + tickets tables
  src/
    db.rs            local SQLite pool (app-data dir) + migrations
    lib.rs           DB setup + command registration
    printer/
      model.rs       PrinterInfo / PrinterStatus / Ticket / PrintResult
      error.rs       PrinterError -> friendly messages
      discovery.rs   enumerate printers (lpstat / Get-Printer)
      status.rs      live online/offline per printer
      escpos.rs      raw ESC/POS buffer builder (+ unit tests)
      spooler.rs     send raw bytes (lp -o raw / winspool RAW)
      commands.rs    #[tauri::command] entry points
src/
  tauri.ts             typed invoke() wrappers
  PrinterSettings.tsx  detect / select / save default / test
  PrinterStatusBadge.tsx  live 🟢/🔴 indicator
  TestTicket.tsx       test print + reprint + last-ticket preview
  App.tsx              adds the "Printer" tab
```

### Tauri commands (the contract)

| Command | Purpose |
|---|---|
| `list_printers` | OS printers + saved default flag + live status |
| `refresh_printers` | re-scan and upsert into SQLite |
| `get_printer_status(name)` | live 🟢/🔴 for the badge |
| `set_default_printer(name)` / `get_default_printer` | persist / read default |
| `test_print(name)` | small "PRINTER OK" page |
| `print_ticket(name, company, footer)` | new ticket # + ESC/POS + store row |
| `reprint_last(name)` | re-send last ticket, **same** number |
| `get_last_ticket` | "View Last Printed Ticket" |

### Database (local, app-data dir)

`ticketing-prototype.db` lives in the OS app-data directory (not the CWD), so it
survives reinstalls and isn't tied to where the binary starts.

```sql
printers(id, name UNIQUE, type DEFAULT 'thermal', is_default 0/1)
tickets (id, ticket_number UNIQUE, qr_data, company, footer, printed_at)
```

## Prerequisites

- **Rust** ≥ 1.85 (built/verified on 1.96)
- **Node** ≥ 18 (verified on 20) + npm
- **Linux build deps** (Debian/Ubuntu):
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    libayatana-appindicator3-dev build-essential curl wget file
  ```
- **Linux runtime**: CUPS (`lp`, `lpstat`) — preinstalled on most desktops.
- **Windows**: PowerShell (built in). The printer must be installed in Windows
  (Settings → Printers). For pure-USB ESC/POS units, install the vendor driver
  so it appears in the spooler.

## Develop

```bash
cd ticketing-desktop
npm install
npm run tauri dev
```

Open the **Printer** tab → Refresh list → pick a printer → Save default → Test
connection. Then Test print / Reprint last.

## Run the ESC/POS tests (no hardware needed)

```bash
cd ticketing-desktop/src-tauri
cargo test --lib       # validates the raw byte buffers (init, QR, cut, sanitize)
```

## Build installers

```bash
cd ticketing-desktop
npm install
npm run tauri build
```

Output lands in `src-tauri/target/release/bundle/`:

- **Linux**: `appimage/*.AppImage` and `deb/*.deb`
- **Windows** (run on Windows, or cross-build): `msi/*.msi` and `nsis/*-setup.exe`

> Cross-compiling Windows from Linux is possible but fiddly; the reliable path is
> to run `npm run tauri build` **on each target OS**. `bundle.targets` is already
> `"all"`.

## Testing without a physical thermal printer (Linux)

Create a CUPS queue that writes to a file, so you can validate the whole flow:

```bash
# A raw queue pointing at a file sink
sudo lpadmin -p TestThermal -E -v file:/tmp/thermal.out -m raw
sudo cupsenable TestThermal
sudo cupsaccept TestThermal
```

`TestThermal` will now appear in the app. A Test print writes the raw ESC/POS
bytes to `/tmp/thermal.out` — inspect them with `xxd /tmp/thermal.out`. Unplugging
is simulated with `sudo cupsdisable TestThermal` (badge flips to 🔴).

## Notes / future extension

- The Axum backend (`ticketing-backend`) and the existing Console/Scan tabs are
  untouched — they remain the basis for the future cloud-sync system. Printing is
  now a real ESC/POS path, separate from the old browser `window.print()`.
- `print_ticket` stores the row **before** sending bytes, so a hardware failure
  still leaves a reprintable record with a stable number.
- ESC/POS QR uses error-correction level **H** for scan robustness on cheap paper.
