# Parity Checklist — Ev_Ap (Rust) vs. Ev_Ap (Python)

The checklist of Phase 7 of [`RUST_MIGRATION.md`](RUST_MIGRATION.md). Every
item was checked against the Python app in
[`TriangleBear/Ev_Ap`](https://github.com/TriangleBear/Ev_Ap).

**Result: all items pass, with one known upstream difference — the points on
the Google Sheets backend (see [D1](#d1-the-points-do-not-work-on-the-sheets-backend)).**

## How it was checked

| Method | What it covers |
|---|---|
| `cargo test` (137 tests) | the behaviour of every layer |
| `tests/parity.rs` | the **same** scenario run against the SQLite backend and against the Google Sheets backend; both must produce the same result |
| `cargo run --example inspect_db -- <Ev_Ap.db>` | the database the **Python app** wrote is opened by the Rust build and read back identically |
| Driving the running app | the UI flows: register, create event, scan, delete, export, settings |

The Sheets backend is tested against a **stateful** fake of the Apps Script
(`tests/support/apps_script.rs`) that follows `apps-script/gsheet_api.gs`, so
the two backends really do the same work.

## The database

| # | Check | Result |
|---|---|---|
| D1 | The `Ev_Ap.db` written by the **Python app** opens in the Rust build | ✅ — the member `rfid=123456, memberid=TEST-1, name=Kurt, student_num=2013516, date_registered=2026-07-16` is read back unchanged |
| D2 | The database opens **without a migration** | ✅ — no data is rewritten, the file is only read |
| D3 | The `points` column is added when an old database is opened | ✅ — additive only, the existing rows survive |
| D4 | A **fresh** database gets the same schema | ✅ — `rfid, memberid, name, student_num, program, year, date_registered, points` |
| D5 | The `Members` table and the per-event tables have the same columns as the Python app | ✅ |
| D6 | The event tables are named `Event_Name` (spaces → underscores) | ✅ — the same normalisation the Python app does |
| D7 | An event name with a quote or a semicolon is rejected | ✅ — the Rust port validates the identifier instead of interpolating it into the SQL (the Python app does interpolate it) |

## The features

| # | Check | SQLite | Google Sheets | Result |
|---|---|---|---|---|
| F1 | **Register** a member | ✅ | ✅ | The same result on both backends |
| F2 | Registering the same RFID twice | ✅ | ✅ | *Member already exists!* |
| F3 | **Create** an event | ✅ | ✅ | `Test Event` → `Test_Event` |
| F4 | **Delete** an event | ✅ | ✅ | The table is dropped, the registry entry is removed |
| F5 | **Scan** a registered tag | ✅ | ✅ | *RFID recorded* |
| F6 | Scan an unknown tag | ✅ | ✅ | *RFID number not found* |
| F7 | Scan the same tag twice | ✅ | ✅ | *… was scanned within the last 15 seconds. Ignoring scan...* |
| F8 | Scan the same tag after 15 s | ✅ | ✅ | *Member has already attended this event.* |
| F9 | **Points** — add | ✅ | ❌ | [D1](#d1-the-points-do-not-work-on-the-sheets-backend) |
| F10 | Points — read | ✅ | ❌ | [D1](#d1-the-points-do-not-work-on-the-sheets-backend) |
| F11 | Points — redeem | ✅ | ❌ | [D1](#d1-the-points-do-not-work-on-the-sheets-backend) |
| F12 | **Export** to CSV | ✅ | ✅ | The same columns: `memberid,name,student_num,attendance_time` |
| F13 | **Export** to Excel | ✅ | ✅ | A real workbook |
| F14 | **Mode switch** SQLite ↔ Google Sheets | ✅ | ✅ | The switch takes effect without a restart |
| F15 | The **search** filters the rows | ✅ | ✅ | Case insensitive, on the cell values |
| F16 | The **error messages** | ✅ | ✅ | Byte for byte the same as the Python app |

## The user interface

| # | Check | Result |
|---|---|---|
| U1 | The window opens at 1100×650 with the title *Events Attendance System* | ✅ |
| U2 | The sidebar has the seven navigation buttons in the Python order | ✅ |
| U3 | A view is built the first time it is opened | ✅ |
| U4 | The database is initialised on a worker thread with a progress bar | ✅ |
| U5 | The message box is modal | ✅ |
| U6 | The appearance mode switches between Light / Dark / System | ✅ |
| U7 | The attendance window shows the table, the search, the refresh and the export | ✅ |
| U8 | The RFID entry reacts to **Enter** and regains the focus after every scan | ✅ |
| U9 | The dashboard buttons react to clicks | ✅ |

## Known differences

<a name="d1-the-points-do-not-work-on-the-sheets-backend"></a>
### D1 — The points do not work on the Sheets backend

The Apps Script has **no `addPoints` / `redeemPoints` handler**, so it answers
*"Unknown action: addPoints"* and the balance stays at 0.

**This is not a port defect — the Python app has exactly the same gap.** The
Rust client already sends the two actions and surfaces the script's message
unchanged, so the Sheets backend starts working the moment the handler is
added to `apps-script/gsheet_api.gs` — with **no** change to the client.

The `Members` sheet of the script also has no `points` column, so the balance
cannot be stored there yet either.

### D2 — Visual parity is not the goal

egui does not look like CustomTkinter. The bar is **behavioural** parity: the
same data, the same messages, the same flows.

### D3 — The event table is a window inside the window

The Python app opens a separate `1000x700` OS window for the attendance
table. The Rust port opens an `egui::Window` inside the main window, so the
RFID entry is always reachable without switching windows.

## Known gaps in the Python app that were **not** copied

| # | Gap | Why |
|---|---|---|
| G1 | No automatic point award on a scan | The Python app never awards points on its own — the redemption UI is commented out in `member_manager.py` and `points_per_event` is commented out in `event_manager.py`. The port does not invent a rule. |
| G2 | The *Redeem Points* card is disabled | Same as the Python app — it shows the "This feature is temporarily unavailable" tooltip. |
| G3 | The points redemption dialog is not built | The Python one is commented out. The `redeem_points` **operation** exists and is tested. |
