# Ev_Ap (Rust)

**Ev_Ap (Event Attendance)** is a desktop application for managing event
attendance via RFID scanning. It supports a **dual-backend** architecture —
local **SQLite** or cloud **Google Sheets** through a Google Apps Script Web
App.

This is the **Rust rewrite** of the original Python/CustomTkinter application
([`TriangleBear/Ev_Ap`](https://github.com/TriangleBear/Ev_Ap)), which is kept
as the read-only reference. The migration is documented in
[`docs/RUST_MIGRATION.md`](docs/RUST_MIGRATION.md) and the result of the
parity check in [`docs/PARITY_CHECKLIST.md`](docs/PARITY_CHECKLIST.md).

## Features

- **Member registration** — RFID, member id, name, student number, program
  and year.
- **Event management** — each event gets its own Google Sheet file (Sheets
  mode) or its own SQLite table (local mode).
- **Attendance tracking** — scan RFID cards to mark attendance, with the
  15 second de-duplication window.
- **Background scanning** — the scan runs on a worker thread, so the window
  stays responsive and shows a spinner.
- **Points** — award and redeem points.
- **Dual database backend** — switch between SQLite and Google Sheets in
  Settings, without a restart.
- **Data export** — attendance to CSV or Excel.
- **Light / Dark / System** appearance mode.

## Requirements

- Windows (the release artifact is a Windows executable)
- A Google account — only for the Google Sheets backend

## Installation

Download the latest `Attendance_App_<version>.zip` from the
[releases](https://github.com/TriangleBear/ev-ap-rust/releases), unpack it and
run `Attendance_App.exe`. **No Python and no runtime dependencies are
needed.**

To build it yourself:

```bash
git clone https://github.com/TriangleBear/ev-ap-rust.git
cd ev-ap-rust
cargo build --release
.\target\release\Attendance_App.exe
```

Rust 1.85 or newer is needed; the stable toolchain is enough.

## Usage

### Local mode (SQLite — the default)

Works out of the box. Everything is stored in `Ev_Ap.db` in the working
directory, and the settings in `ev_ap_config.json`.

### Google Sheets mode

1. Open the Google Sheet you want to use → **Extensions → Apps Script** →
   paste `apps-script/gsheet_api.gs`.
2. Run the **Ev_Ap → Configure Spreadsheet** menu item to set your sheet.
3. Deploy → **New deployment** → **Web App** (Execute as: Me, Access: Anyone).
4. In Ev_Ap: **Settings → Database Mode → Google Sheets**, paste the
   deployment URL, press **Save API URL** and then **Apply & Restart
   Connection**.

The main spreadsheet holds the `Members` sheet and the `Events` registry.
**Each event gets its own spreadsheet file** with an `Attendance` sheet.

### Scanning

Open an event, type the tag into the **Scan RFID Card** field and press
**Enter** — a keyboard-wedge scanner does that for you. The same tag is
ignored for 15 seconds, so a double scan cannot record the attendance twice.

## Development

```bash
cargo test              # 144 tests
cargo fmt --all         # format
cargo clippy --all-targets --all-features -- -D warnings
cargo run               # start the app
```

The crate is split into a library (`ev_ap`) and a thin binary
(`Attendance_App`), so every layer can be tested without a window:

```
src/
├── main.rs              # binary entry point
├── lib.rs               # library root
├── app.rs               # shell: sidebar nav, view routing, theme state
├── config.rs            # ev_ap_config.json
├── db/
│   ├── mod.rs           # Backend trait + the router that dispatches per db_mode
│   ├── sqlite.rs        # local SQLite backend
│   └── sheets.rs        # Google Sheets (Apps Script) backend
├── managers/            # event, member (15s cache), table (scan/export)
├── ui/                  # message box, form modal, table, attendance window
├── views/               # home, events, members, reports, settings, help, about
├── theme.rs             # light / dark / system
└── version.rs           # the release version rules
tests/                   # integration tests
docs/                    # the migration plan and the parity checklist
apps-script/             # the Google Apps Script of the Sheets backend
```

## Contributing

Issues and pull requests are welcome — see
[`CONTRIBUTE.md`](../Ev_Ap/CONTRIBUTE.md) in the Python repository for the
house rules, and the phase issues in this repository for what is being
worked on.

## License

MIT — see [LICENSE](LICENSE).
