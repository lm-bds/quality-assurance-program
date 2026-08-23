use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use qapsimple::pipeline::{
    load_csv, make_output_directory_name, run_from_args, sanitize_path_component,
};
use qapsimple::reporting::{generate_reports, render_report_bytes};

fn unique_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time moved backwards")
        .as_nanos();
    std::env::temp_dir().join(format!("qaprs-{prefix}-{nanos}"))
}

fn write_csv(dir: &Path, name: &str, contents: &str) -> PathBuf {
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(name);
    fs::write(&path, contents).unwrap();
    path
}

fn synthetic_csv() -> String {
    [
        "site,device_type,device_id,test,cycle,datetime,value,units",
        "Site {Alpha}\\Path,Abbott,DEV_1,CRP,Cycle-1,2026-01-01T00:00:00Z,10.5,mg/L",
        "Site {Alpha}\\Path,Abbott,DEV_1,CRP,Cycle-1,2026-01-01T00:05:00Z,11.5,mg/L",
        "Site {Alpha}\\Path,Abbott,DEV_1,CRP,Cycle-2,2026-01-02T00:00:00Z,12.0,mg/L",
        "Site {Alpha}\\Path,Abbott,DEV_1,CRP,Cycle-2,2026-01-02T00:05:00Z,13.0,mg/L",
    ]
    .join("\n")
        + "\n"
}

#[test]
fn help_text_is_available() {
    let output = Command::new(env!("CARGO_BIN_EXE_qapsimple"))
        .arg("--help")
        .output()
        .expect("binary runs");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Usage:"));
    assert!(stdout.contains("<CSV_PATH>"));
    assert!(stdout.contains("--output DIR"));
}

#[test]
fn cli_rejects_bad_args() {
    let output = Command::new(env!("CARGO_BIN_EXE_qapsimple"))
        .args(["input.csv", "extra"])
        .output()
        .expect("binary runs");
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Usage:"));
}

#[test]
fn invalid_header_is_rejected() {
    let dir = unique_dir("invalid-header");
    let csv = write_csv(
        &dir,
        "bad.csv",
        "site,device_type,device_id,test,cycle,datetime,value,wrong\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10.5,mg/L\nSite A,Abbott,DEV-1,CRP,Cycle-2,2026-01-02T00:00:00Z,12.0,mg/L\n",
    );
    let err = load_csv(&csv).unwrap_err();
    let message = err.to_string();
    assert!(message.contains("header"));
    assert!(!message.contains("Site A"));
}

#[test]
fn nonfinite_values_are_rejected() {
    let dir = unique_dir("nonfinite");
    let csv = write_csv(
        &dir,
        "nonfinite.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,NaN,mg/L\nSite A,Abbott,DEV-1,CRP,Cycle-2,2026-01-02T00:00:00Z,12.0,mg/L\n",
    );
    let err = load_csv(&csv).unwrap_err();
    assert!(err.to_string().to_lowercase().contains("finite"));
}

#[test]
fn header_only_csv_is_rejected() {
    let dir = unique_dir("header-only");
    let csv = write_csv(
        &dir,
        "empty.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\n",
    );
    assert!(load_csv(&csv)
        .unwrap_err()
        .to_string()
        .contains("no submissions"));
}

#[test]
fn multiple_assays_are_rejected() {
    let dir = unique_dir("multiple-assays");
    let csv = write_csv(
        &dir,
        "multiple.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10,mg/L\nSite A,Abbott,DEV-1,Troponin,Cycle-2,2026-01-02T00:00:00Z,20,mg/L\n",
    );
    assert!(load_csv(&csv)
        .unwrap_err()
        .to_string()
        .contains("one assay"));
}

#[test]
fn more_than_two_cycles_are_rejected() {
    let dir = unique_dir("three-cycles");
    let csv = write_csv(
        &dir,
        "cycles.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10,mg/L\nSite A,Abbott,DEV-1,CRP,Cycle-2,2026-01-02T00:00:00Z,20,mg/L\nSite A,Abbott,DEV-1,CRP,Cycle-3,2026-01-03T00:00:00Z,30,mg/L\n",
    );
    assert!(load_csv(&csv).unwrap_err().to_string().contains("two"));
}

