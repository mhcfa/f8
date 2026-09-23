use f8_core::compiler::compiled::CellInspection;

/// Formats an individual cell inspection record into a detailed audit card.
#[must_use]
pub fn render_cell(cell: &CellInspection) -> String {
    let mut out = String::new();
    out.push_str(&format!("Coordinate:     {}\n", cell.coordinate));
    out.push_str(&format!("Sheet Address:  {}\n", cell.physical_address));

    let formatted_val = cell.formatted_value.as_deref().unwrap_or_default();
    let mut meta = Vec::new();
    if let Some(f) = cell.format {
        meta.push(format!("{f:?}"));
    }
    if let Some(d) = cell.decimals {
        meta.push(if d == 1 {
            "1 decimal".to_string()
        } else {
            format!("{d} decimals")
        });
    }
    if let Some(c) = cell.currency {
        meta.push(c.as_str().to_uppercase());
    }

    if meta.is_empty() {
        out.push_str(&format!("Evaluated:      {}\n", formatted_val));
    } else {
        out.push_str(&format!(
            "Evaluated:      {} ({})\n",
            formatted_val,
            meta.join(", ")
        ));
    }

    if cell.is_formula {
        if let Some(expr) = &cell.formula_expr {
            out.push_str(&format!("Formula Expr:   {}\n", expr));
        }
        if let Some(excel) = &cell.excel_formula {
            out.push_str(&format!("Excel Formula:  {}\n", excel));
        }
    } else {
        out.push_str("Type:           Driver Input (Literal)\n");
    }

    if let Some(note) = &cell.note {
        out.push_str(&format!("Note:           {}\n", note));
    }

    out.push('\n');

    if cell.precedents.is_empty() {
        out.push_str("Precedents:     None (Input driver)\n");
    } else {
        out.push_str(&format!("Precedents ({}):\n", cell.precedents.len()));
        let count = cell.precedents.len();
        for (i, p) in cell.precedents.iter().enumerate() {
            let branch = if i == count - 1 {
                "└── "
            } else {
                "├── "
            };
            let val_suffix = match &p.formatted_value {
                Some(v) if !v.is_empty() => format!(" = {v}"),
                _ => String::new(),
            };
            out.push_str(&format!("  {}{}{val_suffix}\n", branch, p.cell_id));
        }
    }

    out.push('\n');

    if cell.dependents.is_empty() {
        out.push_str("Dependents:     None (Terminal output)\n");
    } else {
        out.push_str(&format!("Dependents ({}):\n", cell.dependents.len()));
        let count = cell.dependents.len();
        for (i, d) in cell.dependents.iter().enumerate() {
            let branch = if i == count - 1 {
                "└── "
            } else {
                "├── "
            };
            let val_suffix = match &d.formatted_value {
                Some(v) if !v.is_empty() => format!(" = {v}"),
                _ => String::new(),
            };
            out.push_str(&format!("  {}{}{val_suffix}\n", branch, d.cell_id));
        }
    }

    out
}
