use crate::error::ReportError;
use crate::models::{Assay, DeviceType, SiteReportInput, Submission, ValidatedDataset};
use csv::{ReaderBuilder, StringRecord};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const CSV_HEADERS: [&str; 8] = [
    "site",
    "device_type",
    "device_id",
    "test",
    "cycle",
    "datetime",
    "value",
    "units",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliConfig {
    pub csv_path: PathBuf,
    pub output_dir: Option<PathBuf>,
}

pub fn usage() -> &'static str {
    "Usage: qapsimple <CSV_PATH> [--output DIR]\n       qapsimple --help"
}

pub fn parse_cli_args(args: &[String]) -> Result<Option<CliConfig>, ReportError> {
    let program = args.first().map(String::as_str).unwrap_or("qapsimple");
    match args.len() {
        0 => Err(ReportError::usage(usage())),
        1 => Err(ReportError::usage(usage_message(program))),
        2 if args[1] == "--help" || args[1] == "-h" => Ok(None),
        2 => Ok(Some(CliConfig {
            csv_path: PathBuf::from(&args[1]),
            output_dir: None,
        })),
        4 if args[2] == "--output" => Ok(Some(CliConfig {
            csv_path: PathBuf::from(&args[1]),
            output_dir: Some(PathBuf::from(&args[3])),
        })),
        _ => Err(ReportError::usage(usage_message(program))),
    }
}

pub fn usage_message(program: &str) -> String {
    format!("{}\n", usage().replace("qapsimple", program))
}

pub fn run_from_args(args: &[String]) -> Result<Vec<PathBuf>, ReportError> {
    let config = parse_cli_args(args)?.ok_or_else(|| ReportError::usage(usage().to_string()))?;
    let output_dir = match config.output_dir {
        Some(dir) => dir,
        None => {
            let dataset = load_csv(&config.csv_path)?;
            PathBuf::from(make_output_directory_name(
                &dataset.assay.to_string(),
                &dataset.cycles[0],
                &dataset.cycles[1],
            ))
        }
    };
    let reports = crate::reporting::generate_reports(&config.csv_path, &output_dir)?;
    Ok(reports.into_iter().map(|report| report.path).collect())
}

pub fn default_output_dir_name(csv_path: &Path) -> String {
    let stem = csv_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("reports");
    sanitize_path_component(stem)
}

pub fn make_output_directory_name(assay: &str, cycle1: &str, cycle2: &str) -> String {
    format!(
        "Output_{}_{}_{}",
        sanitize_path_component(assay),
        sanitize_path_component(cycle1),
        sanitize_path_component(cycle2)
    )
}

pub fn sanitize_path_component(input: &str) -> String {
    let mut out = String::new();
    let mut prev_underscore = false;
    for ch in input.chars() {
        let keep = ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_');
        let mapped = if keep { ch } else { '_' };
        if mapped == '_' {
            if prev_underscore {
                continue;
            }
            prev_underscore = true;
        } else {
            prev_underscore = false;
        }
        out.push(mapped);
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "report".to_string()
    } else {
        trimmed
    }
}

pub fn load_csv(path: &Path) -> Result<ValidatedDataset, ReportError> {
    if fs::metadata(path)
        .map_err(|_| ReportError::io("unable to read CSV file"))?
        .len()
        == 0
    {
        return Err(ReportError::validation("CSV file is empty"));
    }

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|_| ReportError::csv("unable to parse CSV file"))?;

    validate_headers(
        reader
            .headers()
            .map_err(|_| ReportError::csv("unable to read CSV header"))?,
    )?;

    let mut records = Vec::new();
    for (index, record) in reader.records().enumerate() {
        let record = record.map_err(|_| ReportError::csv("unable to read CSV row"))?;
        records.push(parse_submission_row(&record, index + 2)?);
    }

    validate_records(records)
}

