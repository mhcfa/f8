use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::ModelError;
use super::format::{FormatPreset, SemanticStyle};
use super::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};

/// Calculation axis role: measures broadcast formulas; descriptors isolate labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AxisRole {
    #[default]
    Measure,
    Descriptor,
}

impl AxisRole {
    /// Returns true if this axis is a measure.
    pub fn is_measure(&self) -> bool {
        matches!(self, Self::Measure)
    }

    /// Returns true if this axis is a descriptor.
    pub fn is_descriptor(&self) -> bool {
        matches!(self, Self::Descriptor)
    }
}

/// Column definition within a table specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColumnSpec {
    pub id: ColId,
    #[serde(default)]
    pub role: AxisRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<SemanticStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
}

impl ColumnSpec {
    /// Creates a measure column specification with default attributes.
    pub fn new(id: impl Into<ColId>) -> Self {
        Self {
            id: id.into(),
            role: AxisRole::Measure,
            label: None,
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            expr: None,
            note: None,
            source: None,
        }
    }

    /// Creates a descriptor column specification with an explicit display label.
    pub fn descriptor(id: impl Into<ColId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: AxisRole::Descriptor,
            label: Some(label.into()),
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            expr: None,
            note: None,
            source: None,
        }
    }

    /// Builder method attaching a formatting preset.
    pub fn with_format(mut self, format: Option<FormatPreset>) -> Self {
        self.format = format;
        self
    }

    /// Builder method attaching a structural style.
    pub fn with_style(mut self, style: Option<SemanticStyle>) -> Self {
        self.style = style;
        self
    }

    /// Builder method attaching visual highlight emphasis.
    pub fn with_highlight(mut self, highlight: Option<bool>) -> Self {
        self.highlight = highlight;
        self
    }

    /// Builder method attaching display precision.
    pub fn with_decimals(mut self, decimals: Option<u32>) -> Self {
        self.decimals = decimals;
        self
    }

    /// Builder method attaching a formula expression.
    pub fn with_expr(mut self, expr: impl Into<String>) -> Self {
        self.expr = Some(expr.into());
        self
    }

    /// Builder method attaching an audit note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Builder method attaching provenance source metadata.
    pub fn with_source(mut self, source: Option<serde_json::Value>) -> Self {
        self.source = source;
        self
    }

    /// Returns the effective display label, falling back to the column ID string.
    pub fn display_label(&self) -> &str {
        self.label.as_deref().unwrap_or(self.id.as_str())
    }
}

impl Default for ColumnSpec {
    fn default() -> Self {
        Self::new("")
    }
}

/// Contiguous column group definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnGroup {
    pub id: ColGroupId,
    pub label: String,
    pub columns: Vec<ColId>,
}

/// Child node in a hierarchical row group: either a direct row ID or a nested sub-group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RowGroupChild {
    Row(RowId),
    Group(RowGroup),
}

/// Hierarchical row section definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowGroup {
    pub id: RowGroupId,
    pub label: String,
    pub children: Vec<RowGroupChild>,
}

/// Explicit literal cell value: number, string, boolean, or null.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueLiteral {
    Number(f64),
    String(String),
    Boolean(bool),
    Null,
}

impl fmt::Display for ValueLiteral {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Number(n) => write!(f, "{n}"),
            Self::String(s) => write!(f, "{s}"),
            Self::Boolean(b) => write!(f, "{b}"),
            Self::Null => write!(f, "null"),
        }
    }
}

impl<'de> Deserialize<'de> for ValueLiteral {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> serde::de::Visitor<'de> for ValueVisitor {
            type Value = ValueLiteral;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a number, string, boolean, or null")
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(ValueLiteral::Boolean(v))
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(ValueLiteral::Number(v as f64))
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(ValueLiteral::Number(v as f64))
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                Ok(ValueLiteral::Number(v))
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(ValueLiteral::String(v.to_string()))
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(ValueLiteral::String(v))
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(ValueLiteral::Null)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(ValueLiteral::Null)
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

impl Serialize for ValueLiteral {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Number(n) => serializer.serialize_f64(*n),
            Self::String(s) => serializer.serialize_str(s),
            Self::Boolean(b) => serializer.serialize_bool(*b),
            Self::Null => serializer.serialize_unit(),
        }
    }
}

