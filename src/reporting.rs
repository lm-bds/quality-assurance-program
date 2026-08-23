use crate::error::ReportError;
use crate::models::{Assay, GeneratedReport, SiteReportInput, ValidatedDataset};
use crate::pipeline::sanitize_path_component;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;

pub fn generate_reports(
    csv_path: &Path,
    output_dir: &Path,
) -> Result<Vec<GeneratedReport>, ReportError> {
    let dataset = crate::pipeline::load_csv(csv_path)?;
    fs::create_dir_all(output_dir)
        .map_err(|_| ReportError::io("unable to create output directory"))?;

    let mut used_names = BTreeSet::new();
    let mut reports = Vec::with_capacity(dataset.sites.len());
    for site in &dataset.sites {
        let contents = render_site_report(&dataset, site);
        let file_name =
            unique_report_filename(&dataset.assay, &dataset.cycles, site, &mut used_names);
        let path = output_dir.join(&file_name);
        reports.push(GeneratedReport {
            site: site.site.clone(),
            path,
            contents,
        });
    }

    for report in &reports {
        if report
            .path
            .try_exists()
            .map_err(|_| ReportError::io("unable to inspect report output path"))?
        {
            return Err(ReportError::io("report file already exists"));
        }
    }

    let mut created_paths = Vec::with_capacity(reports.len());
    for report in &reports {
        let mut output = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&report.path)
        {
            Ok(output) => output,
            Err(error) => {
                remove_created_reports(&created_paths);
                return Err(if error.kind() == ErrorKind::AlreadyExists {
                    ReportError::io("report file already exists")
                } else {
                    ReportError::io("unable to write report file")
                });
            }
        };
        created_paths.push(report.path.clone());
        if output.write_all(&report.contents).is_err() {
            remove_created_reports(&created_paths);
            return Err(ReportError::io("unable to write report file"));
        }
    }

    Ok(reports)
}

fn remove_created_reports(paths: &[std::path::PathBuf]) {
    for path in paths {
        let _ = fs::remove_file(path);
    }
}

pub fn render_report_bytes(dataset: &ValidatedDataset) -> Result<Vec<u8>, ReportError> {
    let site = dataset
        .sites
        .first()
        .ok_or_else(|| ReportError::validation("CSV contains no reportable sites"))?;
    Ok(render_site_report(dataset, site))
}

fn render_site_report(dataset: &ValidatedDataset, site: &SiteReportInput) -> Vec<u8> {
    let template = include_str!("../Template/template.tex");
    let cycle1 = &dataset.cycles[0];
    let cycle2 = &dataset.cycles[1];
    let group1_values = format_number_list(&dataset.cycle_values[0]);
    let group2_values = format_number_list(&dataset.cycle_values[1]);
    let mut filled = template.to_string();
    let replacements = [
        ("{{site}}", escape_tex(&site.site)),
        ("{{devicetype}}", escape_tex(&site.devicetype.to_string())),
        ("{{deviceid}}", escape_tex(&site.deviceid)),
        ("{{assay}}", escape_tex(&dataset.assay.to_string())),
        ("{{cycle1}}", escape_tex(cycle1)),
        ("{{cycle2}}", escape_tex(cycle2)),
        (
            "{{datetimes}}",
            site.datetimes
                .iter()
                .map(|datetime| format!("  \\item {}", escape_tex(datetime)))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        ("{{value1}}", format_number(site.cycle_means[0])),
        (
            "{{lower1}}",
            format_number(lower_bound(dataset.assay.clone(), dataset.cycle_means[0])),
        ),
        (
            "{{upper1}}",
            format_number(upper_bound(dataset.assay.clone(), dataset.cycle_means[0])),
        ),
        ("{{value2}}", format_number(site.cycle_means[1])),
        (
            "{{lower2}}",
            format_number(lower_bound(dataset.assay.clone(), dataset.cycle_means[1])),
        ),
        (
            "{{upper2}}",
            format_number(upper_bound(dataset.assay.clone(), dataset.cycle_means[1])),
        ),
        ("{{units}}", escape_tex(&site.units)),
        ("{{group1values}}", group1_values),
        ("{{group2values}}", group2_values),
        (
            "{{table}}",
            render_table(&dataset.cycles, &dataset.cycle_values),
        ),
    ];

    for (needle, replacement) in replacements {
        filled = filled.replace(needle, &replacement);
    }

    filled.into_bytes()
}

fn render_table(cycles: &[String; 2], cycle_values: &[Vec<f64>; 2]) -> String {
    let max_rows = cycle_values[0].len().max(cycle_values[1].len());
    let mut table = String::from(
        r"\begin{tabular}{lr}
Cycle & Value \\ \hline
",
    );
    for index in 0..max_rows {
        let left = cycle_values[0]
            .get(index)
            .map(|value| format_number(*value))
            .unwrap_or_default();
        let right = cycle_values[1]
            .get(index)
            .map(|value| format_number(*value))
            .unwrap_or_default();
        if !left.is_empty() {
            table.push_str(&format!(r"{} & {} \\", escape_tex(&cycles[0]), left));
            table.push('\n');
        }
        if !right.is_empty() {
            table.push_str(&format!(r"{} & {} \\", escape_tex(&cycles[1]), right));
            table.push('\n');
        }
    }
    table.push_str("\\end{tabular}");
    table
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        let mut text = format!("{value:.6}");
        while text.contains('.') && text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
        text
    }
}

