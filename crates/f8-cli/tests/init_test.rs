#![allow(clippy::unwrap_used, clippy::expect_used)]

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_scenario_1_blank_project_initialization() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("new_model");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args(["init", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("✓ Initialized new f8 project"));

    assert!(dest.join("f8.json").exists());
    assert!(dest.join("tables/table.json").exists());

    // Verify f8 check succeeds on the new model
    let mut check_cmd = Command::cargo_bin("f8").unwrap();
    check_cmd
        .args(["check", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);

    // Verify f8 compile succeeds on the new model
    let mut compile_cmd = Command::cargo_bin("f8").unwrap();
    compile_cmd
        .current_dir(temp.path())
        .args(["compile", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}

#[test]
fn test_scenario_2_three_statement_template_initialization() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("corp_ts");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args([
        "init",
        dest.to_str().unwrap(),
        "--template",
        "three-statement",
    ])
    .assert()
    .success()
    .code(0)
    .stdout(predicate::str::contains("three-statement"));

    assert!(dest.join("f8.json").exists());
    assert!(dest.join("tables/assumptions.json").exists());
    assert!(dest.join("tables/income_statement.json").exists());
    assert!(dest.join("tables/cash_flow.json").exists());
    assert!(dest.join("tables/balance_sheet.json").exists());

    let mut check_cmd = Command::cargo_bin("f8").unwrap();
    check_cmd
        .args(["check", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("4 tables"));

    let mut compile_cmd = Command::cargo_bin("f8").unwrap();
    compile_cmd
        .current_dir(temp.path())
        .args(["compile", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}

#[test]
fn test_scenario_3_lbo_template_initialization() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("corp_lbo");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args(["init", dest.to_str().unwrap(), "-t", "lbo"])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("lbo"));

    assert!(dest.join("f8.json").exists());
    assert!(dest.join("tables/scenarios.json").exists());
    assert!(dest.join("tables/deal_assumptions.json").exists());
    assert!(dest.join("tables/sources_and_uses.json").exists());
    assert!(dest.join("tables/returns_summary.json").exists());
    assert!(dest.join("tables/timeline.json").exists());
    assert!(dest.join("tables/operations.json").exists());
    assert!(dest.join("tables/debt_schedule.json").exists());
    assert!(dest.join("tables/cash_flows.json").exists());

    let mut check_cmd = Command::cargo_bin("f8").unwrap();
    check_cmd
        .args(["check", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("8 tables"));

    let mut compile_cmd = Command::cargo_bin("f8").unwrap();
    compile_cmd
        .current_dir(temp.path())
        .args(["compile", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}

#[test]
fn test_scenario_4_trading_comps_template_initialization() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("corp_comps");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args(["init", dest.to_str().unwrap(), "-t", "trading-comps"])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("trading-comps"));

    assert!(dest.join("f8.json").exists());
    assert!(dest.join("tables/comps.json").exists());
    assert!(dest.join("tables/target_valuation.json").exists());

    let mut check_cmd = Command::cargo_bin("f8").unwrap();
    check_cmd
        .args(["check", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("2 tables"));

    let mut compile_cmd = Command::cargo_bin("f8").unwrap();
    compile_cmd
        .current_dir(temp.path())
        .args(["compile", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}

#[test]
fn test_scenario_5_custom_theme_during_init() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("themed_model");
    fs::create_dir_all(&dest).unwrap();

    let mut custom_theme = f8_core::model::theme::Theme::corporate_navy();
    custom_theme.name = "custom_forest".to_string();
    fs::write(
        dest.join("custom_theme.json"),
        serde_json::to_string_pretty(&custom_theme).unwrap(),
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args([
        "init",
        dest.to_str().unwrap(),
        "--theme",
        "custom_theme.json",
    ])
    .assert()
    .success()
    .code(0);

    let manifest_str = fs::read_to_string(dest.join("f8.json")).unwrap();
    assert!(manifest_str.contains("\"custom_theme.json\""));

    let mut check_cmd = Command::cargo_bin("f8").unwrap();
    check_cmd
        .args(["check", "-p", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);
}

#[test]
fn test_scenario_6_machine_readable_json_output() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("json_model");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args(["init", dest.to_str().unwrap(), "--json"])
        .assert()
        .success()
        .code(0)
        .stdout(predicate::str::contains("\"status\": \"ok\""))
        .stdout(predicate::str::contains("\"template\": \"blank\""))
        .stdout(predicate::str::contains("\"theme\": \"corporate_navy\""));
}

#[test]
fn test_scenario_7_error_on_unknown_template() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("bad_model");

    let mut cmd = Command::cargo_bin("f8").unwrap();
    cmd.args(["init", dest.to_str().unwrap(), "-t", "non_existent"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains(
            "Unknown template archetype 'non_existent'",
        ))
        .stderr(predicate::str::contains(
            "Available templates: three-statement, lbo, trading-comps",
        ));
}

#[test]
fn test_scenario_8_error_on_existing_project() {
    let temp = tempdir().unwrap();
    let dest = temp.path().join("existing_model");

    let mut cmd1 = Command::cargo_bin("f8").unwrap();
    cmd1.args(["init", dest.to_str().unwrap()])
        .assert()
        .success()
        .code(0);

    let mut cmd2 = Command::cargo_bin("f8").unwrap();
    cmd2.args(["init", dest.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Project already exists"));
}
