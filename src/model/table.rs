use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::ModelError;
use super::format::FormatPreset;
use super::id::{ColGroupId, ColId, RowGroupId, RowId, TableId};

/// Table specification schema for standalone calculation tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSpec {
    pub id: TableId,
    pub title: String,
    pub columns: Vec<ColId>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub column_groups: Vec<ColumnGroup>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub row_groups: Vec<RowGroup>,
    pub rows: Vec<RowSpec>,
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

/// Explicit literal cell value: number, string, or null.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueLiteral {
    Number(f64),
    String(String),
    Null,
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
                formatter.write_str("a number, string, or null")
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
            Self::Null => serializer.serialize_unit(),
        }
    }
}

/// Calculation specification for a single match arm partition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchArmSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<ValueLiteral>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
}

/// Row definition within a table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RowSpec {
    pub id: RowId,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Vec<ValueLiteral>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(rename = "match", default, skip_serializing_if = "Option::is_none")]
    pub match_arms: Option<IndexMap<String, MatchArmSpec>>,
}

/// Parsed match partition selector key.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MatchSelector {
    /// Single column ID, e.g. `"2024E"`
    Single(ColId),
    /// Contiguous column range, e.g. `"2021A..2023A"`
    Range { start: ColId, end: ColId },
    /// Named column group, e.g. `"cols.forecast"`
    Group(ColGroupId),
}

impl MatchSelector {
    /// Parses a match selector string into a typed selector.
    pub fn parse(selector: &str) -> Result<Self, String> {
        let trimmed = selector.trim();
        if trimmed.is_empty() {
            return Err("selector cannot be empty".to_string());
        }

        if let Some(group_name) = trimmed.strip_prefix("cols.") {
            if group_name.is_empty() {
                return Err("column group identifier cannot be empty".to_string());
            }
            return Ok(Self::Group(ColGroupId::from(group_name)));
        }

        if let Some((start, end)) = trimmed.split_once("..") {
            let start = start.trim();
            let end = end.trim();
            if start.is_empty() || end.is_empty() {
                return Err(format!("invalid range selector: '{trimmed}'"));
            }
            return Ok(Self::Range {
                start: ColId::from(start),
                end: ColId::from(end),
            });
        }

        Ok(Self::Single(ColId::from(trimmed)))
    }

    /// Resolves selector against table columns and declared column groups into a contiguous slice of ColIds.
    pub fn resolve_columns(&self, table: &TableSpec) -> Result<Vec<ColId>, ModelError> {
        match self {
            Self::Single(col) => {
                if !table.columns.contains(col) {
                    return Err(ModelError::UnknownColumn {
                        table: table.id.clone(),
                        column: col.clone(),
                        context: format!("match selector '{col}'"),
                    });
                }
                Ok(vec![col.clone()])
            }
            Self::Range { start, end } => {
                let start_idx = table
                    .columns
                    .iter()
                    .position(|c| c == start)
                    .ok_or_else(|| ModelError::UnknownColumn {
                        table: table.id.clone(),
                        column: start.clone(),
                        context: format!("match range start '{start}'"),
                    })?;

                let end_idx = table.columns.iter().position(|c| c == end).ok_or_else(|| {
                    ModelError::UnknownColumn {
                        table: table.id.clone(),
                        column: end.clone(),
                        context: format!("match range end '{end}'"),
                    }
                })?;

                if start_idx > end_idx {
                    return Err(ModelError::InvalidMatchSelector {
                        table: table.id.clone(),
                        row: RowId::from(""),
                        selector: format!("{start}..{end}"),
                        reason: format!(
                            "start column '{start}' (index {start_idx}) must precede or equal end column '{end}' (index {end_idx})"
                        ),
                    });
                }

                Ok(table.columns[start_idx..=end_idx].to_vec())
            }
            Self::Group(group_id) => {
                let group = table
                    .column_groups
                    .iter()
                    .find(|g| &g.id == group_id)
                    .ok_or_else(|| ModelError::UnknownColumnGroup {
                        table: table.id.clone(),
                        group: group_id.clone(),
                    })?;

                Ok(group.columns.clone())
            }
        }
    }
}

