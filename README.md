# Ev_Ap (Rust)

**Ev_Ap** is a desktop application for event attendance automation. Members are
registered against an RFID tag; at an event, scanning the tag records
attendance and awards points.

This repository is the **Rust rewrite** of the original Python/CustomTkinter
application that lives in
[`TriangleBear/Ev_Ap`](https://github.com/TriangleBear/Ev_Ap). The Python repo
is kept as the read-only reference for parity work.

## Status

| Phase | Scope | Issue |
|---|---|---|
| 0 | Repo & scaffold | [#1](https://github.com/TriangleBear/ev-ap-rust/issues/1) |
| 1 | Data layer (SQLite) | [#2](https://github.com/TriangleBear/ev-ap-rust/issues/2) |
| 2 | Sheets backend | [#3](https://github.com/TriangleBear/ev-ap-rust/issues/3) |
| 3 | Managers | [#4](https://github.com/TriangleBear/ev-ap-rust/issues/4) |
| 4 | UI shell + theme | [#5](https://github.com/TriangleBear/ev-ap-rust/issues/5) |
| 5 | All 7 views | [#6](https://github.com/TriangleBear/ev-ap-rust/issues/6) |
| 6 | Scan flow | [#7](https://github.com/TriangleBear/ev-ap-rust/issues/7) |
| 7 | Parity QA | [#8](https://github.com/TriangleBear/ev-ap-rust/issues/8) |
| 8 | Packaging & release CI | [#9](https://github.com/TriangleBear/ev-ap-rust/issues/9) |
| 9 | Cutover | [#10](https://github.com/TriangleBear/ev-ap-rust/issues/10) |

The full plan lives in [`docs/RUST_MIGRATION.md`](docs/RUST_MIGRATION.md).

## Stack

| Concern | Crate |
|---|---|
| UI | `eframe` / `egui` / `egui_extras` |
| SQLite | `rusqlite` (`bundled`) |
| Sheets | `reqwest` (blocking) + `serde` / `serde_json` |
| Export | `csv` + `rust_xlsxwriter` |
| Save dialogs | `rfd` |
| Config | `serde_json` |
| Background work | `std::thread` + `mpsc` |
| Logging | `tracing` + `tracing_subscriber` |

## Building

```bash
cargo build            # debug
cargo build --release  # optimised binary at target/release/Attendance_App.exe
cargo test             # unit + integration tests
cargo fmt --all        # format
cargo clippy --all-targets -- -D warnings
```

The released artifact is named `Attendance_App_<version>` to stay continuous
with the Python releases.

## Layout

```
src/
├── main.rs              # binary entry point
├── lib.rs               # library root (testable core)
├── app.rs               # shell: sidebar nav, view routing, theme state
├── config.rs            # ev_ap_config.json
├── db/
│   ├── mod.rs           # Backend enum (Sqlite | Sheets) — mirrors DBActions routing
│   ├── sqlite.rs        # local SQLite backend
│   └── sheets.rs        # Google Sheets (Apps Script) backend
├── managers/            # event, member (15s dedup cache), table (scan/export)
├── views/               # home, events, members, reports, settings, help, about
└── theme.rs
tests/                   # integration tests
docs/RUST_MIGRATION.md   # the migration plan
apps-script/             # Google Apps Script source for the Sheets backend
```

## Database backends

The app supports two interchangeable backends, selected in **Settings →
Database**:

- **SQLite** (default) — a local `Ev_Ap.db` file in the working directory.
- **Google Sheets** — the `Members` sheet and the `Events` registry in a main
  spreadsheet, plus one spreadsheet per event.

Both backends are reached through the same façade, mirroring the Python
`DBActions` router, so the UI never needs to know which one is active.

## License

MIT — see [LICENSE](LICENSE).
