#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn setup_financial_model(dir: &std::path::Path) {
    let tables_dir = dir.join("tables");
    fs::create_dir_all(&tables_dir).unwrap();

    let manifest = r#"{
        "model_name": "InspectionTestModel",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Financials",
                "tables": ["Assumptions", "IncomeStatement"]
            }
        ]
    }"#;
    fs::write(dir.join("f8.json"), manifest).unwrap();

    let assumptions = r#"{
        "id": "Assumptions",
        "title": "Model Assumptions",
        "columns": [
            { "kind": "data", "id": "line_item", "role": "descriptor", "headers": ["Parameter"] },
            { "kind": "data", "id": "val", "role": "measure", "headers": ["Value"] }
        ],
        "rows": [
            {
                "kind": "data",
                "id": "rev_growth",
                "format": "percentage",
                "decimals": 1,
                "cells": { "line_item": "Revenue Growth Rate", "val": 0.15 }
            },
            {
                "kind": "data",
                "id": "tax_rate",
                "format": "percentage",
                "decimals": 1,
                "cells": { "line_item": "Corporate Tax Rate", "val": 0.20 }
            }
        ]
    }"#;
    fs::write(tables_dir.join("assumptions.json"), assumptions).unwrap();

    let income_statement = r#"{
        "id": "IncomeStatement",
        "title": "Income Statement",
        "columns": [
            { "kind": "data", "id": "line_item", "role": "descriptor", "headers": ["Line Item"] },
            { "kind": "data", "id": "y2023a", "role": "measure", "headers": ["2023A"] },
            { "kind": "data", "id": "y2024e", "role": "measure", "headers": ["2024E"] }
        ],
        "rows": [
            {
                "kind": "data",
                "id": "revenue",
                "format": "currency",
                "decimals": 1,
                "expr": "IncomeStatement[\"revenue\", col.prev] * (1.0 + Assumptions[\"rev_growth\", \"val\"])",
                "cells": { "line_item": "Revenue", "y2023a": 100.0 }
            },
            {
                "kind": "data",
                "id": "cogs",
                "format": "currency",
                "decimals": 1,
                "expr": "IncomeStatement[\"revenue\", @col] * 0.40",
                "cells": { "line_item": "Cost of Goods Sold" }
            },
            {
                "kind": "data",
                "id": "negative_item",
                "format": "number",
                "decimals": 1,
                "cells": { "line_item": "Negative Benchmark", "y2023a": -50.0, "y2024e": -50.0 }
            },
            {
                "kind": "section_header",
                "title": "Operating Expenses"
            },
            {
                "kind": "data",
                "id": "opex",
                "format": "currency",
                "decimals": 1,
                "cells": { "line_item": "Salaries & SG&A", "y2023a": 20.0, "y2024e": 20.0 }
            },
            {
                "kind": "data",
                "id": "net_income",
                "format": "currency",
                "decimals": 1,
                "expr": "IncomeStatement[\"revenue\", @col] - IncomeStatement[\"cogs\", @col] - IncomeStatement[\"opex\", @col]",
                "cells": { "line_item": "Net Income" }
            }
        ]
    }"#;
    fs::write(tables_dir.join("income_statement.json"), income_statement).unwrap();
}

#[test]
fn test_scenario_1_model_summary_inspection() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // 1. Human summary
    Command::cargo_bin("f8")
        .unwrap()
        .args(["inspect", "-p", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Model:     InspectionTestModel"))
        .stdout(predicate::str::contains("Theme:     corporate_navy"))
        .stdout(predicate::str::contains("Sheets (1):"))
        .stdout(predicate::str::contains("• Financials (2 tables"))
        .stdout(predicate::str::contains(
            "Model Assumptions [Assumptions] [2 rows x 2 cols]",
        ))
        .stdout(predicate::str::contains(
            "Income Statement [IncomeStatement] [6 rows x 3 cols]",
        ))
        .stdout(predicate::str::contains("DAG acyclic (0 cycles)"));

    // 2. JSON summary
    Command::cargo_bin("f8")
        .unwrap()
        .args(["inspect", "--json", "-p", dir.path().to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"model_name\": \"InspectionTestModel\"",
        ))
        .stdout(predicate::str::contains("\"total_sheets\": 1"))
        .stdout(predicate::str::contains("\"total_tables\": 2"));
}

#[test]
fn test_scenario_2_table_matrix_values_view_accounting_format() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // 1. Default format is Markdown
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--view",
            "values",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| Line Item [line_item] | 2023A [y2023a] | 2024E [y2024e] |",
        ))
        .stdout(predicate::str::contains("| Cost of Goods Sold [cogs] |"))
        .stdout(predicate::str::contains("$ 100.0"))
        .stdout(predicate::str::contains("$ 115.0"))
        .stdout(predicate::str::contains("(50.0)"))
        .stdout(predicate::str::contains("| **Operating Expenses** |"));

    // 2. Explicit --format table outputs Unicode box-drawing table
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--view",
            "values",
            "--format",
            "table",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("2024E [y2024e]"))
        .stdout(predicate::str::contains("Cost of Goods Sold [cogs]"))
        .stdout(predicate::str::contains("$ 100.0"))
        .stdout(predicate::str::contains("$ 115.0"))
        .stdout(predicate::str::contains("(50.0)"))
        .stdout(predicate::str::contains("Operating Expenses"))
        .stdout(predicate::str::contains("┌─"))
        .stdout(predicate::str::contains("└─"));
}

