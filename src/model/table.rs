use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::ModelError;
use super::format::{CurrencyCode, FormatPreset, SemanticStyle};
use super::id::{ColId, RowId, TableId};

/// Calculation axis role: measures broadcast formulas; descriptors isolate labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum AxisRole {
    #[default]
    Measure,
    Descriptor,
    PrimaryDescriptor,
}

impl AxisRole {
    /// Returns true if this axis is a measure.
    pub fn is_measure(&self) -> bool {
        matches!(self, Self::Measure)
    }

    /// Returns true if this axis is a descriptor (primary or secondary).
    pub fn is_descriptor(&self) -> bool {
        matches!(self, Self::Descriptor | Self::PrimaryDescriptor)
    }

    /// Returns true if this axis is the primary descriptor.
    pub fn is_primary_descriptor(&self) -> bool {
        matches!(self, Self::PrimaryDescriptor)
    }
}

/// Column definition within a table specification: either a calculable data column or a blank spacing gutter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnSpec {
    Data(DataColumnSpec),
    Gutter,
}

impl ColumnSpec {
    /// Creates a measure column specification with default attributes.
    pub fn new(id: impl Into<ColId>) -> Self {
        Self::Data(DataColumnSpec::new(id))
    }

    /// Creates a descriptor column specification with an explicit display label.
    pub fn descriptor(id: impl Into<ColId>, label: impl Into<String>) -> Self {
        Self::Data(DataColumnSpec::descriptor(id, label))
    }

    /// Creates a primary descriptor column specification.
    pub fn primary_descriptor(id: impl Into<ColId>, label: impl Into<String>) -> Self {
        Self::Data(DataColumnSpec::primary_descriptor(id, label))
    }

    /// Creates a spacing gutter column specification.
    pub fn gutter() -> Self {
        Self::Gutter
    }

    /// Returns the column identifier if this is a data column.
    pub fn id(&self) -> Option<&ColId> {
        match self {
            Self::Data(d) => Some(&d.id),
            Self::Gutter => None,
        }
    }

    /// Returns the axis role if this is a data column.
    pub fn role(&self) -> Option<AxisRole> {
        match self {
            Self::Data(d) => Some(d.role),
            Self::Gutter => None,
        }
    }

    /// Returns the declared header hierarchy if this is a data column.
    pub fn headers(&self) -> Option<&[String]> {
        match self {
            Self::Data(d) => Some(&d.headers),
            Self::Gutter => None,
        }
    }

    /// Returns the bottom-tier metric header label if declared, or None if the column has no headers.
    pub fn bottom_header(&self) -> Option<&str> {
        match self {
            Self::Data(d) => d.bottom_header(),
            Self::Gutter => None,
        }
    }

    /// Builder method attaching a single header label.
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        if let Self::Data(d) = &mut self {
            d.headers.push(header.into());
        }
        self
    }

    /// Builder method attaching multi-tier header labels.
    pub fn with_headers(mut self, headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        if let Self::Data(d) = &mut self {
            d.headers = headers.into_iter().map(Into::into).collect();
        }
        self
    }

    /// Builder method attaching a formatting preset.
    pub fn with_format(mut self, format: Option<FormatPreset>) -> Self {
        if let Self::Data(d) = &mut self {
            d.format = format;
        }
        self
    }

    /// Builder method attaching a structural style.
    pub fn with_style(mut self, style: Option<SemanticStyle>) -> Self {
        if let Self::Data(d) = &mut self {
            d.style = style;
        }
        self
    }

    /// Builder method attaching visual highlight emphasis.
    pub fn with_highlight(mut self, highlight: Option<bool>) -> Self {
        if let Self::Data(d) = &mut self {
            d.highlight = highlight;
        }
        self
    }

    /// Builder method attaching display precision.
    pub fn with_decimals(mut self, decimals: Option<u32>) -> Self {
        if let Self::Data(d) = &mut self {
            d.decimals = decimals;
        }
        self
    }

    /// Builder method attaching a currency denomination.
    pub fn with_currency(mut self, currency: Option<CurrencyCode>) -> Self {
        if let Self::Data(d) = &mut self {
            d.currency = currency;
        }
        self
    }

    /// Builder method attaching a formula expression.
    pub fn with_expr(mut self, expr: impl Into<String>) -> Self {
        if let Self::Data(d) = &mut self {
            d.expr = Some(expr.into());
        }
        self
    }

    /// Builder method attaching an audit note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        if let Self::Data(d) = &mut self {
            d.note = Some(note.into());
        }
        self
    }

    /// Builder method attaching provenance source metadata.
    pub fn with_source(mut self, source: Option<serde_json::Value>) -> Self {
        if let Self::Data(d) = &mut self {
            d.source = source;
        }
        self
    }
}

