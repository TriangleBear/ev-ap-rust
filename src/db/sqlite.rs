//! Local SQLite backend (`rusqlite`).
//!
//! Ported from `App/database/sqlite_db.py` and the SQLite branches of
//! `App/database/dblite.py`. The schema is intentionally identical to the
//! Python app so the same `Ev_Ap.db` file can be opened by both.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OpenFlags};

use super::{normalize_table_name, Backend, DbError, Member, Row, ScanOutcome};

/// Default database file name, resolved against the working directory.
pub const DB_FILE: &str = "Ev_Ap.db";

/// The `Members` table, created by `initialize_db()`.
const CREATE_MEMBERS: &str = "CREATE TABLE IF NOT EXISTS Members (
    rfid TEXT PRIMARY KEY,
    memberid TEXT,
    name TEXT,
    student_num TEXT,
    program TEXT,
    year TEXT,
    date_registered TEXT
)";

/// The `points` column, added to pre-existing databases by `initialize_db()`.
const ADD_POINTS: &str = "ALTER TABLE Members ADD COLUMN points REAL DEFAULT 0";

/// Local SQLite implementation of [`Backend`].
pub struct SqliteBackend {
    conn: Mutex<Connection>,
    path: PathBuf,
}

impl SqliteBackend {
    /// Open (or create) `Ev_Ap.db` in the current working directory.
    pub fn new() -> Result<Self, DbError> {
        SqliteBackend::with_path(Path::new(DB_FILE))
    }