#[test]
fn test_scenario_3_table_matrix_formulas_and_markdown() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--view",
            "formulas",
            "--format",
            "markdown",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "| Line Item [line_item] | 2023A [y2023a] | 2024E [y2024e] |",
        ))
        .stdout(predicate::str::contains("| :--- | ---: | ---: |"))
        .stdout(predicate::str::contains("| Cost of Goods Sold [cogs] |"))
        .stdout(predicate::str::contains("=C12*(1+$C$5)"))
        .stdout(predicate::str::contains("=C12*0.4"))
        .stdout(predicate::str::contains("| **Operating Expenses** |"));

    // View mode: expr (DSL expressions)
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--view",
            "expr",
            "--format",
            "markdown",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "IncomeStatement[\"revenue\", col.prev]",
        ));

    // View mode: addresses (Physical A1 coordinates)
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--view",
            "addresses",
            "--format",
            "markdown",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("C12"))
        .stdout(predicate::str::contains("D12"));
}

#[test]
fn test_scenario_4_table_matrix_both_view_and_json() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // Both view (values + formulas stacked)
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--view",
            "both",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("$ 115.0"))
        .stdout(predicate::str::contains("=C12*(1+$C$5)"));

    // JSON format
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "IncomeStatement",
            "--format",
            "json",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "\"table_id\": \"IncomeStatement\"",
        ))
        .stdout(predicate::str::contains("\"cells\": {"));
}

#[test]
fn test_scenario_5_cell_inspection_card_and_json() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // Human card
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "net_income",
            "y2024e",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Coordinate:     IncomeStatement[\"net_income\", \"y2024e\"]",
        ))
        .stdout(predicate::str::contains("Sheet Address:  Financials!"))
        .stdout(predicate::str::contains("Evaluated:      $ 49.0"))
        .stdout(predicate::str::contains("Formula Expr:   "))
        .stdout(predicate::str::contains("Excel Formula:  "))
        .stdout(predicate::str::contains("Precedents (3):"))
        .stdout(predicate::str::contains(
            "├── IncomeStatement[\"cogs\", \"y2024e\"] = $ 46.0",
        ));

    // JSON format
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "net_income",
            "y2024e",
            "--json",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"coordinate\": {"))
        .stdout(predicate::str::contains("\"evaluated_value\": {"))
        .stdout(predicate::str::contains("\"formatted_value\": \"$ 49.0\""))
        .stdout(predicate::str::contains("\"is_formula\": true"));
}

#[test]
fn test_scenario_6_precedents_direct_and_recursive() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // Direct precedents
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "precedents",
            "IncomeStatement",
            "net_income",
            "y2024e",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Direct Precedents (3):"))
        .stdout(predicate::str::contains(
            "├── IncomeStatement[\"cogs\", \"y2024e\"] = $ 46.0",
        ));

    // Recursive precedents tree
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "precedents",
            "IncomeStatement",
            "net_income",
            "y2024e",
            "--recursive",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Transitive Precedent Tree (5 calculations):",
        ))
        .stdout(predicate::str::contains(
            "Assumptions[\"rev_growth\", \"val\"] = 15.0%",
        ));

    // JSON precedents
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "precedents",
            "IncomeStatement",
            "net_income",
            "y2024e",
            "--json",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"root\": {"))
        .stdout(predicate::str::contains("\"direct_count\": 3"))
        .stdout(predicate::str::contains("\"formatted_value\": \"$ 46.0\""));
}

