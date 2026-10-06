use std::path::{Path, PathBuf};

use f8_core::compiler::CompilerError;
use f8_core::model::id::TableId;
use f8_core::model::manifest::ProjectManifest;
use f8_core::model::table::TableSpec;
use f8_core::model::theme::Theme;
use indexmap::{IndexMap, IndexSet};

use crate::error::CliError;

/// In-memory representation of a loaded f8 multi-file project.
#[derive(Debug, Clone)]
pub struct LoadedProject {
    pub root_dir: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: ProjectManifest,
    pub tables: Vec<TableSpec>,
    pub theme: Theme,
}

/// Project loader handling manifest discovery, upward traversal, and multi-file assembly.
pub struct ProjectLoader;

impl ProjectLoader {
    /// Discovers and loads an f8 project from an optional path or by traversing upwards from the current directory.
    pub fn load(project_path: Option<&Path>) -> Result<LoadedProject, CliError> {
        let (manifest_path, root_dir) = Self::locate_manifest(project_path)?;

        let manifest_content =
            std::fs::read_to_string(&manifest_path).map_err(|source| CliError::Io {
                path: manifest_path.clone(),
                source,
            })?;

        let manifest: ProjectManifest =
            serde_json::from_str(&manifest_content).map_err(|source| CliError::Json {
                path: manifest_path.clone(),
                source,
            })?;

        let mut discovered_tables: IndexMap<TableId, (PathBuf, TableSpec)> = IndexMap::new();
        let tables_dir = root_dir.join("tables");

        if tables_dir.is_dir() {
            let mut file_paths = Vec::new();
            for entry in std::fs::read_dir(&tables_dir).map_err(|source| CliError::Io {
                path: tables_dir.clone(),
                source,
            })? {
                let entry = entry.map_err(|source| CliError::Io {
                    path: tables_dir.clone(),
                    source,
                })?;
                let path = entry.path();
                if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                    file_paths.push(path);
                }
            }

            file_paths.sort();

            for path in file_paths {
                let table_content =
                    std::fs::read_to_string(&path).map_err(|source| CliError::Io {
                        path: path.clone(),
                        source,
                    })?;
                let table_spec: TableSpec =
                    serde_json::from_str(&table_content).map_err(|source| CliError::Json {
                        path: path.clone(),
                        source,
                    })?;

                if let Some((existing_path, _)) = discovered_tables.get(&table_spec.id) {
                    return Err(CliError::DuplicateTableId {
                        id: table_spec.id.clone(),
                        path1: existing_path.clone(),
                        path2: path,
                    });
                }

                discovered_tables.insert(table_spec.id.clone(), (path, table_spec));
            }
        }

        // Assemble tables: first declared in manifest.sheets (preserving order),
        // then append any remaining discovered tables so f8-core can validate unassigned tables.
        let mut tables = Vec::new();
        let mut seen = IndexSet::new();

        for sheet in &manifest.sheets {
            for table_id in &sheet.tables {
                if let Some((_, table_spec)) = discovered_tables.get(table_id)
                    && seen.insert(table_id.clone())
                {
                    tables.push(table_spec.clone());
                }
            }
        }

        for (table_id, (_, table_spec)) in &discovered_tables {
            if seen.insert(table_id.clone()) {
                tables.push(table_spec.clone());
            }
        }

        let theme = Self::resolve_theme(&manifest.theme, &root_dir)?;

