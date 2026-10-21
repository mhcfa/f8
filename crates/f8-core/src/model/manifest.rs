use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::id::TableId;
use super::theme::{Theme, ThemeSpec};

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
        let mut seen_sheets = IndexSet::new();
        for sheet in &self.sheets {
            if !seen_sheets.insert(&sheet.name) {
                return Err(ModelError::DuplicateSheetName {
                    sheet: sheet.name.clone(),
                });
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

const fn default_true() -> bool {
    true
}

/// Worksheet container metadata and contained table identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freeze_panes: Option<FreezePanes>,
    #[serde(default = "default_true")]
    pub show_gridlines: bool,
    pub tables: Vec<TableId>,
}