#[test]
fn test_scenario_7_dependents_direct_and_recursive() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // Direct dependents
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "dependents",
            "Assumptions",
            "rev_growth",
            "val",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Direct Dependents (1):"))
        .stdout(predicate::str::contains(
            "└── IncomeStatement[\"revenue\", \"y2024e\"] = $ 115.0",
        ));

    // Recursive blast radius tree
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "dependents",
            "Assumptions",
            "rev_growth",
            "val",
            "--recursive",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Transitive Blast Radius (3 downstream calculations affected):",
        ))
        .stdout(predicate::str::contains(
            "IncomeStatement[\"cogs\", \"y2024e\"]",
        ))
        .stdout(predicate::str::contains(
            "IncomeStatement[\"net_income\", \"y2024e\"]",
        ));
}

#[test]
fn test_scenario_8_missing_entity_compiler_diagnostics() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // Non-existent table -> Exit code 2
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "NonExistentTable",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "Error: Table 'NonExistentTable' not found in compiled model",
        ));

    // Non-existent cell -> Exit code 2
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "nonexistent_row",
            "y2024e",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains(
            "Error: Cell 'IncomeStatement[\"nonexistent_row\", \"y2024e\"]' not found in compiled model",
        ));

    // Non-existent cell with --json -> Exit code 2 and structured error JSON on stderr
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "nonexistent_row",
            "y2024e",
            "--json",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("\"exit_code\": 2"))
        .stderr(predicate::str::contains("\"status\": \"error\""));
}

#[test]
fn test_scenario_9_lbo_recursive_precedents_pruned_bounds() {
    let lbo_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/lbo");

    let assert = Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "precedents",
            "ReturnsSummary",
            "irr",
            "val",
            "--recursive",
            "-p",
            lbo_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Transitive Precedent Tree (150 calculations):",
        ))
        .stdout(predicate::str::contains("(already visited)"));

    let stdout_str = String::from_utf8_lossy(&assert.get_output().stdout);
    let line_count = stdout_str.lines().count();
    assert!(
        line_count < 300,
        "Pruned LBO recursive tree must have < 300 lines, got {line_count}"
    );
}

#[test]
fn test_scenario_10_recursive_precedents_json_pruned_flag() {
    let lbo_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/lbo");

    let assert = Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "precedents",
            "ReturnsSummary",
            "irr",
            "val",
            "--recursive",
            "--json",
            "-p",
            lbo_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let json_val: serde_json::Value =
        serde_json::from_slice(&assert.get_output().stdout).expect("Must be valid JSON");
    assert_eq!(json_val["transitive_count"], 150);

    // Recursively search for any node with "is_pruned": true
    fn has_pruned_node(node: &serde_json::Value) -> bool {
        if node.get("is_pruned").and_then(|v| v.as_bool()) == Some(true) {
            return true;
        }
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            return children.iter().any(has_pruned_node);
        }
        false
    }

    let nodes = json_val["nodes"].as_array().expect("Nodes must be array");
    assert!(
        nodes.iter().any(has_pruned_node),
        "Expected at least one node to be pruned in recursive tree"
    );
}

#[cfg(unix)]
#[test]
fn test_scenario_11_sigpipe_broken_pipe_head() {
    let lbo_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/lbo");
    let f8_bin = assert_cmd::cargo::cargo_bin("f8");

    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!(
            "{} inspect precedents ReturnsSummary irr val -p {} --recursive --json | head -n 5",
            f8_bin.display(),
            lbo_path.display()
        ))
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("Broken pipe"),
        "Must not emit Broken pipe panic trace: {stderr}"
    );
    assert!(
        !stderr.contains("panicked"),
        "Must not panic on SIGPIPE: {stderr}"
    );
}

#[test]
fn test_scenario_flexible_coordinate_input() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // 1. 3 Positional arguments
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "cogs",
            "y2024e",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Coordinate:     IncomeStatement[\"cogs\", \"y2024e\"]",
        ))
        .stdout(predicate::str::contains("Evaluated:      $ 46.0"));

    // 2. Canonical IR string (double-quoted IDs)
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement[\"cogs\", \"y2024e\"]",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Coordinate:     IncomeStatement[\"cogs\", \"y2024e\"]",
        ))
        .stdout(predicate::str::contains("Evaluated:      $ 46.0"));

    // 3. Unquoted IDs in IR string
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement[cogs, y2024e]",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Coordinate:     IncomeStatement[\"cogs\", \"y2024e\"]",
        ))
        .stdout(predicate::str::contains("Evaluated:      $ 46.0"));

    // 4. Precedents via IR string
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "precedents",
            "IncomeStatement[\"cogs\", \"y2024e\"]",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Direct Precedents (1):"));

    // 5. Dependents via IR string
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "dependents",
            "Assumptions[\"rev_growth\", \"val\"]",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Direct Dependents (1):"));

    // 6. Incomplete 2-arg coordinate -> Exit code 1
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "cogs",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Invalid coordinate 'IncomeStatement cogs'",
        ))
        .stderr(predicate::str::contains(
            "expected 3 positional arguments (<table row col>) or 1 canonical IR string (Table[\"row\", \"col\"])",
        ));

    // 7. Single token without brackets -> Exit code 1
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "cell",
            "IncomeStatement",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(1)
        .stderr(predicate::str::contains(
            "Invalid coordinate 'IncomeStatement'",
        ));
}