fn validate_headers(headers: &StringRecord) -> Result<(), ReportError> {
    if headers.len() != CSV_HEADERS.len() {
        return Err(ReportError::validation(
            "CSV header does not match the required schema",
        ));
    }
    if headers
        .iter()
        .zip(CSV_HEADERS)
        .all(|(actual, expected)| actual == expected)
    {
        Ok(())
    } else {
        Err(ReportError::validation(
            "CSV header does not match the required schema",
        ))
    }
}

fn parse_submission_row(
    record: &StringRecord,
    row_number: usize,
) -> Result<Submission, ReportError> {
    if record.len() != CSV_HEADERS.len() {
        return Err(ReportError::validation(format!(
            "row {row_number} does not match the required schema"
        )));
    }

    let site = trimmed_field(record, 0, row_number, "site")?;
    let device_type = parse_device_type(trimmed_field(record, 1, row_number, "device type")?)?;
    let device_id = trimmed_field(record, 2, row_number, "device id")?;
    let test = parse_assay(trimmed_field(record, 3, row_number, "assay")?)?;
    let cycle = trimmed_field(record, 4, row_number, "cycle")?;
    let datetime = trimmed_field(record, 5, row_number, "datetime")?;
    let value_text = trimmed_field(record, 6, row_number, "value")?;
    let value = parse_finite_f64(&value_text, row_number)?;
    let units = trimmed_field(record, 7, row_number, "units")?;

    Ok(Submission {
        site,
        devicetype: device_type,
        deviceid: device_id,
        test,
        cycle,
        datetime,
        value,
        units,
    })
}

fn trimmed_field(
    record: &StringRecord,
    index: usize,
    row_number: usize,
    field_name: &str,
) -> Result<String, ReportError> {
    let value = record
        .get(index)
        .ok_or_else(|| {
            ReportError::validation(format!("row {row_number} is missing a {field_name} field"))
        })?
        .trim();
    if value.is_empty() {
        Err(ReportError::validation(format!(
            "row {row_number} contains an empty {field_name} field"
        )))
    } else {
        Ok(value.to_string())
    }
}

fn parse_finite_f64(value: &str, row_number: usize) -> Result<f64, ReportError> {
    let parsed = value.parse::<f64>().map_err(|_| {
        ReportError::validation(format!(
            "row {row_number} contains an invalid numeric value"
        ))
    })?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(ReportError::validation(format!(
            "row {row_number} contains a non-finite numeric value"
        )))
    }
}

fn parse_device_type(value: String) -> Result<DeviceType, ReportError> {
    match value.as_str() {
        "Abbott" => Ok(DeviceType::Abbott),
        "Roche" => Ok(DeviceType::Roche),
        "Siemens" => Ok(DeviceType::Siemens),
        "Radiometer" => Ok(DeviceType::Radiometer),
        "Nova" => Ok(DeviceType::Nova),
        "IStat" => Ok(DeviceType::IStat),
        "Alere" => Ok(DeviceType::Alere),
        "Beckman" => Ok(DeviceType::Beckman),
        "Sysmex" => Ok(DeviceType::Sysmex),
        "Other" => Ok(DeviceType::Other),
        "cobas h 232" => Ok(DeviceType::CobasH232),
        _ => Err(ReportError::validation(
            "CSV contains an unsupported device type",
        )),
    }
}

fn parse_assay(value: String) -> Result<Assay, ReportError> {
    match value.as_str() {
        "Troponin" => Ok(Assay::Troponin),
        "CRP" => Ok(Assay::CRP),
        "Lactate" => Ok(Assay::Lactate),
        "Creatinine" => Ok(Assay::Creatinine),
        "Potassium" => Ok(Assay::Potassium),
        "Sodium" => Ok(Assay::Sodium),
        "Chloride" => Ok(Assay::Chloride),
        "Bicarbonate" => Ok(Assay::Bicarbonate),
        "Glucose" => Ok(Assay::Glucose),
        "Hemoglobin" => Ok(Assay::Hemoglobin),
        "WBC" => Ok(Assay::WBC),
        "INR" => Ok(Assay::INR),
        "Bilirubin" => Ok(Assay::Bilirubin),
        "Calcium" => Ok(Assay::Calcium),
        "Magnesium" => Ok(Assay::Magnesium),
        "Phosphorus" => Ok(Assay::Phosphorus),
        "NtProBNP" => Ok(Assay::NtProBNP),
        _ => Err(ReportError::validation("CSV contains an unsupported assay")),
    }
}