/// Detailed cell override containing optional value, formula expression, formatting, note, or provenance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DetailedCellInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<ValueLiteral>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<SemanticStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
}

/// Cell input definition: raw literal, formula expression, or detailed override.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CellInput {
    Literal(ValueLiteral),
    Detailed(DetailedCellInput),
}

impl CellInput {
    /// Constructs a literal number cell input.
    pub fn number(n: impl Into<f64>) -> Self {
        Self::Literal(ValueLiteral::Number(n.into()))
    }

    /// Constructs a literal string cell input.
    pub fn string(s: impl Into<String>) -> Self {
        Self::Literal(ValueLiteral::String(s.into()))
    }

    /// Constructs a literal boolean cell input.
    pub fn boolean(b: bool) -> Self {
        Self::Literal(ValueLiteral::Boolean(b))
    }

    /// Constructs an explicit null cell input.
    pub fn null() -> Self {
        Self::Literal(ValueLiteral::Null)
    }

    /// Constructs a formula expression cell override.
    pub fn formula(expr: impl Into<String>) -> Self {
        Self::Detailed(DetailedCellInput {
            expr: Some(expr.into()),
            ..Default::default()
        })
    }

    /// Returns the literal value reference if present.
    pub fn as_value(&self) -> Option<&ValueLiteral> {
        match self {
            Self::Literal(lit) => Some(lit),
            Self::Detailed(d) => d.value.as_ref(),
        }
    }

    /// Returns the formula expression string if present.
    pub fn as_expr(&self) -> Option<&str> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.expr.as_deref(),
        }
    }

    /// Returns the structural style override if present.
    pub fn style(&self) -> Option<SemanticStyle> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.style,
        }
    }

    /// Returns the visual emphasis override if present.
    pub fn highlight(&self) -> Option<bool> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.highlight,
        }
    }

    /// Returns the format preset override if present.
    pub fn format(&self) -> Option<FormatPreset> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.format,
        }
    }

    /// Returns the display precision override if present.
    pub fn decimals(&self) -> Option<u32> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.decimals,
        }
    }

    /// Returns the audit note if present.
    pub fn note(&self) -> Option<&str> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.note.as_deref(),
        }
    }

    /// Returns the origin provenance metadata if present.
    pub fn source(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.source.as_ref(),
        }
    }
}

impl From<f64> for CellInput {
    fn from(n: f64) -> Self {
        Self::number(n)
    }
}

impl From<i64> for CellInput {
    fn from(n: i64) -> Self {
        Self::number(n as f64)
    }
}

impl From<i32> for CellInput {
    fn from(n: i32) -> Self {
        Self::number(n as f64)
    }
}

impl From<&str> for CellInput {
    fn from(s: &str) -> Self {
        Self::string(s)
    }
}

impl From<String> for CellInput {
    fn from(s: String) -> Self {
        Self::string(s)
    }
}

impl From<bool> for CellInput {
    fn from(b: bool) -> Self {
        Self::boolean(b)
    }
}

/// Row definition within a table specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowSpec {
    pub id: RowId,
    #[serde(default)]
    pub role: AxisRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<SemanticStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub cells: IndexMap<ColId, CellInput>,
}

impl RowSpec {
    /// Creates a measure row specification with default attributes.
    pub fn new(id: impl Into<RowId>) -> Self {
        Self {
            id: id.into(),
            role: AxisRole::Measure,
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            expr: None,
            note: None,
            source: None,
            cells: IndexMap::new(),
        }
    }