    /// Open (or create) the database at an explicit path.
    pub fn with_path(path: impl AsRef<Path>) -> Result<Self, DbError> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_URI
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Ok(SqliteBackend {
            conn: Mutex::new(conn),
            path,
        })
    }

    /// Path of the database file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `SQLiteDB.db_exists` — does the database file exist on disk?
    pub fn db_exists(&self) -> bool {
        self.path.exists()
    }

    /// `SQLiteDB.initialize_db` — create the `Members` table and migrate it.
    ///
    /// Adds the `points` column when it is missing, so a database written by an
    /// older Python build is upgraded in place.
    pub fn initialize_db(&self) -> Result<(), DbError> {
        let conn = self.lock()?;
        conn.execute_batch(CREATE_MEMBERS)?;

        let has_points: bool = conn.query_row(
            "SELECT COUNT(*) FROM pragma_table_info('Members') WHERE name = 'points'",
            [],
            |row| row.get::<_, i64>(0),
        )? > 0;

        if !has_points {
            conn.execute_batch(ADD_POINTS)?;
        }
        Ok(())
    }

    /// `SQLiteDB.fetch_all_data` — every table with all of its rows.
    pub fn fetch_all_data(&self) -> Result<BTreeMap<String, Vec<Row>>, DbError> {
        let conn = self.lock()?;
        let mut names: Vec<String> = {
            let mut stmt =
                conn.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name;")?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        names.retain(|name| !name.starts_with("sqlite_"));

        let mut data = BTreeMap::new();
        for name in names {
            let sql = format!("SELECT * FROM {}", quote_identifier(&name));
            let mut stmt = conn.prepare(&sql)?;
            let column_names: Vec<String> = stmt
                .column_names()
                .into_iter()
                .map(str::to_string)
                .collect();
            let mut rows = stmt.query([])?;
            let mut collected = Vec::new();
            while let Some(row) = rows.next()? {
                let mut cells = Vec::with_capacity(column_names.len());
                for (index, column) in column_names.iter().enumerate() {
                    let value = match row.get_ref(index)? {
                        rusqlite::types::ValueRef::Null => String::new(),
                        rusqlite::types::ValueRef::Text(bytes) => {
                            String::from_utf8_lossy(bytes).into_owned()
                        }
                        rusqlite::types::ValueRef::Integer(value) => value.to_string(),
                        rusqlite::types::ValueRef::Real(value) => value.to_string(),
                        rusqlite::types::ValueRef::Blob(bytes) => {
                            String::from_utf8_lossy(bytes).into_owned()
                        }
                    };
                    cells.push((column.clone(), value));
                }
                collected.push(Row::new(cells));
            }
            data.insert(name, collected);
        }
        Ok(data)
    }

    /// Does a table with this name exist?
    pub fn table_exists(&self, table_name: &str) -> Result<bool, DbError> {
        let conn = self.lock()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name = ?1",
            params![table_name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DbError> {
        self.conn
            .lock()
            .map_err(|e| DbError::Backend(format!("database lock poisoned: {e}")))
    }
}

/// Quote an identifier for use in a SQL statement.
///
/// Only ever called with identifiers that already passed
/// [`normalize_table_name`], so the backticks cannot be escaped by the caller.
fn quote_identifier(identifier: &str) -> String {
    format!("`{}`", identifier.replace('`', ""))
}

fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

impl Backend for SqliteBackend {
    fn member_register(
        &self,
        rfid: &str,
        memberid: &str,
        name: &str,
        student_num: &str,
        program: &str,
        year: &str,
    ) -> Result<(), DbError> {
        let conn = self.lock()?;
        conn.execute(
            "INSERT INTO Members (rfid, memberid, name, student_num, program, year, date_registered)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![rfid, memberid, name, student_num, program, year, today()],
        )?;
        Ok(())
    }

    fn member_exists(&self, rfid: &str) -> Result<Option<Member>, DbError> {
        let conn = self.lock()?;
        let mut stmt =
            conn.prepare("SELECT memberid, name, student_num FROM Members WHERE rfid = ?1")?;
        let mut rows = stmt.query(params![rfid])?;
        let member = match rows.next()? {
            Some(row) => Some(Member {
                memberid: row.get(0)?,
                name: row.get(1)?,
                student_num: row.get(2)?,
            }),
            None => None,
        };
        Ok(member)
    }

    fn list_tables(&self) -> Result<Vec<String>, DbError> {
        let conn = self.lock()?;
        let mut stmt = conn.prepare("SELECT name FROM sqlite_master WHERE type='table';")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        let mut tables = Vec::new();
        for table in rows {
            let table = table?;
            if table != "Members" && table != "sqlite_sequence" {
                tables.push(table);
            }
        }
        Ok(tables)
    }

    fn create_event_table(&self, table_name: &str) -> Result<(), DbError> {
        let table = normalize_table_name(table_name)?;
        let conn = self.lock()?;
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {} (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                rfid TEXT NOT NULL,
                memberid TEXT NOT NULL,
                student_num TEXT NOT NULL,
                name TEXT NOT NULL,
                attendance_time TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                FOREIGN KEY (rfid) REFERENCES Members(rfid)
            )",
            quote_identifier(&table)
        ))?;
        Ok(())
    }

    fn fetch_table_data(&self, table_name: &str) -> Result<Vec<Row>, DbError> {
        let conn = self.lock()?;
        let (sql, columns): (String, Vec<&str>) = if table_name == "Members" {
            (
                "SELECT DISTINCT rfid, memberid, name, student_num, date_registered FROM Members"
                    .to_string(),
                vec!["rfid", "memberid", "name", "student_num", "date_registered"],
            )
        } else {
            let table = normalize_table_name(table_name)?;
            (
                format!(
                    "SELECT DISTINCT memberid, name, student_num, attendance_time FROM {}",
                    quote_identifier(&table)
                ),
                vec!["memberid", "name", "student_num", "attendance_time"],
            )
        };

        let mut stmt = conn.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        let mut result = Vec::new();
        while let Some(row) = rows.next()? {
            let mut cells = Vec::with_capacity(columns.len());
            for (index, column) in columns.iter().enumerate() {
                let value = match row.get_ref(index)? {
                    rusqlite::types::ValueRef::Null => String::new(),
                    rusqlite::types::ValueRef::Text(bytes) => {
                        String::from_utf8_lossy(bytes).into_owned()
                    }
                    rusqlite::types::ValueRef::Integer(value) => value.to_string(),
                    rusqlite::types::ValueRef::Real(value) => value.to_string(),
                    rusqlite::types::ValueRef::Blob(bytes) => {
                        String::from_utf8_lossy(bytes).into_owned()
                    }
                };
                cells.push(((*column).to_string(), value));
            }
            result.push(Row::new(cells));
        }
        Ok(result)
    }

    fn attendance_member_event(&self, table_name: &str, rfid: &str) -> Result<(), DbError> {
        let table = normalize_table_name(table_name)?;
        let member = self
            .member_exists(rfid)?
            .ok_or_else(|| DbError::Backend("No member found with the given RFID".into()))?;

        let conn = self.lock()?;
        conn.execute(
            &format!(
                "INSERT INTO {} (rfid, memberid, name, student_num) VALUES (?1, ?2, ?3, ?4)",
                quote_identifier(&table)
            ),
            params![rfid, member.memberid, member.name, member.student_num],
        )?;
        Ok(())
    }

    fn scan_attendance(&self, table_name: &str, rfid: &str) -> Result<ScanOutcome, DbError> {
        if self.member_exists(rfid)?.is_none() {
            return Ok(ScanOutcome::failed("RFID number not found"));
        }
        if self.member_attended_event(table_name, rfid)? {
            return Ok(ScanOutcome::already_attended());
        }
        self.attendance_member_event(table_name, rfid)?;
        Ok(ScanOutcome::recorded())
    }

    fn get_member_name(&self, rfid: &str) -> Result<Option<String>, DbError> {
        Ok(self.member_exists(rfid)?.map(|member| member.name))
    }

    fn member_attended_event(&self, table_name: &str, rfid: &str) -> Result<bool, DbError> {
        let table = normalize_table_name(table_name)?;
        let conn = self.lock()?;
        let count: i64 = conn.query_row(
            &format!(
                "SELECT COUNT(*) FROM {} WHERE rfid = ?1",
                quote_identifier(&table)
            ),
            params![rfid],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn add_points(&self, rfid: &str, points: f64) -> Result<(), DbError> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE Members SET points = COALESCE(points, 0) + ?1 WHERE rfid = ?2",
            params![points, rfid],
        )?;
        Ok(())
    }

    fn get_member_points(&self, rfid: &str) -> Result<f64, DbError> {
        let conn = self.lock()?;
        let points: Option<f64> = conn
            .query_row(
                "SELECT COALESCE(points, 0) AS points FROM Members WHERE rfid = ?1",
                params![rfid],
                |row| row.get(0),
            )
            .ok();
        Ok(points.unwrap_or(0.0))
    }

    fn redeem_points(&self, rfid: &str, points: f64) -> Result<(), DbError> {
        let conn = self.lock()?;
        conn.execute(
            "UPDATE Members SET points = COALESCE(points, 0) - ?1
             WHERE rfid = ?2 AND COALESCE(points, 0) >= ?3",
            params![points, rfid, points],
        )?;
        Ok(())
    }

    fn delete_event_table(&self, table_name: &str) -> Result<(), DbError> {
        let table = normalize_table_name(table_name)?;
        let conn = self.lock()?;
        conn.execute_batch(&format!(
            "DROP TABLE IF EXISTS {}",
            quote_identifier(&table)
        ))?;
        Ok(())
    }
}
