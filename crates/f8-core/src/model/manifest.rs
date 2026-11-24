use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::id::TableId;
use super::theme::{Theme, ThemeSpec, is_valid_hex_color};

/// Maximum column count supported by OpenXML / Excel (columns 0..=16,383, i.e. A..XFD).
pub const MAX_EXCEL_COLUMNS: u32 = 16_384;

/// Maximum row count supported by OpenXML / Excel (rows 0..=1,048,575, i.e. 1..1,048,576).
pub const MAX_EXCEL_ROWS: u32 = 1_048_576;

/// Minimum allowed uniform column width on a `grid` worksheet (Excel character units).
pub const MIN_GRID_COL_WIDTH: f64 = 1.0;

/// Maximum allowed uniform column width on a `grid` worksheet (Excel 255-char limit).
pub const MAX_GRID_COL_WIDTH: f64 = 255.0;

fn validate_sheet_name(name: &str) -> Result<(), ModelError> {
    if name.is_empty() {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not be empty",
        });
    }
    if name.encode_utf16().count() > 31 {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not exceed 31 UTF-16 code units",
        });
    }
    if name.starts_with('\'') || name.ends_with('\'') {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not start or end with a single quote",
        });
    }
    if name
        .chars()
        .any(|c| matches!(c, ':' | '\\' | '/' | '?' | '*' | '[' | ']'))
    {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name must not contain ':', '\\', '/', '?', '*', '[', or ']'",
        });
    }
    if name.eq_ignore_ascii_case("history") {
        return Err(ModelError::InvalidSheetName {
            sheet: name.to_string(),
            reason: "sheet name 'History' is reserved by Excel",
        });
    }
    Ok(())
}

/// Root workbook manifest defining model metadata, theme, and sheet ordering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub model_name: String,
    #[serde(default)]
    pub theme: ThemeSpec,
    pub sheets: Vec<SheetSpec>,
}

impl ProjectManifest {
    /// Validates sheet naming uniqueness and structural invariants.
    pub fn validate(&self) -> Result<(), ModelError> {
        if self.sheets.is_empty() {
            return Err(ModelError::EmptySheets);
        }
        let mut seen_sheets = IndexSet::new();
        for sheet in &self.sheets {
            validate_sheet_name(&sheet.name)?;
            if !seen_sheets.insert(sheet.name.to_lowercase()) {
                return Err(ModelError::DuplicateSheetName {
                    sheet: sheet.name.clone(),
                });
            }
            if let Some(color) = &sheet.tab_color
                && !is_valid_hex_color(color)
            {
                return Err(ModelError::InvalidSheetTabColor {
                    sheet: sheet.name.clone(),
                    color: color.clone(),
                });
            }
            if let Some(freeze) = sheet.freeze_panes
                && (freeze.row >= MAX_EXCEL_ROWS || freeze.col >= MAX_EXCEL_COLUMNS)
            {
                return Err(ModelError::InvalidFreezePanes {
                    sheet: sheet.name.clone(),
                    row: freeze.row,
                    col: freeze.col,
                    max_rows: MAX_EXCEL_ROWS,
                    max_cols: MAX_EXCEL_COLUMNS,
                });
            }
            match sheet.layout {
                SheetLayoutSpec::Auto | SheetLayoutSpec::Grid { col_width: None } => {}
                SheetLayoutSpec::Grid { col_width: Some(w) } => {
                    if !w.is_finite() || !(MIN_GRID_COL_WIDTH..=MAX_GRID_COL_WIDTH).contains(&w) {
                        return Err(ModelError::InvalidSheetColWidth {
                            sheet: sheet.name.clone(),
                            col_width: w,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Resolves and validates the workbook theme from a built-in name or inline custom theme specification.
    ///
    /// # Errors
    /// Returns [`ModelError::ThemeNotFound`] if a named theme is not in the built-in catalog,
    /// or theme validation errors if a custom theme violates palette, metric, or format invariants.
    pub fn resolve_theme(&self) -> Result<Theme, ModelError> {
        match &self.theme {
            ThemeSpec::Name(name) => {
                let trimmed = name.trim();
                let theme_name = if trimmed.is_empty() {
                    Theme::DEFAULT_NAME
                } else {
                    trimmed
                };
                Theme::resolve_builtin(theme_name)
            }
            ThemeSpec::Custom(theme) => {
                theme.validate()?;
                Ok((**theme).clone())
            }
        }
    }
}

/// Freeze panes coordinate configuration for a worksheet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FreezePanes {
    pub row: u32,
    pub col: u32,
}

/// Worksheet column layout strategy.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SheetLayoutSpec {
    #[default]
    Auto,
    Grid {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        col_width: Option<f64>,
    },
}

impl<'de> Deserialize<'de> for SheetLayoutSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Serde ignores unknown fields on internally-tagged unit variants even with
        // `deny_unknown_fields`; deserialize via an empty struct variant `Auto {}`
        // so `{"kind": "auto", "col_width": ...}` and other unknown keys fail strictly.
        #[derive(Deserialize)]
        #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
        enum SheetLayoutSpecDe {
            Auto {},
            Grid {
                #[serde(default)]
                col_width: Option<f64>,
            },
        }

        match SheetLayoutSpecDe::deserialize(deserializer)? {
            SheetLayoutSpecDe::Auto {} => Ok(Self::Auto),
            SheetLayoutSpecDe::Grid { col_width } => Ok(Self::Grid { col_width }),
        }
    }
}

impl SheetLayoutSpec {
    /// Creates an automatic per-column arbitration sheet layout.
    #[must_use]
    pub const fn auto() -> Self {
        Self::Auto
    }

    /// Creates a uniform grid sheet layout using the theme's `min_col_width` as base column width.
    #[must_use]
    pub const fn grid() -> Self {
        Self::Grid { col_width: None }
    }

    /// Creates a uniform grid sheet layout with an explicit base column width.
    #[must_use]
    pub const fn grid_with_col_width(col_width: f64) -> Self {
        Self::Grid {
            col_width: Some(col_width),
        }
    }

    /// Returns true if this layout is the default `Auto` mode.
    #[must_use]
    pub const fn is_default(&self) -> bool {
        matches!(self, Self::Auto)
    }
}

/// Worksheet container metadata and contained table identifiers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SheetSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freeze_panes: Option<FreezePanes>,
    #[serde(default)]
    pub show_gridlines: bool,
    #[serde(default, skip_serializing_if = "SheetLayoutSpec::is_default")]
    pub layout: SheetLayoutSpec,
    pub tables: Vec<TableId>,
}