#[test]
fn test_scenario_multitier_headers_and_year_formatting() {
    let lbo_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/lbo");

    // Table inspection of Timeline in LBO
    let output = Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "Timeline",
            "-p",
            lbo_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let stdout = String::from_utf8_lossy(&output.get_output().stdout);

    // Multi-tier header assertions
    assert!(stdout.contains("Project Titan LBO"));
    assert!(stdout.contains("Transaction Close"));
    assert!(stdout.contains("5-Year Investment Horizon"));
    assert!(stdout.contains("Line Item [line_item]"));
    assert!(stdout.contains("Close (Y0) [y0]"));
    assert!(stdout.contains("2025 (Y1) [y1]"));

    // Calendar year formatting: must render without commas
    assert!(stdout.contains("2024"));
    assert!(stdout.contains("2025"));
    assert!(stdout.contains("2029"));
    assert!(!stdout.contains("2,024"));
    assert!(!stdout.contains("2,025"));
    assert!(!stdout.contains("2,029"));

    // Markdown inspection of Timeline in LBO
    let md_output = Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "table",
            "Timeline",
            "--format",
            "markdown",
            "-p",
            lbo_path.to_str().unwrap(),
        ])
        .assert()
        .success();

    let md_stdout = String::from_utf8_lossy(&md_output.get_output().stdout);
    assert!(md_stdout.contains("Project Titan LBO"));
    assert!(md_stdout.contains("5-Year Investment Horizon"));
    assert!(md_stdout.contains("2025 (Y1) [y1]"));
    assert!(md_stdout.contains("2029"));
    assert!(!md_stdout.contains("2,029"));
}

#[test]
fn test_scenario_inspect_sheet_markdown_and_view_modes() {
    let dir = tempdir().unwrap();
    setup_financial_model(dir.path());

    // 1. Markdown with default values view
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "sheet",
            "Financials",
            "--format",
            "markdown",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("Sheet: Financials"))
        .stdout(predicate::str::contains(
            "Tables: Assumptions, IncomeStatement",
        ))
        .stdout(predicate::str::contains("| Row | Col B | Col C | Col D |"))
        .stdout(predicate::str::contains("Model Assumptions [Assumptions]"))
        .stdout(predicate::str::contains(
            "Income Statement [IncomeStatement]",
        ))
        .stdout(predicate::str::contains("Revenue Growth Rate [rev_growth]"))
        .stdout(predicate::str::contains("$ 100.0"))
        .stdout(predicate::str::contains("$ 115.0"));

    // 2. Formulas view mode
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "sheet",
            "Financials",
            "--view",
            "formulas",
            "--format",
            "markdown",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("=C12*(1+$C$5)"))
        .stdout(predicate::str::contains("=C12*0.4"));

    // 3. Addresses view mode
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "sheet",
            "Financials",
            "--view",
            "addresses",
            "--format",
            "markdown",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("C12"))
        .stdout(predicate::str::contains("D12"));

    // 4. Unicode Table format
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "sheet",
            "Financials",
            "--format",
            "table",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("┌─"))
        .stdout(predicate::str::contains("└─"))
        .stdout(predicate::str::contains("Col B"))
        .stdout(predicate::str::contains("Income Statement"));

    // 5. JSON format
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "sheet",
            "Financials",
            "--json",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"sheet_id\": \"Financials\""))
        .stdout(predicate::str::contains(
            "\"table_id\": \"IncomeStatement\"",
        ))
        .stdout(predicate::str::contains("\"row_id\": \"revenue\""))
        .stdout(predicate::str::contains("\"physical_a1\": \"D12\""));

    // 6. Non-existent sheet returns error exit code 2
    Command::cargo_bin("f8")
        .unwrap()
        .args([
            "inspect",
            "sheet",
            "NonExistent",
            "-p",
            dir.path().to_str().unwrap(),
        ])
        .assert()
        .code(2)
        .stderr(predicate::str::contains("Sheet 'NonExistent' not found"));
}