    /// Creates a descriptor row specification.
    pub fn descriptor(id: impl Into<RowId>) -> Self {
        Self {
            id: id.into(),
            role: AxisRole::Descriptor,
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            expr: None,
            note: None,
            source: None,
            cells: IndexMap::new(),
        }
    }

    /// Builder method attaching a formatting preset.
    pub fn with_format(mut self, format: Option<FormatPreset>) -> Self {
        self.format = format;
        self
    }

    /// Builder method attaching a structural style.
    pub fn with_style(mut self, style: Option<SemanticStyle>) -> Self {
        self.style = style;
        self
    }

    /// Builder method attaching visual highlight emphasis.
    pub fn with_highlight(mut self, highlight: Option<bool>) -> Self {
        self.highlight = highlight;
        self
    }

    /// Builder method attaching display precision.
    pub fn with_decimals(mut self, decimals: Option<u32>) -> Self {
        self.decimals = decimals;
        self
    }

    /// Builder method attaching a formula expression.
    pub fn with_expr(mut self, expr: impl Into<String>) -> Self {
        self.expr = Some(expr.into());
        self
    }

    /// Builder method attaching an audit note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Builder method attaching cell inputs.
    pub fn with_cells(mut self, cells: IndexMap<ColId, CellInput>) -> Self {
        self.cells = cells;
        self
    }

    /// Builder method attaching provenance source metadata.
    pub fn with_source(mut self, source: Option<serde_json::Value>) -> Self {
        self.source = source;
        self
    }
}

impl Default for RowSpec {
    fn default() -> Self {
        Self::new("")
    }
}

/// Table specification schema for standalone 2D calculation tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSpec {
    pub id: TableId,
    pub title: String,
    pub columns: Vec<ColumnSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_groups: Vec<ColumnGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_groups: Vec<RowGroup>,
    pub rows: Vec<RowSpec>,
}

impl TableSpec {
    /// Returns an iterator over all declared column IDs.
    pub fn col_ids(&self) -> impl Iterator<Item = &ColId> {
        self.columns.iter().map(|c| &c.id)
    }

    /// Returns an iterator over all declared row IDs.
    pub fn row_ids(&self) -> impl Iterator<Item = &RowId> {
        self.rows.iter().map(|r| &r.id)
    }

    /// Returns true if the table contains a column with the given identifier.
    pub fn contains_col(&self, id: &ColId) -> bool {
        self.columns.iter().any(|c| &c.id == id)
    }

    /// Returns true if the table contains a row with the given identifier.
    pub fn contains_row(&self, id: &RowId) -> bool {
        self.rows.iter().any(|r| &r.id == id)
    }

    /// Returns the column specification for a column ID if found.
    pub fn get_column(&self, id: &ColId) -> Option<&ColumnSpec> {
        self.columns.iter().find(|c| &c.id == id)
    }

    /// Returns the row specification for a row ID if found.
    pub fn get_row(&self, id: &RowId) -> Option<&RowSpec> {
        self.rows.iter().find(|r| &r.id == id)
    }

    /// Validates all table-level domain invariants.
    pub fn validate(&self) -> Result<(), ModelError> {
        self.validate_columns()?;
        self.validate_column_groups()?;
        self.validate_rows()?;
        self.validate_row_groups()?;
        self.validate_cells()?;
        Ok(())
    }

    fn validate_columns(&self) -> Result<(), ModelError> {
        if self.columns.is_empty() {
            return Err(ModelError::EmptyColumns {
                table: self.id.clone(),
            });
        }

        let mut seen_cols = IndexSet::new();
        for col in &self.columns {
            if !seen_cols.insert(&col.id) {
                return Err(ModelError::DuplicateColumnId {
                    table: self.id.clone(),
                    column: col.id.clone(),
                });
            }

            if let (Some(format), Some(decimals)) = (col.format, col.decimals)
                && !format.is_numeric()
            {
                return Err(ModelError::InvalidPrecisionForPreset {
                    table: self.id.clone(),
                    context: format!("column '{}'", col.id),
                    preset: format,
                    decimals,
                });
            }
        }
        Ok(())
    }

