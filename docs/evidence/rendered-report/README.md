# Reproducible rendered-report evidence

This directory contains one genuine rendered report produced from the committed synthetic CSV. The PNG is an actual `pdftoppm` conversion of the PDF; it is not a screenshot or mock.

## Generate

The target must be a fresh, existing directory containing a regular, non-symlink `synthetic_input.csv` directly beneath it. Create a new directory for every regeneration; the script refuses to overwrite any existing `report.tex`, `report.pdf`, or `report.png` (including symlinks).

From the repository root, with Tectonic available as `tectonic` (or set `TECTONIC_BIN` to its executable):

```bash
mkdir -p /tmp/qaprs-evidence
cp docs/evidence/rendered-report/synthetic_input.csv /tmp/qaprs-evidence/
./scripts/generate_visual_evidence.sh /tmp/qaprs-evidence
```

To use the committed evidence directory, it must already contain the committed input and outputs are therefore not regenerated in place. Use a fresh target directory to regenerate instead.

The script reads only the non-symlink `synthetic_input.csv` directly inside the supplied evidence directory, invokes the qaprs CLI, and renders with:

```bash
SOURCE_DATE_EPOCH=0 "$TECTONIC_BIN" --outdir WORK/pdf --keep-logs WORK/report.tex
pdftoppm -png -singlefile WORK/pdf/report.pdf WORK/report
```

Temporary files are created and removed only below the supplied evidence directory, and final publication is no-clobber. Publication uses same-filesystem hard-link creation with GNU `ln -T`, so each final artifact is either absent or created without overwriting or following an existing destination directory. If a later publication step fails, the script fails non-zero and reports every final path it created; it deliberately does **not** delete those paths, because pathname rollback could delete a concurrent replacement. Staging cleanup is also fail-closed: identity collection must succeed with non-empty device/inode values, and an identity mismatch or cleanup error is reported with the staging path and makes the command fail. The wrapper resolves the target once and checks staging-directory identity, but it cannot provide descriptor-level locking in portable POSIX shell; do not concurrently rename/replace the target or staging tree during a run. No pre-existing target/input path is overwritten or recursively deleted. No historical or implicit input is used. `SOURCE_DATE_EPOCH=0` makes repeated runs deterministic on the same local qaprs/Tectonic bundle/font toolchain; PDF and PNG bytes may vary across hosts or toolchain/font versions. The source CSV, `.tex`, PDF, and PNG are deliberately date-free at report-generation level.

## Interpretation

The report demonstrates CSV validation, LaTeX generation, PDF rendering, and PDF-to-PNG conversion behavior. All values and identifiers are synthetic. It does **not** demonstrate clinical validity, diagnostic performance, or suitability for clinical use.

The PDF is expected to be a valid, unencrypted, text-extractable document with no JavaScript. Captions and documentation intentionally contain no generation date.
