# Ev_Ap → Rust Migration Plan

**Status:** In progress — this document now lives in the Rust repo (`docs/RUST_MIGRATION.md`).
**Decisions (locked):** egui/eframe UI · Full feature parity · Separate new repo · This doc moved to the new repo's `docs/` in Phase 0.

> **Tracking:** Each phase below is tracked as a GitHub issue in this repo — issues [#1](https://github.com/TriangleBear/ev-ap-rust/issues/1)–[#10](https://github.com/TriangleBear/ev-ap-rust/issues/10), one per phase, with the phase bullets as a checklist. Each phase is delivered as its own PR with a test case per checklist item.

---

## 1. Current state (what we're porting)

Python 3.12 + CustomTkinter app, ~2,300 LOC.

| Area | Python today | Notes |
|---|---|---|
| Entry / shell | `App/rfid_app.py` — `MainApp`, sidebar nav, 7 lazy-loaded views, background DB init with progress bar | 1100×650 window |
| DB facade | `App/database/dblite.py` — `DBActions` static methods routing per-call to SQLite or Sheets based on `db_mode` | 17 methods |
| SQLite backend | `App/database/sqlite_db.py` — `Ev_Ap.db`, `Members` table + `points` column migration, one table per event | Schema must stay identical |
| Sheets backend | `App/database/sheet_db.py` — HTTP client for **14 Apps Script actions** (`addMember`, `scanAttendance`, `deleteEvent`, …) | `gsheet_api.gs` is server-side JS — **unchanged, no redeploy needed** |
| Config | `App/database/config.py` — `ev_ap_config.json` (`db_mode`, `gsheet_api_url`) in CWD | Must stay format-compatible |
| Managers | `App/managers/` — event create, member register + **15s RFID dedup cache**, table window with **threaded scan + loading spinner**, search/filter, CSV/Excel export | Core business logic |
| Views | `App/views/` — home, events, members, reports, settings (mode toggle, GS URL, test connection, set spreadsheet), help, about (hardcoded version string) | |
| Export | pandas → `to_csv` / `to_excel` + `tkinter.filedialog` | |
| Tests | `App/TEST/` — 3 pytest files, **7 tests** (register/exists, scan attendance, dedup ignore) | Port these first |
| CI | `.github/workflows/build-release.yml` — pytest → branch-name version bump (`major/*`, `feature/*`, else patch) → PyInstaller `.exe` → tag + release | Version injected by editing `about_view.py` |
| RFID input | Keyboard-wedge scanner types into an entry + `Return` binding | No serial/HID driver needed — keep same model |

## 2. Rust stack

| Concern | Crate | Rationale |
|---|---|---|
| UI | `eframe` + `egui` + `egui_extras` (tables) | Immediate mode — fastest path to tables/modals/forms parity |
| Message boxes | `egui-modal` or egui windows | Replaces CTkMessagebox |
| Save dialogs | `rfd` | Replaces `tkinter.filedialog` (native OS dialogs) |
| SQLite | `rusqlite` with `bundled` feature | Drop-in compatible with `Ev_Ap.db`, no system dep |
| HTTP | `reqwest` (blocking) + `serde` / `serde_json` | SheetDB calls; blocking keeps threading model simple |
| CSV / Excel | `csv` + `rust_xlsxwriter` | Replaces pandas (no formatting is used, so no loss) |
| Config | `serde_json` | Same `ev_ap_config.json` file, same keys |
| Background work | `std::thread` + `mpsc` channel → `ctx.request_repaint()` | Mirrors Python's `threading` + `.after()` pattern |
| Logging | `tracing` + `tracing_subscriber` | Replaces `icecream.ic()` |
| Version | `env!("CARGO_PKG_VERSION")` injected at build time | Replaces the CI edit of `about_view.py` |

### Target repo layout (new repo, e.g. `ev-ap-rust`)

```
src/
├── main.rs              # eframe bootstrap
├── app.rs               # shell: sidebar nav, view routing, theme state
├── config.rs            # ev_ap_config.json
├── db/
│   ├── mod.rs           # Backend enum (Sqlite | Sheets) — mirrors DBActions routing
│   ├── sqlite.rs
│   └── sheets.rs        # 14 Apps Script actions
├── managers/            # event, member (15s cache), table (scan/export)
├── views/               # home, events, members, reports, settings, help, about
└── theme.rs
tests/                   # ported pytest suite
docs/RUST_MIGRATION.md   # this document
.github/workflows/       # ported build-release.yml (cargo instead of PyInstaller)
```

Estimated size: **~4–6k LOC** (Rust runs ~1.5–2× Python for this shape of app).

## 3. Phases

### Phase 0 — Repo & scaffold
- Create the new repo (`ev-ap-rust` or similar).
- `cargo init`, CI skeleton (`cargo fmt` / `clippy` / `test` on push), MIT license, README.
- Copy `gsheet_api.gs` + reference docs into the new repo.
- **Move this document** to the new repo's `docs/RUST_MIGRATION.md`.
- Decide binary/package name — recommend keeping `Attendance_App_<ver>` for release continuity.

### Phase 1 — Data layer (SQLite)
- `config.rs`, `db/sqlite.rs` with an enum/trait mirroring all 17 `DBActions` methods.
- Identical schema (incl. `points` column auto-migration).
- **Port the 7 pytest tests** to Rust — this is the parity gate for the core.

### Phase 2 — Sheets backend
- `db/sheets.rs` covering all 14 Apps Script actions.
- Error-message parity — the Python client has carefully worded messages for sign-in redirects, 404, non-JSON responses; port them verbatim.
- Test-connection + set-spreadsheet flows.
- Test with `wiremock` or a manual checklist against a real deployment.

### Phase 3 — Managers
- Member register + 15s dedup cache.
- Event create/delete (table-per-event / sheet-per-event).
- Batched `scan_attendance`, points add/redeem.
- CSV/XLSX export.

### Phase 4 — UI shell + theme
- eframe window (1100×650), sidebar navigation, lazy view init.
- Light/Dark/System theming, background init with progress display.
- Modal/messagebox component.

### Phase 5 — All 7 views
- Port view-by-view against the Python source: home → events → members → reports → settings → help → about.
- Settings gets full parity (mode toggle, GS URL save/test/set/apply).

### Phase 6 — Scan flow
- Event table window, RFID entry with Enter binding.
- Background scan thread with loading spinner, dedup message, already-attended / not-found error paths.
- Highest-interaction feature — do last among UI work.

### Phase 7 — Parity QA
- Side-by-side checklist against the Python app using the **same `Ev_Ap.db`** (must open without migration), plus a fresh-DB run.
- Cover: register / scan / points / export / dedup / mode-switch, both backends.

### Phase 8 — Packaging & release CI
- Port `build-release.yml`: `cargo test` → same branch-name version-bump rules → build Windows `.exe` (recommend `cargo-dist` or plain `cargo build --release` + zip, replacing PyInstaller) → tag + release with auto notes.
- Version comes from the tag via env var — no source-file editing.

### Phase 9 — Cutover
- Final README, archive note pointing Python users at the Rust version.
- Keep the Python repo as read-only reference.

## 4. Risks & gotchas

- **Visual parity won't be exact** — egui looks different from CustomTkinter. Behavioral parity is the bar, not pixel parity.
- **SQL table names are string-interpolated** in Python (event names → table names). Rust must validate identifiers (`[A-Za-z0-9_]` after space→underscore) rather than repeat the injection-prone pattern.
- **egui focus vs. keyboard-wedge scanners** — scans arrive as rapid keystrokes + Enter; verify the RFID entry regains focus automatically after each scan/messagebox (Python had implicit Tk focus behavior).
- **Excel export** via `rust_xlsxwriter` is write-only vs. pandas — fine here since we only write.
- **Do not port:** root-level strays (`time.py`, `AHO_MEMBER.db`, empty `managers/` / `database/` dirs) and `.vscode/settings.json` (contains live Aiven MySQL credentials — the Rust repo must not include it).
- **`gsheet_api.gs` needs zero changes** — the Apps Script contract is HTTP-level, so existing deployments keep working.

## 5. Definition of done

1. All 7 ported tests pass under `cargo test`.
2. Existing `Ev_Ap.db` opens and reads/writes identically.
3. Every checklist item in Phase 7 passes against both backends.
4. `cargo clippy` clean, CI green on push.
5. Windows artifact builds and runs on a clean machine (no Python installed).
6. This document + README updated to reflect the Rust build.
