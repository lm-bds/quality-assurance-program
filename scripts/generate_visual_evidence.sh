#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
target_arg=${1:-"$repo_root/docs/evidence/rendered-report"}

# The evidence root itself must be an existing, real directory.  Resolve its
# physical path once so all later workspace operations stay below that root.
if [[ -L "$target_arg" || ! -d "$target_arg" ]]; then
  printf '%s\n' "evidence directory must be an existing non-symlink directory" >&2
  exit 1
fi
target=$(CDPATH= cd -- "$target_arg" && pwd -P)
input="$target/synthetic_input.csv"

# Do not follow an input link, even if it ultimately points at a regular file.
if [[ -L "$input" || ! -f "$input" ]]; then
  printf '%s\n' "synthetic input must be a regular file directly under evidence directory" >&2
  exit 1
fi

# Refuse every collision, including dangling symlinks.  This is deliberately
# done before creating the private workspace or invoking the generator.
for artifact in report.tex report.pdf report.png; do
  if [[ -e "$target/$artifact" || -L "$target/$artifact" ]]; then
    printf '%s\n' "evidence output already exists; use a fresh evidence directory" >&2
    exit 1
  fi
done

tectonic_bin=${TECTONIC_BIN:-tectonic}
if [[ "$tectonic_bin" != */* ]]; then
  tectonic_bin=$(command -v -- "$tectonic_bin" || true)
fi
if [[ ! -x "$tectonic_bin" ]]; then
  printf '%s\n' "Tectonic executable is unavailable" >&2
  exit 1
fi
if ! command -v pdftoppm >/dev/null 2>&1; then
  printf '%s\n' "pdftoppm executable is unavailable" >&2
  exit 1
fi

work=$(mktemp -d "$target/.work.XXXXXX")
published=()
work_identity=
if ! work_identity=$(stat -c '%d:%i' -- "$work" 2>/dev/null); then
  work_identity=
fi
cleanup() {
  local exit_status=$? cleanup_failed=0 current_identity

  # Do not roll back published paths by name.  They may have been replaced
  # since ln created them; deleting by pathname could destroy another
  # process's file.  A failed publication is therefore reported, not undone.
  if ((${#published[@]})); then
    printf '%s\n' "publication failed; partial artifacts may remain at:" >&2
    printf '  %s\n' "${published[@]}" >&2
    exit_status=1
  fi

  # Verify the invocation-owned staging directory before recursive cleanup.
  # This is the strongest safe check available to this local shell wrapper;
  # do not follow a replacement path.
  current_identity=
  if ! current_identity=$(stat -c '%d:%i' -- "$work" 2>/dev/null); then
    current_identity=
  fi
  if [[ -z "$work_identity" || -z "$current_identity" || "$current_identity" != "$work_identity" || -L "$work" || ! -d "$work" ]]; then
    printf '%s\n' "staging cleanup refused; inspect partial artifacts under: $work" >&2
    cleanup_failed=1
  elif ! rm -rf -- "$work" || [[ -e "$work" || -L "$work" ]]; then
    printf '%s\n' "staging cleanup failed; inspect partial artifacts under: $work" >&2
    cleanup_failed=1
  fi
  if ((cleanup_failed)); then
    exit_status=1
  fi
  trap - EXIT
  exit "$exit_status"
}
trap cleanup EXIT

cargo run --quiet --manifest-path "$repo_root/Cargo.toml" -- "$input" --output "$work/reports"
tex=$(find "$work/reports" -maxdepth 1 -type f -name '*.tex' -print -quit)
if [[ -z "$tex" ]]; then
  printf '%s\n' "report source was not generated" >&2
  exit 1
fi
cp -- "$tex" "$work/report.tex"

mkdir -p -- "$work/pdf"
SOURCE_DATE_EPOCH=0 "$tectonic_bin" --outdir "$work/pdf" --keep-logs "$work/report.tex"
pdf="$work/pdf/report.pdf"
png="$work/report.png"
if [[ ! -f "$pdf" || -L "$pdf" || ! -s "$pdf" ]]; then
  printf '%s\n' "rendered PDF was not generated as a non-empty regular file" >&2
  exit 1
fi
cp -- "$pdf" "$work/report.pdf"
pdftoppm -png -singlefile "$pdf" "$work/report"
if [[ ! -f "$png" || -L "$png" || ! -s "$png" ]]; then
  printf '%s\n' "rendered PNG was not generated as a non-empty regular file" >&2
  exit 1
fi
if [[ ! -f "$work/report.tex" || -L "$work/report.tex" || ! -s "$work/report.tex" ]]; then
  printf '%s\n' "report source was not generated as a non-empty regular file" >&2
  exit 1
fi

# Hard-link publication is an atomic create-without-following-destination
# operation on the same filesystem.  If any create fails, do not remove
# successfully created links by pathname: a concurrent replacement could make
# that destructive.  The failure handler reports every partial final path.
for artifact in report.tex report.pdf report.png; do
  if ! ln -T -- "$work/$artifact" "$target/$artifact"; then
    printf '%s\n' "could not publish evidence without overwriting an existing path" >&2
    exit 1
  fi
  published+=("$target/$artifact")
done

# Keep the post-publication contract explicit as a final safety check.
for artifact in report.tex report.pdf report.png; do
  if [[ ! -f "$target/$artifact" || -L "$target/$artifact" || ! -s "$target/$artifact" ]]; then
    printf '%s\n' "published evidence artifact failed validation" >&2
    exit 1
  fi
done
published=()