#[test]
fn inconsistent_site_metadata_is_rejected() {
    let dir = unique_dir("mixed-metadata");
    let csv = write_csv(
        &dir,
        "mixed.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10.5,mg/L\nSite A,Roche,DEV-1,CRP,Cycle-2,2026-01-02T00:00:00Z,12.0,mg/L\n",
    );
    let err = load_csv(&csv).unwrap_err();
    assert!(err.to_string().contains("metadata"));
}

#[test]
fn every_site_datetime_is_preserved_in_the_report() {
    let dir = unique_dir("multiple-datetimes");
    let csv = write_csv(
        &dir,
        "multiple.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10.5,mg/L\nSite A,Abbott,DEV-1,CRP,Cycle-2,2026-01-02T00:00:00Z,12.0,mg/L\n",
    );
    let report = String::from_utf8(render_report_bytes(&load_csv(&csv).unwrap()).unwrap()).unwrap();
    assert!(report.contains("\\item Date/time values:"));
    assert!(report.contains("\\item 2026-01-01T00:00:00Z"));
    assert!(report.contains("\\item 2026-01-02T00:00:00Z"));
}

#[test]
fn missing_site_cycle_is_rejected() {
    let dir = unique_dir("missing-cycle");
    let csv = write_csv(
        &dir,
        "missing.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10.5,mg/L\nSite B,Abbott,DEV-2,CRP,Cycle-1,2026-01-01T00:00:00Z,11.5,mg/L\nSite B,Abbott,DEV-2,CRP,Cycle-2,2026-01-02T00:00:00Z,12.0,mg/L\n",
    );
    let err = load_csv(&csv).unwrap_err();
    assert!(err.to_string().contains("both cycles"));
}

#[test]
fn text_fields_are_escaped_and_output_filename_is_safe() {
    let dir = unique_dir("escape");
    let csv = write_csv(&dir, "input.csv", &synthetic_csv());
    let output_dir = dir.join("reports");

    let reports = generate_reports(&csv, &output_dir).unwrap();
    assert_eq!(reports.len(), 1);
    let report_path = &reports[0].path;
    assert_eq!(report_path.parent().unwrap(), output_dir.as_path());
    let file_name = report_path.file_name().unwrap().to_string_lossy();
    assert!(!file_name.contains('/'));
    assert!(!file_name.contains('\\'));
    assert!(!file_name.contains(".."));

    let bytes = fs::read(report_path).unwrap();
    let expected = render_report_bytes(&load_csv(&csv).unwrap()).unwrap();
    assert_eq!(bytes, expected);
    let tex = String::from_utf8(bytes).unwrap();
    assert!(tex.contains(r"Site \{Alpha\}\textbackslash{}Path"));
    assert!(tex.contains(r"\_") || !tex.contains('_'));
    assert!(tex.ends_with("\n"));
}

#[test]
fn output_directory_name_is_deterministic() {
    let name = make_output_directory_name("CRP", "Cycle-1", "Cycle-2");
    assert_eq!(name, "Output_CRP_Cycle-1_Cycle-2");
}

#[test]
fn traversal_components_are_removed_from_filenames() {
    assert_eq!(sanitize_path_component("../../"), "report");
    assert_eq!(sanitize_path_component("site..name"), "site_name");
}

