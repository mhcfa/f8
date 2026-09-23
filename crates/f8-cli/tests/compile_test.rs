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
            { "kind": "data", "id": "r1", "cells": { "val": { "expr": "circular[\"r2\", \"val\"]" } } },
            { "kind": "data", "id": "r2", "cells": { "val": { "expr": "circular[\"r1\", \"val\"]" } } }
        ]
    }"#;
    fs::write(tables_dir.join("circular.json"), table).unwrap();
}

#[test]
fn test_scenario_1_default_compilation_to_disk() {
    let temp = tempdir().unwrap();
    setup_valid_project(temp.path());

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.current_dir(temp.path())
        .arg("compile")
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains(
            "✓ Compiled 1 sheets (2 cells) to TestModel.xlsx",
        ));

    let out_file = temp.path().join("TestModel.xlsx");
    assert!(out_file.exists());
    let bytes = fs::read(&out_file).unwrap();
    assert!(
        bytes.starts_with(b"PK\x03\x04"),
        "Must be a valid OpenXML zip archive"
    );
}

#[test]
fn test_scenario_2_explicit_output_path_and_theme_override() {
    let temp = tempdir().unwrap();
    setup_valid_project(temp.path());

    let dest = temp.path().join("dist/custom_output.xlsx");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.current_dir(temp.path())
        .args([
            "compile",
            "-o",
            dest.to_str().unwrap(),
            "--theme",
            "corporate_navy",
        ])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("dist/custom_output.xlsx"));

    assert!(dest.exists());
    let bytes = fs::read(&dest).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn test_scenario_3_stdout_streaming_when_piped() {
    let temp = tempdir().unwrap();
    setup_valid_project(temp.path());

    let mut cmd = Command::cargo_bin("f8").unwrap();
    let assert = cmd
        .current_dir(temp.path())
        .args(["compile", "-o", "-"])
        .assert()
        .success()
        .code(0);

    let stdout_bytes = assert.get_output().stdout.clone();
    assert!(
        stdout_bytes.starts_with(b"PK\x03\x04"),
        "Stdout must contain raw OpenXML zip archive bytes when -o - is used"
    );
}

#[test]
fn test_scenario_4_compilation_flags_values_only_and_omit_cache() {
    let temp = tempdir().unwrap();
    setup_valid_project(temp.path());

    let dest = temp.path().join("frozen.xlsx");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.current_dir(temp.path())
        .args([
            "compile",
            "-o",
            dest.to_str().unwrap(),
            "--values-only",
            "--omit-cache",
        ])
        .assert()
        .success()
        .code(0);

    assert!(dest.exists());
    let bytes = fs::read(&dest).unwrap();
    assert!(bytes.starts_with(b"PK\x03\x04"));
}

#[test]
fn test_scenario_5_machine_readable_json_output() {
    let temp = tempdir().unwrap();
    setup_valid_project(temp.path());

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.current_dir(temp.path())
        .args(["compile", "--json"])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"size_bytes\":"))
        .stdout(predicate::str::contains("\"path\": \"TestModel.xlsx\""));
}

#[test]
fn test_scenario_6_compiler_diagnostic_exit_code_2() {
    let temp = tempdir().unwrap();
    setup_circular_project(temp.path());

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.current_dir(temp.path())
        .arg("compile")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("Circular dependency detected"));
}
