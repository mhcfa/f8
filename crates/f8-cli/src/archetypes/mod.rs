//! Embedded template archetypes bundled into the f8 binary.

/// A single file in an archetype template.
#[derive(Debug, Clone, Copy)]
pub struct TemplateFile {
    /// Relative path within the project root (e.g. "f8.json", "tables/table.json").
    pub path: &'static str,
    /// UTF-8 file content.
    pub content: &'static str,
}

/// Archetype definition containing template metadata and embedded files.
#[derive(Debug, Clone, Copy)]
pub struct Archetype {
    pub name: &'static str,
    pub description: &'static str,
    pub files: &'static [TemplateFile],
}

static BLANK_FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "f8.json",
        content: include_str!("../../templates/blank/f8.json"),
    },
    TemplateFile {
        path: "tables/table.json",
        content: include_str!("../../templates/blank/tables/table.json"),
    },
];

pub static BLANK_ARCHETYPE: Archetype = Archetype {
    name: "blank",
    description: "Minimal starter project with a single table",
    files: BLANK_FILES,
};

static THREE_STATEMENT_FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "f8.json",
        content: include_str!("../../templates/three-statement/f8.json"),
    },
    TemplateFile {
        path: "tables/assumptions.json",
        content: include_str!("../../templates/three-statement/tables/assumptions.json"),
    },
    TemplateFile {
        path: "tables/income_statement.json",
        content: include_str!("../../templates/three-statement/tables/income_statement.json"),
    },
    TemplateFile {
        path: "tables/cash_flow.json",
        content: include_str!("../../templates/three-statement/tables/cash_flow.json"),
    },
    TemplateFile {
        path: "tables/balance_sheet.json",
        content: include_str!("../../templates/three-statement/tables/balance_sheet.json"),
    },
];

pub static THREE_STATEMENT_ARCHETYPE: Archetype = Archetype {
    name: "three-statement",
    description: "Integrated Income Statement, Balance Sheet, and Cash Flow Statement",
    files: THREE_STATEMENT_FILES,
};

static LBO_FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "f8.json",
        content: include_str!("../../templates/lbo/f8.json"),
    },
    TemplateFile {
        path: "tables/scenarios.json",
        content: include_str!("../../templates/lbo/tables/scenarios.json"),
    },
    TemplateFile {
        path: "tables/deal_assumptions.json",
        content: include_str!("../../templates/lbo/tables/deal_assumptions.json"),
    },
    TemplateFile {
        path: "tables/sources_and_uses.json",
        content: include_str!("../../templates/lbo/tables/sources_and_uses.json"),
    },
    TemplateFile {
        path: "tables/returns_summary.json",
        content: include_str!("../../templates/lbo/tables/returns_summary.json"),
    },
    TemplateFile {
        path: "tables/timeline.json",
        content: include_str!("../../templates/lbo/tables/timeline.json"),
    },
    TemplateFile {
        path: "tables/operations.json",
        content: include_str!("../../templates/lbo/tables/operations.json"),
    },
    TemplateFile {
        path: "tables/debt_schedule.json",
        content: include_str!("../../templates/lbo/tables/debt_schedule.json"),
    },
    TemplateFile {
        path: "tables/cash_flows.json",
        content: include_str!("../../templates/lbo/tables/cash_flows.json"),
    },
];

pub static LBO_ARCHETYPE: Archetype = Archetype {
    name: "lbo",
    description: "Leveraged buyout with debt tranches, amortization, and returns sensitivity",
    files: LBO_FILES,
};

static TRADING_COMPS_FILES: &[TemplateFile] = &[
    TemplateFile {
        path: "f8.json",
        content: include_str!("../../templates/trading-comps/f8.json"),
    },
    TemplateFile {
        path: "tables/comps.json",
        content: include_str!("../../templates/trading-comps/tables/comps.json"),
    },
    TemplateFile {
        path: "tables/target_valuation.json",
        content: include_str!("../../templates/trading-comps/tables/target_valuation.json"),
    },
];

pub static TRADING_COMPS_ARCHETYPE: Archetype = Archetype {
    name: "trading-comps",
    description: "Peer group multiples valuation and statistical benchmarking matrix",
    files: TRADING_COMPS_FILES,
};

static ARCHETYPES: &[&Archetype] = &[
    &THREE_STATEMENT_ARCHETYPE,
    &LBO_ARCHETYPE,
    &TRADING_COMPS_ARCHETYPE,
];

/// Returns all named archetypes available for `--template`.
pub fn named_archetypes() -> &'static [&'static Archetype] {
    ARCHETYPES
}

/// Resolves an archetype by name (e.g. "three-statement", "lbo", "trading-comps").
pub fn get_archetype(name: &str) -> Option<&'static Archetype> {
    match name {
        "blank" => Some(&BLANK_ARCHETYPE),
        "three-statement" => Some(&THREE_STATEMENT_ARCHETYPE),
        "lbo" => Some(&LBO_ARCHETYPE),
        "trading-comps" => Some(&TRADING_COMPS_ARCHETYPE),
        _ => None,
    }
}

/// Returns a comma-separated list of valid archetype names.
pub fn available_names() -> String {
    "three-statement, lbo, trading-comps".to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use f8_core::compiler::Compiler;
    use f8_core::model::manifest::ProjectManifest;
    use f8_core::model::table::TableSpec;

    #[test]
    fn test_all_embedded_archetypes_compile_cleanly() {
        for archetype in named_archetypes() {
            let mut manifest: Option<ProjectManifest> = None;
            let mut tables = Vec::new();

            for file in archetype.files {
                if file.path == "f8.json" {
                    manifest =
                        Some(serde_json::from_str(file.content).expect("manifest json valid"));
                } else if file.path.ends_with(".json") {
                    let table: TableSpec =
                        serde_json::from_str(file.content).expect("table json valid");
                    tables.push(table);
                }
            }

            let manifest = manifest.expect("archetype must contain f8.json");
            let theme = manifest.resolve_theme().expect("theme resolves");
            let compiled = Compiler::compile(&manifest, &tables, &theme).unwrap_or_else(|e| {
                panic!("Archetype '{}' failed to compile: {e}", archetype.name)
            });

            assert!(compiled.summary().total_cells > 0);
        }
    }

    #[test]
    fn test_blank_archetype_compiles_cleanly() {
        let archetype = &BLANK_ARCHETYPE;
        let mut manifest: Option<ProjectManifest> = None;
        let mut tables = Vec::new();

        for file in archetype.files {
            if file.path == "f8.json" {
                manifest = Some(serde_json::from_str(file.content).expect("manifest json valid"));
            } else if file.path.ends_with(".json") {
                let table: TableSpec =
                    serde_json::from_str(file.content).expect("table json valid");
                tables.push(table);
            }
        }

        let manifest = manifest.expect("archetype must contain f8.json");
        let theme = manifest.resolve_theme().expect("theme resolves");
        let compiled = Compiler::compile(&manifest, &tables, &theme)
            .unwrap_or_else(|e| panic!("Blank archetype failed to compile: {e}"));

        assert_eq!(compiled.summary().total_cells, 4);
    }
}