#[test]
fn sanitized_site_name_collisions_receive_stable_suffixes() {
    let dir = unique_dir("filename-collision");
    let csv = write_csv(
        &dir,
        "input.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nA/B,Abbott,DEV-1,CRP,C1,2026-01-01,10,mg/L\nA/B,Abbott,DEV-1,CRP,C2,2026-01-02,11,mg/L\nA?B,Abbott,DEV-2,CRP,C1,2026-01-01,12,mg/L\nA?B,Abbott,DEV-2,CRP,C2,2026-01-02,13,mg/L\n",
    );
    let reports = generate_reports(&csv, &dir.join("reports")).unwrap();
    let names: Vec<_> = reports
        .iter()
        .map(|report| {
            report
                .path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    assert_eq!(names, ["A_B_CRP_C1_C2.tex", "A_B_CRP_C1_C2_2.tex"]);
}

#[test]
fn report_matches_reviewed_golden_bytes() {
    let dir = unique_dir("golden");
    let csv = write_csv(&dir, "input.csv", &synthetic_csv());
    let actual = render_report_bytes(&load_csv(&csv).unwrap()).unwrap();
    assert_eq!(actual, include_bytes!("golden/synthetic_report.tex"));
}

#[test]
fn generation_refuses_to_overwrite_an_existing_report() {
    let dir = unique_dir("no-clobber");
    let csv = write_csv(&dir, "input.csv", &synthetic_csv());
    let output_dir = dir.join("reports");
    let first = generate_reports(&csv, &output_dir).unwrap();
    let original = fs::read(&first[0].path).unwrap();

    let error = generate_reports(&csv, &output_dir).unwrap_err();
    assert!(error.to_string().contains("already exists"));
    assert_eq!(fs::read(&first[0].path).unwrap(), original);
}

#[test]
fn later_existing_target_leaves_no_new_partial_reports() {
    let dir = unique_dir("atomic-output");
    let csv = write_csv(
        &dir,
        "input.csv",
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,C1,2026-01-01,10,mg/L\nSite A,Abbott,DEV-1,CRP,C2,2026-01-01,11,mg/L\nSite B,Abbott,DEV-2,CRP,C1,2026-01-02,12,mg/L\nSite B,Abbott,DEV-2,CRP,C2,2026-01-02,13,mg/L\n",
    );
    let output_dir = dir.join("reports");
    fs::create_dir_all(&output_dir).unwrap();
    let later_target = output_dir.join("Site_B_CRP_C1_C2.tex");
    fs::write(&later_target, b"pre-existing").unwrap();

    let error = generate_reports(&csv, &output_dir).unwrap_err();
    assert!(error.to_string().contains("already exists"));
    assert!(!output_dir.join("Site_A_CRP_C1_C2.tex").exists());
    assert_eq!(fs::read(&later_target).unwrap(), b"pre-existing");
}

#[test]
fn allowable_ranges_use_the_whole_cycle_peer_mean() {
    let data = load_csv_from_text(
        "site,device_type,device_id,test,cycle,datetime,value,units\nSite A,Abbott,DEV-1,CRP,Cycle-1,2026-01-01T00:00:00Z,10.0,mg/L\nSite A,Abbott,DEV-1,CRP,Cycle-2,2026-01-02T00:00:00Z,20.0,mg/L\nSite B,Abbott,DEV-2,CRP,Cycle-1,2026-01-01T00:00:00Z,30.0,mg/L\nSite B,Abbott,DEV-2,CRP,Cycle-2,2026-01-02T00:00:00Z,40.0,mg/L\n",
    )
    .unwrap();
    let rendered = String::from_utf8(render_report_bytes(&data).unwrap()).unwrap();
    assert!(rendered.contains("Cycle-1 allowable range: 19--21"));
    assert!(rendered.contains("Cycle-2 allowable range: 28.5--31.5"));
}

fn load_csv_from_text(text: &str) -> Result<qapsimple::ValidatedDataset, qapsimple::ReportError> {
    let dir = unique_dir("load-text");
    let csv = write_csv(&dir, "input.csv", text);
    load_csv(&csv)
}

#[test]
fn run_from_args_supports_output_flag() {
    let dir = unique_dir("cli-output");
    let csv = write_csv(&dir, "input.csv", &synthetic_csv());
    let out = dir.join("generated");
    let args = vec![
        "qapsimple".to_string(),
        csv.to_string_lossy().into_owned(),
        "--output".to_string(),
        out.to_string_lossy().into_owned(),
    ];
    let result = run_from_args(&args);
    assert!(result.is_ok());
}