impl TableSpec {
    /// Validates all table-level domain invariants.
    pub fn validate(&self) -> Result<(), ModelError> {
        self.validate_columns()?;
        self.validate_column_groups()?;
        self.validate_rows()?;
        self.validate_row_groups()?;
        self.validate_row_payloads()?;
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
            if !seen_cols.insert(col) {
                return Err(ModelError::DuplicateColumnId {
                    table: self.id.clone(),
                    column: col.clone(),
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
                let table_idx = self.columns.iter().position(|c| c == col).ok_or_else(|| {
                    ModelError::UnknownColumn {
                        table: self.id.clone(),
                        column: col.clone(),
                        context: format!("column group '{}'", group.id),
                    }
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

    fn validate_row_payloads(&self) -> Result<(), ModelError> {
        for row in &self.rows {
            let payload_count = (row.value.is_some() as usize)
                + (row.expr.is_some() as usize)
                + (row.match_arms.is_some() as usize);

            if payload_count != 1 {
                return Err(ModelError::MutuallyExclusivePayload {
                    table: self.id.clone(),
                    row: row.id.clone(),
                });
            }

            if let Some(values) = &row.value
                && values.len() != self.columns.len()
            {
                return Err(ModelError::DimensionMismatch {
                    table: self.id.clone(),
                    row: row.id.clone(),
                    expected: self.columns.len(),
                    actual: values.len(),
                });
            }

            if let Some(arms) = &row.match_arms {
                if arms.is_empty() {
                    return Err(ModelError::UncoveredColumns {
                        table: self.id.clone(),
                        row: row.id.clone(),
                        missing: self.columns.clone(),
                    });
                }

                let mut matched_columns = IndexMap::new();

                for (selector_str, arm) in arms {
                    let arm_payload_count =
                        (arm.value.is_some() as usize) + (arm.expr.is_some() as usize);
                    if arm_payload_count != 1 {
                        return Err(ModelError::MutuallyExclusiveArmPayload {
                            table: self.id.clone(),
                            row: row.id.clone(),
                            arm: selector_str.clone(),
                        });
                    }

                    let selector = MatchSelector::parse(selector_str).map_err(|reason| {
                        ModelError::InvalidMatchSelector {
                            table: self.id.clone(),
                            row: row.id.clone(),
                            selector: selector_str.clone(),
                            reason,
                        }
                    })?;

                    let target_columns = selector.resolve_columns(self).map_err(|err| {
                        if let ModelError::InvalidMatchSelector {
                            table,
                            selector,
                            reason,
                            ..
                        } = err
                        {
                            ModelError::InvalidMatchSelector {
                                table,
                                row: row.id.clone(),
                                selector,
                                reason,
                            }
                        } else {
                            err
                        }
                    })?;

                    if let Some(values) = &arm.value
                        && values.len() != target_columns.len()
                    {
                        return Err(ModelError::DimensionMismatch {
                            table: self.id.clone(),
                            row: row.id.clone(),
                            expected: target_columns.len(),
                            actual: values.len(),
                        });
                    }

                    for col in target_columns {
                        if matched_columns
                            .insert(col.clone(), selector_str.clone())
                            .is_some()
                        {
                            return Err(ModelError::OverlappingMatchArm {
                                table: self.id.clone(),
                                row: row.id.clone(),
                                column: col,
                            });
                        }
                    }
                }

                if matched_columns.len() != self.columns.len() {
                    let missing: Vec<ColId> = self
                        .columns
                        .iter()
                        .filter(|c| !matched_columns.contains_key(*c))
                        .cloned()
                        .collect();

                    return Err(ModelError::UncoveredColumns {
                        table: self.id.clone(),
                        row: row.id.clone(),
                        missing,
                    });
                }
            }
        }

        Ok(())
    }
}