impl From<DataColumnSpec> for ColumnSpec {
    fn from(d: DataColumnSpec) -> Self {
        Self::Data(d)
    }
}

/// Calculable data column definition within a table specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataColumnSpec {
    pub id: ColId,
    #[serde(default)]
    pub role: AxisRole,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub headers: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<SemanticStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
}

impl DataColumnSpec {
    /// Creates a measure column specification with default attributes and empty headers.
    pub fn new(id: impl Into<ColId>) -> Self {
        Self {
            id: id.into(),
            role: AxisRole::Measure,
            headers: Vec::new(),
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            currency: None,
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
            headers: vec![label.into()],
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            currency: None,
            expr: None,
            note: None,
            source: None,
        }
    }

    /// Creates a primary descriptor column specification.
    pub fn primary_descriptor(id: impl Into<ColId>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            role: AxisRole::PrimaryDescriptor,
            headers: vec![label.into()],
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            currency: None,
            expr: None,
            note: None,
            source: None,
        }
    }

    /// Returns the bottom-tier metric header label if declared, or None if the column has no headers.
    pub fn bottom_header(&self) -> Option<&str> {
        self.headers.last().map(|s| s.as_str())
    }

    /// Builder method attaching a single header label.
    pub fn with_header(mut self, header: impl Into<String>) -> Self {
        self.headers.push(header.into());
        self
    }

    /// Builder method attaching multi-tier header labels.
    pub fn with_headers(mut self, headers: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.headers = headers.into_iter().map(Into::into).collect();
        self
    }

    /// Builder method setting the axis role.
    pub fn with_role(mut self, role: AxisRole) -> Self {
        self.role = role;
        self
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

    /// Builder method attaching a currency denomination.
    pub fn with_currency(mut self, currency: Option<CurrencyCode>) -> Self {
        self.currency = currency;
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
    pub currency: Option<CurrencyCode>,
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

    /// Returns the currency override if present.
    pub fn currency(&self) -> Option<CurrencyCode> {
        match self {
            Self::Literal(_) => None,
            Self::Detailed(d) => d.currency,
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

/// Row definition within a table specification: calculable data row, visual section header, or blank spacing gutter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RowSpec {
    Data(DataRowSpec),
    SectionHeader(SectionHeaderRowSpec),
    Gutter,
}

impl RowSpec {
    /// Constructs a calculable data row specification.
    pub fn new(id: impl Into<RowId>) -> Self {
        Self::Data(DataRowSpec::new(id))
    }

    /// Constructs a visual section header row specification.
    pub fn section_header(title: impl Into<String>) -> Self {
        Self::SectionHeader(SectionHeaderRowSpec::new(title))
    }

    /// Constructs a spacing gutter row specification.
    pub fn gutter() -> Self {
        Self::Gutter
    }

    /// Returns the row identifier if this is a data row.
    pub fn id(&self) -> Option<&RowId> {
        match self {
            Self::Data(d) => Some(&d.id),
            Self::SectionHeader(_) | Self::Gutter => None,
        }
    }

    /// Returns the typographic cell indentation if specified.
    pub fn indent(&self) -> Option<u8> {
        match self {
            Self::Data(d) => d.indent,
            Self::SectionHeader(h) => h.indent,
            Self::Gutter => None,
        }
    }

    /// Builder method attaching indentation.
    pub fn with_indent(mut self, indent: Option<u8>) -> Self {
        match &mut self {
            Self::Data(d) => d.indent = indent,
            Self::SectionHeader(h) => h.indent = indent,
            Self::Gutter => {}
        }
        self
    }

    /// Builder method attaching a formatting preset.
    pub fn with_format(mut self, format: Option<FormatPreset>) -> Self {
        if let Self::Data(d) = &mut self {
            d.format = format;
        }
        self
    }

    /// Builder method attaching a structural style.
    pub fn with_style(mut self, style: Option<SemanticStyle>) -> Self {
        if let Self::Data(d) = &mut self {
            d.style = style;
        }
        self
    }

    /// Builder method attaching visual highlight emphasis.
    pub fn with_highlight(mut self, highlight: Option<bool>) -> Self {
        if let Self::Data(d) = &mut self {
            d.highlight = highlight;
        }
        self
    }

    /// Builder method attaching display precision.
    pub fn with_decimals(mut self, decimals: Option<u32>) -> Self {
        if let Self::Data(d) = &mut self {
            d.decimals = decimals;
        }
        self
    }

    /// Builder method attaching a currency denomination.
    pub fn with_currency(mut self, currency: Option<CurrencyCode>) -> Self {
        if let Self::Data(d) = &mut self {
            d.currency = currency;
        }
        self
    }

    /// Builder method attaching a formula expression.
    pub fn with_expr(mut self, expr: impl Into<String>) -> Self {
        if let Self::Data(d) = &mut self {
            d.expr = Some(expr.into());
        }
        self
    }

    /// Builder method attaching an audit note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        match &mut self {
            Self::Data(d) => d.note = Some(note.into()),
            Self::SectionHeader(h) => h.note = Some(note.into()),
            Self::Gutter => {}
        }
        self
    }

    /// Builder method attaching cell inputs.
    pub fn with_cells(mut self, cells: IndexMap<ColId, CellInput>) -> Self {
        if let Self::Data(d) = &mut self {
            d.cells = cells;
        }
        self
    }

    /// Builder method attaching a single cell input.
    pub fn with_cell(mut self, col: impl Into<ColId>, cell: impl Into<CellInput>) -> Self {
        if let Self::Data(d) = &mut self {
            d.cells.insert(col.into(), cell.into());
        }
        self
    }

    /// Builder method attaching provenance source metadata.
    pub fn with_source(mut self, source: Option<serde_json::Value>) -> Self {
        if let Self::Data(d) = &mut self {
            d.source = source;
        }
        self
    }
}

impl From<DataRowSpec> for RowSpec {
    fn from(d: DataRowSpec) -> Self {
        Self::Data(d)
    }
}

impl From<SectionHeaderRowSpec> for RowSpec {
    fn from(h: SectionHeaderRowSpec) -> Self {
        Self::SectionHeader(h)
    }
}

/// Calculable data row definition within a table specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DataRowSpec {
    pub id: RowId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<SemanticStyle>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highlight: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<FormatPreset>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub cells: IndexMap<ColId, CellInput>,
}

