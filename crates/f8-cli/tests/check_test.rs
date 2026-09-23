#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

fn setup_valid_project(dir: &std::path::Path) {
    let tables_dir = dir.join("tables");
    fs::create_dir_all(&tables_dir).unwrap();

    let manifest = r#"{
        "model_name": "TestModel",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Summary",
                "tables": ["tables/table.json"]
            }
        ]
    }"#;
    fs::write(dir.join("f8.json"), manifest).unwrap();

    let table = r#"{
        "id": "table",
        "title": "Table One",
        "columns": [
            { "kind": "data", "id": "val", "headers": ["Value"] }
        ],
        "rows": [
            { "kind": "data", "id": "r1", "cells": { "val": 100.0 } },
            { "kind": "data", "id": "r2", "cells": { "val": { "expr": "table[\"r1\", \"val\"] * 2" } } }
        ]
    }"#;
    fs::write(tables_dir.join("table.json"), table).unwrap();
}

fn setup_circular_project(dir: &std::path::Path) {
    let tables_dir = dir.join("tables");
    fs::create_dir_all(&tables_dir).unwrap();

    let manifest = r#"{
        "model_name": "CircularModel",
        "theme": "corporate_navy",
        "sheets": [
            {
                "name": "Model",
                "tables": ["tables/circular.json"]
            }
        ]
    }"#;
    fs::write(dir.join("f8.json"), manifest).unwrap();

    let table = r#"{
        "id": "circular",
        "title": "Circular Table",
        "columns": [
            { "kind": "data", "id": "val", "headers": ["Value"] }
        ],
        "rows": [
            { "kind": "data", "id": "r1", "cells": { "val": { "expr": "circular[\"r2\", \"val\"] + 1" } } },
            { "kind": "data", "id": "r2", "cells": { "val": { "expr": "circular[\"r1\", \"val\"] + 1" } } }
        ]
    }"#;
    fs::write(tables_dir.join("circular.json"), table).unwrap();
}

#[test]
fn test_scenario_1_upward_project_discovery() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    setup_valid_project(root);

    let deeply_nested = root.join("a").join("b").join("c");
    fs::create_dir_all(&deeply_nested).unwrap();

    Command::cargo_bin("f8")
        .unwrap()
        .current_dir(&deeply_nested)
        .arg("check")
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("Status: OK"))
        .stdout(predicate::str::contains(
            "Validated 1 sheets, 1 tables, 2 cells",
        ))
        .stdout(predicate::str::contains(
            "DAG acyclicity verified (0 cycles)",
        ));
}

#[test]
fn test_scenario_2_explicit_project_flag() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    setup_valid_project(root);

    // Flag before subcommand: f8 -p <path> check
    Command::cargo_bin("f8")
        .unwrap()
        .args(["-p", root.to_str().unwrap(), "check"])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("Status: OK"));

    // Flag after subcommand: f8 check -p <path>
    Command::cargo_bin("f8")
        .unwrap()
        .args(["check", "-p", root.to_str().unwrap()])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("Status: OK"));
}

#[test]
fn test_scenario_3_missing_manifest_operational_error() {
    let temp = tempdir().unwrap();
    let empty_dir = temp.path();

    Command::cargo_bin("f8")
        .unwrap()
        .current_dir(empty_dir)
        .arg("check")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "Project manifest 'f8.json' not found",
        ));
}

#[test]
fn test_scenario_4_invalid_json_file_error() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    setup_valid_project(root);

    // Corrupt the table JSON
    fs::write(
        root.join("tables").join("table.json"),
        "{ invalid json syntax",
    )
    .unwrap();

    Command::cargo_bin("f8")
        .unwrap()
        .args(["-p", root.to_str().unwrap(), "check"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("JSON error in"));
}

#[test]
fn test_scenario_5_fail_fast_compiler_diagnostic() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    setup_circular_project(root);

    Command::cargo_bin("f8")
        .unwrap()
        .args(["-p", root.to_str().unwrap(), "check"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Cycle"));
}

#[test]
fn test_scenario_6_machine_readable_json_output_on_success() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    setup_valid_project(root);

    let assert = Command::cargo_bin("f8")
        .unwrap()
        .args(["-p", root.to_str().unwrap(), "check", "--json"])
        .assert()
        .success()
        .code(0);

    let output = assert.get_output();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["summary"]["sheets"], 1);
    assert_eq!(json["summary"]["tables"], 1);
    assert_eq!(json["summary"]["cells"], 2);
    assert!(json["summary"]["duration_ms"].is_number());
}

#[test]
fn test_scenario_7_machine_readable_json_output_on_compiler_failure() {
    let temp = tempdir().unwrap();
    let root = temp.path();
    setup_circular_project(root);

    let assert = Command::cargo_bin("f8")
        .unwrap()
        .args(["-p", root.to_str().unwrap(), "check", "--json"])
        .assert()
        .failure()
        .code(2);

    let output = assert.get_output();
    let stderr = String::from_utf8_lossy(&output.stderr);
    let json: serde_json::Value = serde_json::from_str(&stderr).unwrap();

    assert_eq!(json["status"], "error");
    assert_eq!(json["exit_code"], 2);
    assert!(json["error"].as_str().unwrap().contains("Cycle"));
}
