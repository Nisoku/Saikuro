use std::path::PathBuf;
use std::process::Command;

#[test]
fn c_schema_extractor_cli_outputs_expected_schema() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("schema_service.h");

    let output = Command::new(env!("CARGO_BIN_EXE_saikuro-c-schema"))
        .arg("--namespace")
        .arg("parityns")
        .arg(&fixture)
        .output()
        .expect("run C schema extractor");

    assert!(
        output.status.success(),
        "schema extractor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let schema: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse schema JSON");

    let functions = &schema["namespaces"]["parityns"]["functions"];
    assert!(functions.get("add").is_some());
    assert!(functions.get("maybe").is_some());
    assert!(functions.get("avg").is_some());
    assert!(functions.get("fire_and_forget").is_some());
}

#[test]
fn c_schema_extractor_cli_pretty_flag_outputs_multiline_json() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("schema_service.h");

    let output = Command::new(env!("CARGO_BIN_EXE_saikuro-c-schema"))
        .arg("--namespace")
        .arg("parityns")
        .arg("--pretty")
        .arg(&fixture)
        .output()
        .expect("run C schema extractor with --pretty");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains('\n'));
    assert!(stdout.contains("\"namespaces\""));
}

#[test]
fn c_schema_extractor_cli_missing_file_returns_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_saikuro-c-schema"))
        .arg("--namespace")
        .arg("parityns")
        .arg("/definitely/missing/header.h")
        .output()
        .expect("run C schema extractor with missing file");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to read"));
}
