//! Drawing a table of rows.

use crate::db::Row;
use egui_extras::{Column, TableBuilder};

/// Draw the rows as a table with the given columns.
///
/// The columns that a row does not have are drawn as empty cells, so a table
/// stays readable when the backends return different columns.
pub fn draw_table(ui: &mut egui::Ui, rows: &[Row], columns: &[&str]) {
    TableBuilder::new(ui)
        .striped(true)
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .columns(Column::auto(), columns.len())
        .header(20.0, |mut header| {
            for column in columns {
                header.col(|ui| {
                    ui.strong(*column);
                });
            }
        })
        .body(|mut body| {
            for data_row in rows {
                body.row(20.0, |mut row| {
                    for column in columns {
                        let value = data_row.get(column).unwrap_or_default().to_string();
                        row.col(|ui| {
                            ui.label(value);
                        });
                    }
                });
            }
        });
}
