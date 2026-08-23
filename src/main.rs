use std::error::Error;
use std::path::PathBuf;

use qapsimple::pipeline::{load_csv, make_output_directory_name, parse_cli_args, usage};
use qapsimple::reporting::generate_reports;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    match parse_cli_args(&args)? {
        Some(config) => {
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
            let reports = generate_reports(&config.csv_path, &output_dir)?;
            println!(
                "Wrote {} report(s) to {}",
                reports.len(),
                output_dir.display()
            );
            Ok(())
        }
        None => {
            println!("{}", usage());
            Ok(())
        }
    }
}
