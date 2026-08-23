use std::fmt::{Display, Formatter};

#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Assay {
    Troponin,
    CRP,
    Lactate,
    Creatinine,
    Potassium,
    Sodium,
    Chloride,
    Bicarbonate,
    Glucose,
    Hemoglobin,
    WBC,
    INR,
    Bilirubin,
    Calcium,
    Magnesium,
    Phosphorus,
    NtProBNP,
}

impl Display for Assay {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Troponin => "Troponin",
            Self::CRP => "CRP",
            Self::Lactate => "Lactate",
            Self::Creatinine => "Creatinine",
            Self::Potassium => "Potassium",
            Self::Sodium => "Sodium",
            Self::Chloride => "Chloride",
            Self::Bicarbonate => "Bicarbonate",
            Self::Glucose => "Glucose",
            Self::Hemoglobin => "Hemoglobin",
            Self::WBC => "WBC",
            Self::INR => "INR",
            Self::Bilirubin => "Bilirubin",
            Self::Calcium => "Calcium",
            Self::Magnesium => "Magnesium",
            Self::Phosphorus => "Phosphorus",
            Self::NtProBNP => "NtProBNP",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeviceType {
    Abbott,
    CobasH232,
    Roche,
    Siemens,
    Radiometer,
    Nova,
    IStat,
    Alere,
    Beckman,
    Sysmex,
    Other,
}

impl Display for DeviceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Abbott => "Abbott",
            Self::CobasH232 => "cobas h 232",
            Self::Roche => "Roche",
            Self::Siemens => "Siemens",
            Self::Radiometer => "Radiometer",
            Self::Nova => "Nova",
            Self::IStat => "IStat",
            Self::Alere => "Alere",
            Self::Beckman => "Beckman",
            Self::Sysmex => "Sysmex",
            Self::Other => "Other",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Submission {
    pub site: String,
    pub devicetype: DeviceType,
    pub deviceid: String,
    pub test: Assay,
    pub cycle: String,
    pub datetime: String,
    pub value: f64,
    pub units: String,
}

#[derive(Debug, Clone)]
pub struct SiteReportInput {
    pub site: String,
    pub devicetype: DeviceType,
    pub deviceid: String,
    pub datetimes: Vec<String>,
    pub units: String,
    pub cycle_values: [Vec<f64>; 2],
    pub cycle_means: [f64; 2],
}

#[derive(Debug, Clone)]
pub struct ValidatedDataset {
    pub assay: Assay,
    pub cycles: [String; 2],
    pub sites: Vec<SiteReportInput>,
    pub cycle_values: [Vec<f64>; 2],
    pub cycle_means: [f64; 2],
}

#[derive(Debug, Clone)]
pub struct GeneratedReport {
    pub site: String,
    pub path: std::path::PathBuf,
    pub contents: Vec<u8>,
}
