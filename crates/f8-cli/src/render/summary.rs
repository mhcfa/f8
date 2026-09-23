use f8_core::compiler::compiled::ModelSummary;

/// Formats the high-level model overview summary into a human-readable string card.
#[must_use]
pub fn render_summary(summary: &ModelSummary, duration_ms: Option<f64>) -> String {
    let mut out = String::new();
    out.push_str(&format!("Model:     {}\n", summary.model_name));
    out.push_str(&format!("Theme:     {}\n", summary.theme_name));
    out.push_str(&format!("Sheets ({}):\n", summary.total_sheets));

    for sheet in &summary.sheets {
        let table_label = if sheet.tables.len() == 1 {
            "1 table".to_string()
        } else {
            format!("{} tables", sheet.tables.len())
        };
        let cell_label = if sheet.total_cells == 1 {
            "1 cell".to_string()
        } else {
            format!("{} cells", sheet.total_cells)
        };
        out.push_str(&format!(
            "  • {} ({}, {})\n",
            sheet.name, table_label, cell_label
        ));

        for table in &sheet.tables {
            out.push_str(&format!(
                "      - {} [{} rows x {} cols] ({} cells)\n",
                table.id, table.row_count, table.col_count, table.cell_count
            ));
        }
    }

    out.push_str(&format!(
        "Total:     {} sheets, {} tables, {} cells ({} driver inputs, {} formulas)\n",
        summary.total_sheets,
        summary.total_tables,
        summary.total_cells,
        summary.total_drivers,
        summary.total_formulas
    ));

    if let Some(ms) = duration_ms {
        out.push_str(&format!(
            "Status:    DAG acyclic (0 cycles), evaluated in {:.1}ms",
            ms
        ));
    } else {
        out.push_str("Status:    DAG acyclic (0 cycles)");
    }

    out
}
