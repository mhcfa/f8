use indexmap::IndexSet;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::error::ModelError;

/// Root project manifest deserialized from `finx.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub model_name: String,
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

/// Worksheet container metadata and contained table specification paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SheetSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub freeze_panes: Option<FreezePanes>,
    #[serde(default = "default_true")]
    pub show_gridlines: bool,
    pub tables: Vec<PathBuf>,
}
