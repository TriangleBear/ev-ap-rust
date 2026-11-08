//! CSV and Excel export.
//!
//! Ported from the `export_to_file` closure in `App/managers/table_manager.py`,
//! which built a `pandas.DataFrame` from the rows and wrote it with
//! `to_csv` / `to_excel`. No cell formatting is used in the Python app, so
//! write-only XLSX output loses nothing.

use std::path::{Path, PathBuf};

use crate::db::Row;

/// Errors raised while exporting.
#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    /// The file could not be written.
    #[error("could not write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The CSV file could not be written.
    #[error("could not write {path}: {source}")]
    Csv {
        path: PathBuf,
        #[source]
        source: csv::Error,
    },
    /// The spreadsheet could not be written.
    #[error("could not write {path}: {source}")]
    Workbook {
        path: PathBuf,
        #[source]
        source: rust_xlsxwriter::XlsxError,
    },
}

/// The column names of a set of rows, taken from the first row.
pub fn headers(rows: &[Row]) -> Vec<String> {
    rows.first()
        .map(|row| row.columns().iter().map(|name| name.to_string()).collect())
        .unwrap_or_default()
}

/// Write the rows to a CSV file.
///
/// The output matches `pandas.DataFrame.to_csv(path, index=False)`: a header
/// row followed by one line per row, LF terminated.
pub fn write_csv(rows: &[Row], path: &Path) -> Result<(), ExportError> {
    let mut writer = csv::Writer::from_path(path).map_err(|source| ExportError::Csv {
        path: path.to_path_buf(),
        source,
    })?;

    let columns = headers(rows);
    if !columns.is_empty() {
        writer
            .write_record(&columns)
            .map_err(|source| ExportError::Csv {
                path: path.to_path_buf(),
                source,
            })?;
    }

    for row in rows {
        let values: Vec<String> = columns
            .iter()
            .map(|column| row.get(column).unwrap_or_default().to_string())
            .collect();
        writer
            .write_record(&values)
            .map_err(|source| ExportError::Csv {
                path: path.to_path_buf(),
                source,
            })?;
    }

    writer.flush().map_err(|source| ExportError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// Write the rows to an Excel workbook.
///
/// The output matches `pandas.DataFrame.to_excel(path, index=False)`: one
/// sheet, a header row and one row per record.
pub fn write_xlsx(rows: &[Row], path: &Path) -> Result<(), ExportError> {
    let mut workbook = rust_xlsxwriter::Workbook::new();
    {
        let columns = headers(rows);
        let sheet = workbook.add_worksheet();

        for (index, column) in columns.iter().enumerate() {
            sheet
                .write(0, index as u16, column.as_str())
                .map_err(|source| ExportError::Workbook {
                    path: path.to_path_buf(),
                    source,
                })?;
        }

        for (row_index, row) in rows.iter().enumerate() {
            for (column_index, column) in columns.iter().enumerate() {
                let value = row.get(column).unwrap_or_default();
                sheet
                    .write((row_index + 1) as u32, column_index as u16, value)
                    .map_err(|source| ExportError::Workbook {
                        path: path.to_path_buf(),
                        source,
                    })?;
            }
        }
    }

    workbook.save(path).map_err(|source| ExportError::Workbook {
        path: path.to_path_buf(),
        source,
    })
}

/// Write the rows to a file, choosing the format from the extension.
///
/// `.csv` writes CSV, `.xlsx` writes an Excel workbook.
pub fn write(rows: &[Row], path: &Path) -> Result<(), ExportError> {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("xlsx") => write_xlsx(rows, path),
        _ => write_csv(rows, path),
    }
}

/// Ask the user where to save the export.
///
/// Returns `None` when the dialog was cancelled. The dialog offers both
/// formats, like the Python app's file dialog.
pub fn save_dialog(default_name: &str) -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Export")
        .set_file_name(default_name)
        .add_filter("CSV files", &["csv"])
        .add_filter("Excel files", &["xlsx"])
        .save_file()
}
