use indexmap::IndexSet;
use serde::{Deserialize, Serialize};

use super::error::ModelError;
use super::id::TableId;
use super::theme::Theme;

fn default_theme() -> String {
    Theme::DEFAULT_NAME.to_string()
}

/// Root workbook manifest defining model metadata, theme, and sheet ordering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub model_name: String,
    #[serde(default = "default_theme")]
    pub theme: String,
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

    /// Resolves the workbook theme from the built-in catalog, falling back to corporate navy if empty or omitted.
    ///
    /// # Errors
    /// Returns [`ModelError::ThemeNotFound`] if the configured theme is not a known built-in theme.
    pub fn resolve_theme(&self) -> Result<Theme, ModelError> {
        let theme_name = if self.theme.trim().is_empty() {
            Theme::DEFAULT_NAME
        } else {
            self.theme.trim()
        };
        Theme::resolve_builtin(theme_name)
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
