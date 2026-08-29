# qaprs

`qaprs` is a small Rust CLI that validates a single-analyte quality-assurance CSV and writes one LaTeX report per site.

## CLI

```bash
cargo run -- <CSV_PATH>
cargo run -- <CSV_PATH> --output reports/
cargo run -- --help
```

Arguments:
- `<CSV_PATH>`: required input CSV path
- `--output DIR`: optional output directory; defaults to a deterministic directory name derived from the assay and cycle names
- `--help`: prints usage and exits successfully

Invalid, missing, or extra arguments fail with a concise usage message.

## Input contract

The CSV must:

- contain exactly this header, in this order:

  `site,device_type,device_id,test,cycle,datetime,value,units`

- be non-empty
- contain non-empty trimmed values for `site`, `device_id`, `cycle`, `datetime`, and `units`
- contain finite numeric `value` fields only (`NaN` and infinities are rejected)
- contain exactly one assay
- contain exactly two non-empty cycles
- give every site both cycles
- keep `device_type`, `device_id`, and `units` consistent within each site

Assay and device-type names are case-sensitive and must match the variants accepted in `src/pipeline.rs`.
Sites and cycle labels are ordered deterministically. Measurement rows retain their CSV order within each cycle.

## Output

- A `.tex` file is written for each site
- Output filenames are sanitized so path separators, traversal sequences, and control characters cannot escape the output directory
- Filename collisions within a run are resolved deterministically with numeric suffixes
- Existing report files are never overwritten; generation fails closed if a target already exists
- Report text escapes user-controlled LaTeX content, including backslash, braces, dollar, ampersand, hash, underscore, percent, tilde, and caret
- Template loading is embedded at compile time; the current working directory does not matter

## Supported runtime

The repository targets stable Rust with the Rust 2021 edition, as declared by `Cargo.toml` and the verification workflow. Report generation writes LaTeX source; compiling that source to PDF requires a separately installed LaTeX distribution, which this project does not provide or pin.

## Release policy

This project has no tags or releases in this local release-hygiene pass. A future release requires a clean clone, passing CI and the verification commands below before creating a tag; version numbers are changed only as part of an intentional release.

## Verification

```bash
cargo fmt --check
cargo check
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings

git diff --check
```

## Limitations

The generated LaTeX is for QA/reporting validation only. It is not a clinical decision-support tool and does not make unsupported clinical claims.

## Licence

MIT.
