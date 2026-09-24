use std::path::Path;

use f8_core::model::manifest::ProjectManifest;

use crate::archetypes::Archetype;
use crate::error::CliError;
use crate::project::ProjectLoader;

/// Scaffolds project files and directory structures on disk.
pub struct ProjectWriter;

impl ProjectWriter {
    /// Writes an archetype project to the target destination directory.
    pub fn write_archetype(
        dest_dir: &Path,
        archetype: &Archetype,
        theme_override: Option<&str>,
    ) -> Result<(), CliError> {
        let manifest_path = dest_dir.join("f8.json");
        if manifest_path.exists() {
            return Err(CliError::ProjectAlreadyExists {
                path: dest_dir.to_path_buf(),
            });
        }

        if !dest_dir.exists() {
            std::fs::create_dir_all(dest_dir).map_err(|source| CliError::Io {
                path: dest_dir.to_path_buf(),
                source,
            })?;
        }

        // Validate theme override early if provided
        if let Some(theme_name) = theme_override {
            ProjectLoader::resolve_theme(theme_name, dest_dir)?;
        }

        for file in archetype.files {
            let target_file_path = dest_dir.join(file.path);
            if let Some(parent) = target_file_path.parent()
                && !parent.exists()
            {
                std::fs::create_dir_all(parent).map_err(|source| CliError::Io {
                    path: parent.to_path_buf(),
                    source,
                })?;
            }

            if file.path == "f8.json" {
                let mut manifest: ProjectManifest =
                    serde_json::from_str(file.content).map_err(|source| CliError::Json {
                        path: target_file_path.clone(),
                        source,
                    })?;

                if let Some(theme_name) = theme_override {
                    manifest.theme = theme_name.to_string();
                }

                // If blank archetype and dest_dir has a valid folder name, adopt it as model_name
                if archetype.name == "blank"
                    && let Some(dir_name) = dest_dir.file_name().and_then(|n| n.to_str())
                    && dir_name != "."
                    && !dir_name.is_empty()
                {
                    manifest.model_name = dir_name.to_string();
                }

                let serialized =
                    serde_json::to_string_pretty(&manifest).map_err(|source| CliError::Json {
                        path: target_file_path.clone(),
                        source,
                    })?;

                std::fs::write(&target_file_path, serialized).map_err(|source| CliError::Io {
                    path: target_file_path,
                    source,
                })?;
            } else {
                std::fs::write(&target_file_path, file.content).map_err(|source| CliError::Io {
                    path: target_file_path,
                    source,
                })?;
            }
        }

        Ok(())
    }
}