fn validate_records(records: Vec<Submission>) -> Result<ValidatedDataset, ReportError> {
    if records.is_empty() {
        return Err(ReportError::validation("CSV contains no submissions"));
    }

    let mut assays = BTreeSet::new();
    let mut cycles = BTreeSet::new();
    let mut cycle_values: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    let mut sites: BTreeMap<String, SiteAccumulator> = BTreeMap::new();

    for record in records {
        assays.insert(record.test.clone());
        cycles.insert(record.cycle.clone());
        cycle_values
            .entry(record.cycle.clone())
            .or_default()
            .push(record.value);

        let entry = sites
            .entry(record.site.clone())
            .or_insert_with(|| SiteAccumulator {
                site: record.site.clone(),
                devicetype: record.devicetype.clone(),
                deviceid: record.deviceid.clone(),
                units: record.units.clone(),
                datetimes: BTreeSet::from([record.datetime.clone()]),
                cycle_values: BTreeMap::new(),
            });

        if entry.devicetype != record.devicetype
            || entry.deviceid != record.deviceid
            || entry.units != record.units
        {
            return Err(ReportError::validation(
                "CSV contains inconsistent device metadata for a site",
            ));
        }

        entry.datetimes.insert(record.datetime.clone());
        entry
            .cycle_values
            .entry(record.cycle.clone())
            .or_default()
            .push(record.value);
    }

    if assays.len() != 1 {
        return Err(ReportError::validation(
            "CSV must contain exactly one assay",
        ));
    }
    if cycles.len() != 2 {
        return Err(ReportError::validation(
            "CSV must contain exactly two non-empty cycles",
        ));
    }

    let cycles: Vec<String> = cycles.into_iter().collect();
    let cycle1 = cycles[0].clone();
    let cycle2 = cycles[1].clone();

    let mut site_inputs = Vec::with_capacity(sites.len());
    for site in sites.into_values() {
        let values1 = site.cycle_values.get(&cycle1).cloned().unwrap_or_default();
        let values2 = site.cycle_values.get(&cycle2).cloned().unwrap_or_default();
        if values1.is_empty() || values2.is_empty() {
            return Err(ReportError::validation(
                "each site must contain both cycles",
            ));
        }

        site_inputs.push(SiteReportInput {
            site: site.site,
            devicetype: site.devicetype,
            deviceid: site.deviceid,
            datetimes: site.datetimes.into_iter().collect(),
            units: site.units,
            cycle_values: [values1.clone(), values2.clone()],
            cycle_means: [mean(&values1)?, mean(&values2)?],
        });
    }

    site_inputs.sort_by(|left, right| left.site.cmp(&right.site));

    let cycle_values = [
        cycle_values.get(&cycle1).cloned().unwrap_or_default(),
        cycle_values.get(&cycle2).cloned().unwrap_or_default(),
    ];
    let cycle_means = [mean(&cycle_values[0])?, mean(&cycle_values[1])?];

    Ok(ValidatedDataset {
        assay: assays.into_iter().next().expect("assay set is non-empty"),
        cycles: [cycle1, cycle2],
        sites: site_inputs,
        cycle_values,
        cycle_means,
    })
}

fn mean(values: &[f64]) -> Result<f64, ReportError> {
    if values.is_empty() {
        return Err(ReportError::validation(
            "cannot calculate a mean for an empty cycle",
        ));
    }
    Ok(values.iter().sum::<f64>() / values.len() as f64)
}

#[derive(Debug)]
struct SiteAccumulator {
    site: String,
    devicetype: DeviceType,
    deviceid: String,
    units: String,
    datetimes: BTreeSet<String>,
    cycle_values: BTreeMap<String, Vec<f64>>,
}