impl DataRowSpec {
    /// Creates a data row specification with default attributes.
    pub fn new(id: impl Into<RowId>) -> Self {
        Self {
            id: id.into(),
            indent: None,
            style: None,
            highlight: None,
            format: None,
            decimals: None,
            currency: None,
            expr: None,
            note: None,
            source: None,
            cells: IndexMap::new(),
        }
    }

    /// Builder method attaching typographic indentation.
    pub fn with_indent(mut self, indent: Option<u8>) -> Self {
        self.indent = indent;
        self
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

    /// Builder method attaching a currency denomination.
    pub fn with_currency(mut self, currency: Option<CurrencyCode>) -> Self {
        self.currency = currency;
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

    /// Builder method attaching a single cell input.
    pub fn with_cell(mut self, col: impl Into<ColId>, cell: impl Into<CellInput>) -> Self {
        self.cells.insert(col.into(), cell.into());
        self
    }

    /// Builder method attaching provenance source metadata.
    pub fn with_source(mut self, source: Option<serde_json::Value>) -> Self {
        self.source = source;
        self
    }
}

/// Visual section header row specification without calculable cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SectionHeaderRowSpec {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl SectionHeaderRowSpec {
    /// Creates a new section header row specification.
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            indent: None,
            note: None,
        }
    }

    /// Builder method attaching typographic indentation.
    pub fn with_indent(mut self, indent: Option<u8>) -> Self {
        self.indent = indent;
        self
    }

    /// Builder method attaching an audit note.
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }
}