fn format_number_list(values: &[f64]) -> String {
    values
        .iter()
        .map(|value| format_number(*value))
        .collect::<Vec<_>>()
        .join(", ")
}

fn escape_tex(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => escaped.push_str(r"\textbackslash{}"),
            '{' => escaped.push_str(r"\{"),
            '}' => escaped.push_str(r"\}"),
            '$' => escaped.push_str(r"\$"),
            '&' => escaped.push_str(r"\&"),
            '#' => escaped.push_str(r"\#"),
            '_' => escaped.push_str(r"\_"),
            '%' => escaped.push_str(r"\%"),
            '~' => escaped.push_str(r"\textasciitilde{}"),
            '^' => escaped.push_str(r"\textasciicircum{}"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn unique_report_filename(
    assay: &Assay,
    cycles: &[String; 2],
    site: &SiteReportInput,
    used_names: &mut BTreeSet<String>,
) -> String {
    let base = format!(
        "{}_{}_{}_{}.tex",
        sanitize_path_component(&site.site),
        sanitize_path_component(&assay.to_string()),
        sanitize_path_component(&cycles[0]),
        sanitize_path_component(&cycles[1])
    );
    if used_names.insert(base.clone()) {
        return base;
    }

    let mut suffix = 2usize;
    loop {
        let candidate = format!("{}_{}.tex", base.trim_end_matches(".tex"), suffix);
        if used_names.insert(candidate.clone()) {
            return candidate;
        }
        suffix += 1;
    }
}

fn lower_bound(assay: Assay, mean: f64) -> f64 {
    allowable_range(mean, assay).0
}

fn upper_bound(assay: Assay, mean: f64) -> f64 {
    allowable_range(mean, assay).1
}

pub fn allowable_range(mean: f64, assay: Assay) -> (f64, f64) {
    let (lower, upper) = match assay {
        Assay::Troponin if mean < 50.0 => (mean - 10.0, mean + 10.0),
        Assay::Troponin => (mean * 0.9, mean * 1.1),
        _ => (mean * 0.95, mean * 1.05),
    };
    (lower, upper)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{Assay, DeviceType, SiteReportInput, ValidatedDataset};

    #[test]
    fn escapes_special_tex_characters() {
        assert_eq!(
            escape_tex(r"\{}$&#_%~^"),
            r"\textbackslash{}\{\}\$\&\#\_\%\textasciitilde{}\textasciicircum{}"
        );
    }

    #[test]
    fn allowable_range_is_lower_upper_ordered() {
        assert_eq!(allowable_range(100.0, Assay::CRP), (95.0, 105.0));
    }

    #[test]
    fn mean_range_uses_semantically_ordered_bounds() {
        assert_eq!(lower_bound(Assay::CRP, 100.0), 95.0);
        assert_eq!(upper_bound(Assay::CRP, 100.0), 105.0);
    }

    #[test]
    fn deterministic_filename_collisions_get_suffixes() {
        let mut used = BTreeSet::new();
        let site = SiteReportInput {
            site: "A/B".into(),
            devicetype: DeviceType::Abbott,
            deviceid: "1".into(),
            datetimes: vec!["2026".into()],
            units: "mg/L".into(),
            cycle_values: [vec![1.0], vec![2.0]],
            cycle_means: [1.0, 2.0],
        };
        let first =
            unique_report_filename(&Assay::CRP, &["C1".into(), "C2".into()], &site, &mut used);
        let second =
            unique_report_filename(&Assay::CRP, &["C1".into(), "C2".into()], &site, &mut used);
        assert_eq!(first, "A_B_CRP_C1_C2.tex");
        assert_eq!(second, "A_B_CRP_C1_C2_2.tex");
    }

    #[test]
    fn render_report_has_template_tokens_resolved() {
        let dataset = ValidatedDataset {
            assay: Assay::CRP,
            cycles: ["C1".into(), "C2".into()],
            sites: vec![SiteReportInput {
                site: "A".into(),
                devicetype: DeviceType::Abbott,
                deviceid: "1".into(),
                datetimes: vec!["2026".into()],
                units: "mg/L".into(),
                cycle_values: [vec![1.0], vec![2.0]],
                cycle_means: [1.0, 2.0],
            }],
            cycle_values: [vec![1.0], vec![2.0]],
            cycle_means: [1.0, 2.0],
        };
        let bytes = render_report_bytes(&dataset).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.contains("\\end{document}"));
        assert!(!text.contains("{{"));
    }
}
