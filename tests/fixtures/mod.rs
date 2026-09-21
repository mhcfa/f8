//! Shared model fixtures for end-to-end compiler testing and example exports.

#![allow(dead_code)]

pub mod lbo;
pub mod styling_showcase;
pub mod three_statement;

use indexmap::IndexMap;

use finx::model::format::{CurrencyCode, FormatPreset, SemanticStyle};
use finx::model::id::ColId;
use finx::model::table::{CellInput, ColumnSpec, RowSpec};

/// Builds a measure column with default ID label.
pub fn build_measure_col(id: &str) -> ColumnSpec {
    ColumnSpec::new(id).with_header(id)
}

/// Builds a measure column with an explicit display label.
pub fn build_measure_col_label(id: &str, label: &str) -> ColumnSpec {
    ColumnSpec::new(id).with_header(label)
}

/// Builds a descriptor column for row labels / metadata.
pub fn build_descriptor_col(id: &str, label: &str) -> ColumnSpec {
    ColumnSpec::descriptor(id, label)
}

/// Builds a primary descriptor column.
pub fn build_primary_descriptor_col(id: &str, label: &str) -> ColumnSpec {
    ColumnSpec::primary_descriptor(id, label)
}

/// Fluent builder for constructing fully styled, labeled `RowSpec` definitions.
#[derive(Debug, Clone)]
pub struct RowBuilder {
    spec: RowSpec,
}

impl RowBuilder {
    /// Creates a new row specification with the given identifier.
    pub fn new(id: &str) -> Self {
        Self {
            spec: RowSpec::new(id),
        }
    }

    /// Creates a new row specification with a descriptor column display label.
    pub fn descriptor(id: &str, desc_col: &str, label: &str) -> Self {
        let row = RowSpec::new(id).with_cell(desc_col, CellInput::string(label));
        Self { spec: row }
    }

    /// Convenience constructor attaching a display label to the standard `"line_item"` descriptor column.
    pub fn line_item(id: &str, label: &str) -> Self {
        Self::descriptor(id, "line_item", label)
    }

    /// Sets the typographic indentation level.
    pub fn indent(mut self, indent: u8) -> Self {
        self.spec = self.spec.with_indent(Some(indent));
        self
    }

    /// Sets the format preset.
    pub fn format(mut self, format: FormatPreset) -> Self {
        self.spec = self.spec.with_format(Some(format));
        self
    }

    /// Sets display fractional decimal precision.
    pub fn decimals(mut self, decimals: u32) -> Self {
        self.spec = self.spec.with_decimals(Some(decimals));
        self
    }

    /// Sets the row-level currency denomination.
    pub fn currency(mut self, currency: CurrencyCode) -> Self {
        self.spec = self.spec.with_currency(Some(currency));
        self
    }

    /// Sets the semantic presentation style (Subtotal, Total).
    pub fn style(mut self, style: SemanticStyle) -> Self {
        self.spec = self.spec.with_style(Some(style));
        self
    }

    /// Sets the highlight visual emphasis flag.
    pub fn highlight(mut self, highlight: bool) -> Self {
        self.spec = self.spec.with_highlight(Some(highlight));
        self
    }

    /// Sets the row-wide vector formula expression.
    pub fn expr(mut self, expr: impl Into<String>) -> Self {
        self.spec = self.spec.with_expr(expr);
        self
    }

    /// Attaches an arbitrary cell override.
    pub fn cell(mut self, col: &str, input: CellInput) -> Self {
        self.spec = self.spec.with_cell(col, input);
        self
    }

    /// Attaches a literal number cell override.
    pub fn cell_num(mut self, col: &str, val: f64) -> Self {
        self.spec = self.spec.with_cell(col, CellInput::number(val));
        self
    }

    /// Attaches a literal number cell override with custom format preset and decimals.
    pub fn cell_num_format(
        mut self,
        col: &str,
        val: f64,
        format: FormatPreset,
        decimals: u32,
    ) -> Self {
        self.spec = self.spec.with_cell(
            col,
            CellInput::Detailed(finx::model::table::DetailedCellInput {
                value: Some(finx::model::table::ValueLiteral::Number(val)),
                format: Some(format),
                decimals: Some(decimals),
                ..Default::default()
            }),
        );
        self
    }

    /// Attaches a literal string cell override.
    pub fn cell_str(mut self, col: &str, val: &str) -> Self {
        self.spec = self.spec.with_cell(col, CellInput::string(val));
        self
    }

    /// Attaches a formula override for a specific column.
    pub fn cell_formula(mut self, col: &str, expr: &str) -> Self {
        self.spec = self.spec.with_cell(col, CellInput::formula(expr));
        self
    }

    /// Attaches multiple numerical cell overrides.
    pub fn cells_num(mut self, values: &[(&str, f64)]) -> Self {
        for &(col, val) in values {
            self.spec = self.spec.with_cell(col, CellInput::number(val));
        }
        self
    }

    /// Attaches multiple typed cell inputs.
    pub fn cells(mut self, values: &[(&str, CellInput)]) -> Self {
        for &(col, ref input) in values {
            self.spec = self.spec.with_cell(col, input.clone());
        }
        self
    }

    /// Builds and returns the final `RowSpec`.
    pub fn build(self) -> RowSpec {
        self.spec
    }
}

impl From<RowBuilder> for RowSpec {
    fn from(b: RowBuilder) -> Self {
        b.build()
    }
}

pub fn build_point_row(id: &str, values: &[(&str, CellInput)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, ref input) in values {
        cells.insert(ColId::from(col), input.clone());
    }
    RowSpec::new(id)
        .with_format(Some(FormatPreset::Number))
        .with_cells(cells)
}

pub fn build_point_row_num(id: &str, values: &[(&str, f64)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, val) in values {
        cells.insert(ColId::from(col), CellInput::number(val));
    }
    RowSpec::new(id)
        .with_format(Some(FormatPreset::Number))
        .with_cells(cells)
}

pub fn build_vector_row(id: &str, expr: &str, overrides: &[(&str, CellInput)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, ref input) in overrides {
        cells.insert(ColId::from(col), input.clone());
    }
    RowSpec::new(id)
        .with_format(Some(FormatPreset::Number))
        .with_expr(expr)
        .with_cells(cells)
}

pub fn build_vector_row_num(id: &str, expr: &str, overrides: &[(&str, f64)]) -> RowSpec {
    let mut cells = IndexMap::new();
    for &(col, val) in overrides {
        cells.insert(ColId::from(col), CellInput::number(val));
    }
    RowSpec::new(id)
        .with_format(Some(FormatPreset::Number))
        .with_expr(expr)
        .with_cells(cells)
}