impl Default for TableSpec {
    fn default() -> Self {
        Self::new("", "")
    }
}

/// Table specification schema for standalone 2D calculation tables.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableSpec {
    pub id: TableId,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<CurrencyCode>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub collapsible: bool,
    pub columns: Vec<ColumnSpec>,
    pub rows: Vec<RowSpec>,
}

impl TableSpec {
    /// Creates a table specification with empty columns and rows.
    pub fn new(id: impl Into<TableId>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            currency: None,
            collapsible: false,
            columns: Vec::new(),
            rows: Vec::new(),
        }
    }

    /// Builder method attaching a subtitle banner.
    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    /// Builder method setting whether the table is collapsible into its title banner.
    pub fn with_collapsible(mut self, collapsible: bool) -> Self {
        self.collapsible = collapsible;
        self
    }

    /// Builder method attaching default table currency.
    pub fn with_currency(mut self, currency: Option<CurrencyCode>) -> Self {
        self.currency = currency;
        self
    }

    /// Returns an iterator over all declared data column IDs.
    pub fn col_ids(&self) -> impl Iterator<Item = &ColId> {
        self.columns.iter().filter_map(|c| c.id())
    }

    /// Returns an iterator over all declared data row IDs.
    pub fn row_ids(&self) -> impl Iterator<Item = &RowId> {
        self.rows.iter().filter_map(|r| r.id())
    }

    /// Returns true if the table contains a data column with the given identifier.
    pub fn contains_col(&self, id: &ColId) -> bool {
        self.columns.iter().any(|c| c.id() == Some(id))
    }

    /// Returns true if the table contains a data row with the given identifier.
    pub fn contains_row(&self, id: &RowId) -> bool {
        self.rows.iter().any(|r| r.id() == Some(id))
    }

    /// Returns the column specification for a column ID if found.
    pub fn get_column(&self, id: &ColId) -> Option<&DataColumnSpec> {
        self.columns.iter().find_map(|c| match c {
            ColumnSpec::Data(d) if &d.id == id => Some(d),
            _ => None,
        })
    }

    /// Returns the row specification for a row ID if found.
    pub fn get_row(&self, id: &RowId) -> Option<&DataRowSpec> {
        self.rows.iter().find_map(|r| match r {
            RowSpec::Data(d) if &d.id == id => Some(d),
            _ => None,
        })
    }

    /// Validates all table-level domain invariants.
    pub fn validate(&self) -> Result<(), ModelError> {
        if let Some(subtitle) = &self.subtitle
            && subtitle.trim().is_empty()
        {
            return Err(ModelError::EmptyTableSubtitle {
                table: self.id.clone(),
            });
        }

        self.validate_columns()?;
        self.validate_rows()?;
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
        let mut primary_desc: Option<&ColId> = None;

        for col in &self.columns {
            let data_col = match col {
                ColumnSpec::Data(d) => d,
                ColumnSpec::Gutter => continue,
            };

            if !seen_cols.insert(&data_col.id) {
                return Err(ModelError::DuplicateColumnId {
                    table: self.id.clone(),
                    column: data_col.id.clone(),
                });
            }

            if data_col.role.is_primary_descriptor() {
                if let Some(first) = primary_desc {
                    return Err(ModelError::DuplicatePrimaryDescriptor {
                        table: self.id.clone(),
                        first: first.clone(),
                        second: data_col.id.clone(),
                    });
                }
                primary_desc = Some(&data_col.id);
            }

            if let Some(decimals) = data_col.decimals
                && decimals > 14
            {
                return Err(ModelError::PrecisionOutOfBounds {
                    table: self.id.clone(),
                    context: format!("column '{}'", data_col.id),
                    decimals,
                    max: 14,
                });
            }

            if let (Some(format), Some(currency)) = (data_col.format, data_col.currency)
                && format != FormatPreset::Currency
            {
                return Err(ModelError::InvalidCurrencyForPreset {
                    table: self.id.clone(),
                    context: format!("column '{}'", data_col.id),
                    preset: format,
                    currency,
                });
            }

            if let (Some(format), Some(decimals)) = (data_col.format, data_col.decimals)
                && !format.is_numeric()
            {
                return Err(ModelError::InvalidPrecisionForPreset {
                    table: self.id.clone(),
                    context: format!("column '{}'", data_col.id),
                    preset: format,
                    decimals,
                });
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

        let has_primary_desc = self
            .columns
            .iter()
            .any(|c| c.role().is_some_and(|r| r.is_primary_descriptor()));

        let mut seen_rows = IndexSet::new();
        for row in &self.rows {
            if let Some(indent) = row.indent()
                && indent > 0
                && !has_primary_desc
            {
                return Err(ModelError::IndentWithoutPrimaryDescriptor {
                    table: self.id.clone(),
                    row: row.id().cloned(),
                    indent,
                });
            }

            let data_row = match row {
                RowSpec::Data(d) => d,
                RowSpec::SectionHeader(_) | RowSpec::Gutter => continue,
            };

            if !seen_rows.insert(&data_row.id) {
                return Err(ModelError::DuplicateRowId {
                    table: self.id.clone(),
                    row: data_row.id.clone(),
                });
            }

            if let Some(decimals) = data_row.decimals
                && decimals > 14
            {
                return Err(ModelError::PrecisionOutOfBounds {
                    table: self.id.clone(),
                    context: format!("row '{}'", data_row.id),
                    decimals,
                    max: 14,
                });
            }

            if let (Some(format), Some(currency)) = (data_row.format, data_row.currency)
                && format != FormatPreset::Currency
            {
                return Err(ModelError::InvalidCurrencyForPreset {
                    table: self.id.clone(),
                    context: format!("row '{}'", data_row.id),
                    preset: format,
                    currency,
                });
            }

            if let (Some(format), Some(decimals)) = (data_row.format, data_row.decimals)
                && !format.is_numeric()
            {
                return Err(ModelError::InvalidPrecisionForPreset {
                    table: self.id.clone(),
                    context: format!("row '{}'", data_row.id),
                    preset: format,
                    decimals,
                });
            }
        }
        Ok(())
    }

    fn validate_cells(&self) -> Result<(), ModelError> {
        for row in &self.rows {
            let data_row = match row {
                RowSpec::Data(d) => d,
                RowSpec::SectionHeader(_) | RowSpec::Gutter => continue,
            };

            for (col_id, cell_input) in &data_row.cells {
                if !self.contains_col(col_id) {
                    return Err(ModelError::UnknownCellColumn {
                        table: self.id.clone(),
                        row: data_row.id.clone(),
                        column: col_id.clone(),
                    });
                }

                if let CellInput::Detailed(d) = cell_input {
                    let has_value = d.value.is_some();
                    let has_expr = d.expr.is_some();
                    if has_value && has_expr {
                        return Err(ModelError::MutuallyExclusiveCellPayload {
                            table: self.id.clone(),
                            row: data_row.id.clone(),
                            column: col_id.clone(),
                        });
                    }
                    if !has_value && !has_expr {
                        return Err(ModelError::EmptyCellPayload {
                            table: self.id.clone(),
                            row: data_row.id.clone(),
                            column: col_id.clone(),
                        });
                    }
                    if let Some(decimals) = d.decimals
                        && decimals > 14
                    {
                        return Err(ModelError::PrecisionOutOfBounds {
                            table: self.id.clone(),
                            context: format!("row '{}', column '{}' cell", data_row.id, col_id),
                            decimals,
                            max: 14,
                        });
                    }
                    if let (Some(format), Some(currency)) = (d.format, d.currency)
                        && format != FormatPreset::Currency
                    {
                        return Err(ModelError::InvalidCurrencyForPreset {
                            table: self.id.clone(),
                            context: format!("row '{}', column '{}' cell", data_row.id, col_id),
                            preset: format,
                            currency,
                        });
                    }
                    if let (Some(format), Some(decimals)) = (d.format, d.decimals)
                        && !format.is_numeric()
                    {
                        return Err(ModelError::InvalidPrecisionForPreset {
                            table: self.id.clone(),
                            context: format!("row '{}', column '{}' cell", data_row.id, col_id),
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