    fn validate_column_groups(&self) -> Result<(), ModelError> {
        let mut seen_groups = IndexSet::new();
        let mut assigned_columns: IndexMap<&ColId, &ColGroupId> = IndexMap::new();

        for group in &self.column_groups {
            if !seen_groups.insert(&group.id) {
                return Err(ModelError::DuplicateColumnGroupId {
                    table: self.id.clone(),
                    group: group.id.clone(),
                });
            }

            if group.columns.is_empty() {
                return Err(ModelError::NonContiguousColumnGroup {
                    table: self.id.clone(),
                    group: group.id.clone(),
                });
            }

            // Verify contiguity within table.columns
            let mut prev_table_idx: Option<usize> = None;
            for col in &group.columns {
                let table_idx =
                    self.columns
                        .iter()
                        .position(|c| &c.id == col)
                        .ok_or_else(|| ModelError::UnknownColumn {
                            table: self.id.clone(),
                            column: col.clone(),
                            context: format!("column group '{}'", group.id),
                        })?;

                if let Some(prev) = prev_table_idx
                    && table_idx != prev + 1
                {
                    return Err(ModelError::NonContiguousColumnGroup {
                        table: self.id.clone(),
                        group: group.id.clone(),
                    });
                }
                prev_table_idx = Some(table_idx);

                // Verify disjointness
                if let Some(existing_group) = assigned_columns.insert(col, &group.id) {
                    return Err(ModelError::OverlappingColumnGroup {
                        table: self.id.clone(),
                        column: col.clone(),
                        first_group: (*existing_group).clone(),
                        second_group: group.id.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    fn validate_rows(&self) -> Result<(), ModelError> {
        if self.rows.is_empty() {
            return Err(ModelError::EmptyRows {
                table: self.id.clone(),
            });
        }

        let mut seen_rows = IndexSet::new();
        for row in &self.rows {
            if !seen_rows.insert(&row.id) {
                return Err(ModelError::DuplicateRowId {
                    table: self.id.clone(),
                    row: row.id.clone(),
                });
            }

            if let (Some(format), Some(decimals)) = (row.format, row.decimals)
                && !format.is_numeric()
            {
                return Err(ModelError::InvalidPrecisionForPreset {
                    table: self.id.clone(),
                    context: format!("row '{}'", row.id),
                    preset: format,
                    decimals,
                });
            }
        }
        Ok(())
    }

    fn validate_row_groups(&self) -> Result<(), ModelError> {
        let mut seen_group_ids = IndexSet::new();
        let mut assigned_rows: IndexMap<RowId, RowGroupId> = IndexMap::new();

        let row_indices: IndexMap<&RowId, usize> = self
            .rows
            .iter()
            .enumerate()
            .map(|(idx, r)| (&r.id, idx))
            .collect();

        for root_group in &self.row_groups {
            self.validate_row_group_recursive(
                root_group,
                &mut seen_group_ids,
                &mut assigned_rows,
                &row_indices,
            )?;
        }

        Ok(())
    }

    fn validate_row_group_recursive(
        &self,
        group: &RowGroup,
        seen_group_ids: &mut IndexSet<RowGroupId>,
        assigned_rows: &mut IndexMap<RowId, RowGroupId>,
        row_indices: &IndexMap<&RowId, usize>,
    ) -> Result<(), ModelError> {
        if !seen_group_ids.insert(group.id.clone()) {
            return Err(ModelError::DuplicateRowGroupId {
                table: self.id.clone(),
                group: group.id.clone(),
            });
        }

        let mut descendant_rows = Vec::new();
        let mut cycle_tracker = IndexSet::new();
        Self::collect_descendant_rows(group, &self.id, &mut cycle_tracker, &mut descendant_rows)?;

        if descendant_rows.is_empty() {
            return Err(ModelError::NonContiguousRowGroup {
                table: self.id.clone(),
                group: group.id.clone(),
            });
        }

        // Verify descendant row contiguity
        let first_row = &descendant_rows[0];
        let start_idx =
            row_indices
                .get(first_row)
                .copied()
                .ok_or_else(|| ModelError::UnknownRow {
                    table: self.id.clone(),
                    row: first_row.clone(),
                    context: format!("row group '{}'", group.id),
                })?;

        for (offset, row_id) in descendant_rows.iter().enumerate() {
            let actual_idx =
                row_indices
                    .get(row_id)
                    .copied()
                    .ok_or_else(|| ModelError::UnknownRow {
                        table: self.id.clone(),
                        row: row_id.clone(),
                        context: format!("row group '{}'", group.id),
                    })?;

            if actual_idx != start_idx + offset {
                return Err(ModelError::NonContiguousRowGroup {
                    table: self.id.clone(),
                    group: group.id.clone(),
                });
            }
        }

        // For direct children that are rows, register membership and check overlap
        for child in &group.children {
            match child {
                RowGroupChild::Row(row_id) => {
                    if let Some(existing_group) =
                        assigned_rows.insert(row_id.clone(), group.id.clone())
                    {
                        return Err(ModelError::OverlappingRowGroup {
                            table: self.id.clone(),
                            row: row_id.clone(),
                            first_group: existing_group,
                            second_group: group.id.clone(),
                        });
                    }
                }
                RowGroupChild::Group(subgroup) => {
                    self.validate_row_group_recursive(
                        subgroup,
                        seen_group_ids,
                        assigned_rows,
                        row_indices,
                    )?;
                }
            }
        }

        Ok(())
    }

    fn collect_descendant_rows<'a>(
        group: &'a RowGroup,
        table_id: &TableId,
        cycle_tracker: &mut IndexSet<&'a RowGroupId>,
        out: &mut Vec<RowId>,
    ) -> Result<(), ModelError> {
        if !cycle_tracker.insert(&group.id) {
            return Err(ModelError::CyclicRowGroup {
                table: table_id.clone(),
                group: group.id.clone(),
            });
        }

        for child in &group.children {
            match child {
                RowGroupChild::Row(row_id) => out.push(row_id.clone()),
                RowGroupChild::Group(subgroup) => {
                    Self::collect_descendant_rows(subgroup, table_id, cycle_tracker, out)?;
                }
            }
        }

        cycle_tracker.shift_remove(&group.id);
        Ok(())
    }

    fn validate_cells(&self) -> Result<(), ModelError> {
        for row in &self.rows {
            for (col_id, cell_input) in &row.cells {
                if !self.contains_col(col_id) {
                    return Err(ModelError::UnknownCellColumn {
                        table: self.id.clone(),
                        row: row.id.clone(),
                        column: col_id.clone(),
                    });
                }

                if let CellInput::Detailed(d) = cell_input {
                    let has_value = d.value.is_some();
                    let has_expr = d.expr.is_some();
                    if has_value && has_expr {
                        return Err(ModelError::MutuallyExclusiveCellPayload {
                            table: self.id.clone(),
                            row: row.id.clone(),
                            column: col_id.clone(),
                        });
                    }
                    if !has_value && !has_expr {
                        return Err(ModelError::EmptyCellPayload {
                            table: self.id.clone(),
                            row: row.id.clone(),
                            column: col_id.clone(),
                        });
                    }
                    if let (Some(format), Some(decimals)) = (d.format, d.decimals)
                        && !format.is_numeric()
                    {
                        return Err(ModelError::InvalidPrecisionForPreset {
                            table: self.id.clone(),
                            context: format!("row '{}', column '{}' cell", row.id, col_id),
                            preset: format,
                            decimals,
                        });
                    }
                }
            }
        }
        Ok(())
    }
}