        Ok(LoadedProject {
            root_dir,
            manifest_path,
            manifest,
            tables,
            theme,
        })
    }

    /// Locates the `f8.json` manifest file and determines the root project directory.
    ///
    /// Returns `(manifest_path, root_dir)`.
    pub fn locate_manifest(project_path: Option<&Path>) -> Result<(PathBuf, PathBuf), CliError> {
        match project_path {
            Some(path) => {
                if path.is_file() {
                    let parent = match path.parent() {
                        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
                        _ => PathBuf::from("."),
                    };
                    Ok((path.to_path_buf(), parent))
                } else if path.is_dir() {
                    let manifest_path = path.join("f8.json");
                    if manifest_path.is_file() {
                        Ok((manifest_path, path.to_path_buf()))
                    } else {
                        Err(CliError::ProjectNotFound {
                            start_dir: path.to_path_buf(),
                        })
                    }
                } else {
                    Err(CliError::ProjectNotFound {
                        start_dir: path.to_path_buf(),
                    })
                }
            }
            None => {
                let start_dir = std::env::current_dir().map_err(|source| CliError::Io {
                    path: PathBuf::from("."),
                    source,
                })?;

                let mut curr = start_dir.clone();
                loop {
                    let candidate = curr.join("f8.json");
                    if candidate.is_file() {
                        return Ok((candidate, curr));
                    }
                    match curr.parent() {
                        Some(parent) => curr = parent.to_path_buf(),
                        None => return Err(CliError::ProjectNotFound { start_dir }),
                    }
                }
            }
        }
    }

    /// Resolves the theme specified by name or custom theme file path, checking for custom theme files on disk
    /// before falling back to built-in themes.
    pub fn resolve_theme(theme_str: &str, root_dir: &Path) -> Result<Theme, CliError> {
        let theme_str = theme_str.trim();
        if theme_str.is_empty() {
            return Theme::resolve_builtin(Theme::DEFAULT_NAME)
                .map_err(|e| CliError::Compiler(CompilerError::Model(e)));
        }

        let custom_path = root_dir.join(theme_str);
        if custom_path.is_file() {
            let content = std::fs::read_to_string(&custom_path).map_err(|source| CliError::Io {
                path: custom_path.clone(),
                source,
            })?;
            let theme: Theme = serde_json::from_str(&content).map_err(|source| CliError::Json {
                path: custom_path,
                source,
            })?;
            theme
                .validate()
                .map_err(|source| CliError::Compiler(CompilerError::Model(source)))?;
            return Ok(theme);
        }

        if theme_str.ends_with(".json") {
            return Err(CliError::Io {
                path: custom_path,
                source: std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("Custom theme file not found: '{theme_str}'"),
                ),
            });
        }

        Theme::resolve_builtin(theme_str).map_err(|e| CliError::Compiler(CompilerError::Model(e)))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_project_loader_direct_path() {
        let temp = tempdir().unwrap();
        let root = temp.path();

        let manifest_content = r#"{
            "model_name": "TestModel",
            "sheets": []
        }"#;
        fs::write(root.join("f8.json"), manifest_content).unwrap();

        let (manifest_path, root_dir) = ProjectLoader::locate_manifest(Some(root)).unwrap();
        assert_eq!(manifest_path, root.join("f8.json"));
        assert_eq!(root_dir, root);

        let project = ProjectLoader::load(Some(root)).unwrap();
        assert_eq!(project.manifest.model_name, "TestModel");
        assert_eq!(project.tables.len(), 0);
        assert_eq!(project.theme.name, "corporate_navy");
    }

    #[test]
    fn test_project_loader_multi_file_and_custom_theme() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let tables_dir = root.join("tables");
        fs::create_dir_all(&tables_dir).unwrap();

        let table_content = r#"{
            "id": "T1",
            "title": "Table One",
            "columns": [{"kind": "data", "id": "c1", "headers": ["C1"]}],
            "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 42.0}}]
        }"#;
        fs::write(tables_dir.join("t1.json"), table_content).unwrap();

        let mut custom_theme = Theme::corporate_navy();
        custom_theme.name = "custom_navy".to_string();
        let custom_theme_content = serde_json::to_string(&custom_theme).unwrap();
        fs::write(root.join("custom_theme.json"), custom_theme_content).unwrap();

        let manifest_content = r#"{
            "model_name": "MultiFileModel",
            "theme": "custom_theme.json",
            "sheets": [
                {
                    "name": "Sheet1",
                    "tables": ["T1"]
                }
            ]
        }"#;
        fs::write(root.join("f8.json"), manifest_content).unwrap();

        let project = ProjectLoader::load(Some(root)).unwrap();
        assert_eq!(project.manifest.model_name, "MultiFileModel");
        assert_eq!(project.tables.len(), 1);
        assert_eq!(project.tables[0].id.as_str(), "T1");
        assert_eq!(project.theme.name, "custom_navy");
    }

    #[test]
    fn test_project_loader_decoupled_filename_discovery() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let tables_dir = root.join("tables");
        fs::create_dir_all(&tables_dir).unwrap();

        let table_content = r#"{
            "id": "OPERATIONS",
            "title": "Operations Table",
            "columns": [{"kind": "data", "id": "c1", "headers": ["C1"]}],
            "rows": [{"kind": "data", "id": "r1", "cells": {"c1": 100.0}}]
        }"#;
        // Filename is completely decoupled from table ID
        fs::write(tables_dir.join("01_custom_ops_v2.json"), table_content).unwrap();

        let manifest_content = r#"{
            "model_name": "DecoupledModel",
            "sheets": [
                {
                    "name": "OpsSheet",
                    "tables": ["OPERATIONS"]
                }
            ]
        }"#;
        fs::write(root.join("f8.json"), manifest_content).unwrap();

        let project = ProjectLoader::load(Some(root)).unwrap();
        assert_eq!(project.tables.len(), 1);
        assert_eq!(project.tables[0].id.as_str(), "OPERATIONS");
    }

    #[test]
    fn test_project_loader_duplicate_table_id_rejected() {
        let temp = tempdir().unwrap();
        let root = temp.path();
        let tables_dir = root.join("tables");
        fs::create_dir_all(&tables_dir).unwrap();

        let table_content1 = r#"{
            "id": "T1",
            "title": "Table One",
            "columns": [{"kind": "data", "id": "c1"}],
            "rows": [{"kind": "data", "id": "r1"}]
        }"#;
        let table_content2 = r#"{
            "id": "T1",
            "title": "Table One Duplicate",
            "columns": [{"kind": "data", "id": "c1"}],
            "rows": [{"kind": "data", "id": "r1"}]
        }"#;
        fs::write(tables_dir.join("a.json"), table_content1).unwrap();
        fs::write(tables_dir.join("b.json"), table_content2).unwrap();

        let manifest_content = r#"{
            "model_name": "DupModel",
            "sheets": [{"name": "S1", "tables": ["T1"]}]
        }"#;
        fs::write(root.join("f8.json"), manifest_content).unwrap();

        let result = ProjectLoader::load(Some(root));
        assert!(matches!(
            result,
            Err(CliError::DuplicateTableId { ref id, .. }) if id.as_str() == "T1"
        ));
    }

    #[test]
    fn test_project_loader_missing_manifest() {
        let temp = tempdir().unwrap();
        let result = ProjectLoader::load(Some(temp.path()));
        assert!(matches!(result, Err(CliError::ProjectNotFound { .. })));
    }
}
