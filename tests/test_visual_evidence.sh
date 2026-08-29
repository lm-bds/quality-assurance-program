#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
script="$repo_root/scripts/generate_visual_evidence.sh"
fixture="$repo_root/docs/evidence/rendered-report/synthetic_input.csv"
tmp=$(mktemp -d "${TMPDIR:-/tmp}/qaprs-visual-tests.XXXXXX")
trap 'rm -rf -- "$tmp"' EXIT

fake_bin="$tmp/fake-bin"
mkdir -p "$fake_bin"
cat >"$fake_bin/tectonic" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
outdir=
for arg in "$@"; do
  if [[ "$arg" == --outdir ]]; then
    next=1
  elif [[ "${next:-0}" == 1 ]]; then
    outdir=$arg
    next=0
  fi
done
mkdir -p "$outdir"
printf 'fake pdf\n' >"$outdir/report.pdf"
EOF
cat >"$fake_bin/pdftoppm" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
printf '\x89PNG\r\n' >"${@: -1}.png"
EOF
chmod +x "$fake_bin/tectonic" "$fake_bin/pdftoppm"

new_target() {
  local name=$1
  local target="$tmp/$name"
  mkdir -p "$target"
  cp "$fixture" "$target/synthetic_input.csv"
  printf '%s\n' "$target"
}
run_script() {
  PATH="$fake_bin:$PATH" TECTONIC_BIN=tectonic "$script" "$1"
}

# A symlinked root and a symlinked direct input are rejected before any read/write.
outside="$tmp/outside"
mkdir -p "$outside"
cp "$fixture" "$outside/synthetic_input.csv"
ln -s "$outside" "$tmp/root-link"
if run_script "$tmp/root-link" >/dev/null 2>&1; then
  printf '%s\n' 'symlinked target was accepted' >&2
  exit 1
fi
symlink_input=$(new_target symlink-input)
rm "$symlink_input/synthetic_input.csv"
ln -s "$outside/synthetic_input.csv" "$symlink_input/synthetic_input.csv"
if run_script "$symlink_input" >/dev/null 2>&1; then
  printf '%s\n' 'symlinked input was accepted' >&2
  exit 1
fi
[[ ! -e "$symlink_input/report.tex" && ! -e "$symlink_input/report.pdf" && ! -e "$symlink_input/report.png" ]]
cmp "$outside/synthetic_input.csv" "$fixture"

# Every existing final artifact, regular or symlink, is refused without changing its sentinel.
for artifact in report.tex report.pdf report.png; do
  target=$(new_target "regular-$artifact")
  printf 'regular sentinel\n' >"$target/$artifact"
  if run_script "$target" >/dev/null 2>&1; then exit 1; fi
  [[ "$(cat "$target/$artifact")" == 'regular sentinel' ]]

done
for artifact in report.tex report.pdf report.png; do
  target=$(new_target "symlink-$artifact")
  printf 'symlink sentinel\n' >"$tmp/sentinel-$artifact"
  ln -s "$tmp/sentinel-$artifact" "$target/$artifact"
  if run_script "$target" >/dev/null 2>&1; then exit 1; fi
  [[ -L "$target/$artifact" ]]
  [[ "$(cat "$tmp/sentinel-$artifact")" == 'symlink sentinel' ]]
done

# A fresh target publishes exactly the three non-empty artifacts.
fresh=$(new_target fresh)
run_script "$fresh" >/dev/null
for artifact in report.tex report.pdf report.png; do
  [[ -f "$fresh/$artifact" && ! -L "$fresh/$artifact" && -s "$fresh/$artifact" ]]
done

# A staging/conversion failure leaves no final artifacts.
failed=$(new_target failed-staging)
cat >"$tmp/failing-pdftoppm" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: >"${@: -1}.png"
EOF
chmod +x "$tmp/failing-pdftoppm"
mkdir -p "$tmp/failing-bin"
ln -s "$fake_bin/tectonic" "$tmp/failing-bin/tectonic"
ln -s "$tmp/failing-pdftoppm" "$tmp/failing-bin/pdftoppm"
if PATH="$tmp/failing-bin:$PATH" TECTONIC_BIN=tectonic "$script" "$failed" >/dev/null 2>&1; then
  printf '%s\n' 'empty staged PNG was accepted' >&2
  exit 1
fi
[[ ! -e "$failed/report.tex" && ! -e "$failed/report.pdf" && ! -e "$failed/report.png" ]]

# Missing and mismatched staging identities are fail-closed.  In particular,
# two failed stat calls must not compare as equal and authorize rm -rf.
for identity_case in missing mismatch; do
  identity_target=$(new_target "identity-$identity_case")
  mkdir -p "$tmp/$identity_case-bin"
  if [[ "$identity_case" == missing ]]; then
    cat >"$tmp/$identity_case-bin/stat" <<'EOF'
#!/usr/bin/env bash
exit 1
EOF
  else
    cat >"$tmp/$identity_case-bin/stat" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count_file=${QAPRS_STAT_COUNT:?}
count=$(<"$count_file")
count=$((count + 1))
printf '%s\n' "$count" >"$count_file"
if [[ "$count" == 1 ]]; then
  printf '%s\n' '1:1'
else
  printf '%s\n' '1:2'
fi
EOF
    printf '0\n' >"$tmp/$identity_case-count"
  fi
  chmod +x "$tmp/$identity_case-bin/stat"
  identity_log="$tmp/identity-$identity_case.log"
  if PATH="$tmp/$identity_case-bin:$fake_bin:$PATH" \
    QAPRS_STAT_COUNT="$tmp/$identity_case-count" TECTONIC_BIN=tectonic \
    "$script" "$identity_target" >"$identity_log" 2>&1; then
    printf 'staging identity %s was accepted\n' "$identity_case" >&2
    exit 1
  fi
  grep -F 'staging cleanup refused; inspect partial artifacts under:' "$identity_log" >/dev/null
  [[ -n "$(compgen -G "$identity_target/.work.*")" ]]
done

# Cleanup failure is also fail-closed and never reported as a successful run.
cleanup_failed=$(new_target cleanup-failure)
mkdir -p "$tmp/cleanup-bin"
real_rm=$(command -v rm)
cat >"$tmp/cleanup-bin/rm" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
path=${@: -1}
if [[ "$path" == *.work.* ]]; then
  exit 1
fi
exec "$QAPRS_REAL_RM" "$@"
EOF
chmod +x "$tmp/cleanup-bin/rm"
cleanup_log="$tmp/cleanup-failure.log"
if PATH="$tmp/cleanup-bin:$fake_bin:$PATH" QAPRS_REAL_RM="$real_rm" \
  TECTONIC_BIN=tectonic "$script" "$cleanup_failed" >"$cleanup_log" 2>&1; then
  printf '%s\n' 'cleanup failure was accepted' >&2
  exit 1
fi
grep -F 'staging cleanup failed; inspect partial artifacts under:' "$cleanup_log" >/dev/null
for artifact in report.tex report.pdf report.png; do
  [[ -f "$cleanup_failed/$artifact" && ! -L "$cleanup_failed/$artifact" && -s "$cleanup_failed/$artifact" ]]
done

# Publication failure is fail-closed: a concurrent replacement of an artifact
# created by this invocation is never deleted, and the partial path is named.
published_failure=$(new_target publication-failure)
mkdir -p "$tmp/publication-bin"
real_ln=$(command -v ln)
cat >"$tmp/publication-bin/ln" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count_file=${QAPRS_LN_COUNT:?}
count=$(<"$count_file")
count=$((count + 1))
printf '%s\n' "$count" >"$count_file"
if [[ "$count" == 1 ]]; then
  exec "$QAPRS_REAL_LN" "$@"
fi
if [[ "$count" == 2 ]]; then
  first=${QAPRS_FIRST_ARTIFACT:?}
  rm -f -- "$first"
  printf '%s\n' 'concurrent replacement sentinel' >"$first"
  exit 1
fi
exec "$QAPRS_REAL_LN" "$@"
EOF
chmod +x "$tmp/publication-bin/ln"
printf '0\n' >"$tmp/ln-count"
first_artifact="$published_failure/report.tex"
publication_log="$tmp/publication-failure.log"
if PATH="$tmp/publication-bin:$fake_bin:$PATH" QAPRS_REAL_LN="$real_ln" \
  QAPRS_LN_COUNT="$tmp/ln-count" QAPRS_FIRST_ARTIFACT="$first_artifact" \
  TECTONIC_BIN=tectonic "$script" "$published_failure" >"$publication_log" 2>&1; then
  printf '%s\n' 'publication failure was accepted' >&2
  exit 1
fi
[[ "$(cat "$first_artifact")" == 'concurrent replacement sentinel' ]]
grep -F "$first_artifact" "$publication_log" >/dev/null
[[ ! -e "$published_failure/report.pdf" && ! -e "$published_failure/report.png" ]]

# A destination replaced with a symlink to a directory must not be followed by
# GNU ln.  The external directory remains empty and the symlink is preserved.
symlink_dir_failure=$(new_target publication-symlink-dir)
mkdir -p "$tmp/external-publication"
mkdir -p "$tmp/symlink-dir-bin"
cat >"$tmp/symlink-dir-bin/ln" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
count_file=${QAPRS_LN_COUNT:?}
count=$(<"$count_file")
count=$((count + 1))
printf '%s\n' "$count" >"$count_file"
destination=${@: -1}
if [[ "$count" == 2 ]]; then
  rm -f -- "$destination"
  "$QAPRS_REAL_LN" -s -- "$QAPRS_EXTERNAL_DIR" "$destination"
fi
exec "$QAPRS_REAL_LN" "$@"
EOF
chmod +x "$tmp/symlink-dir-bin/ln"
printf '0\n' >"$tmp/symlink-dir-count"
symlink_dir_log="$tmp/publication-symlink-dir.log"
if PATH="$tmp/symlink-dir-bin:$fake_bin:$PATH" QAPRS_REAL_LN="$real_ln" \
  QAPRS_LN_COUNT="$tmp/symlink-dir-count" QAPRS_EXTERNAL_DIR="$tmp/external-publication" \
  TECTONIC_BIN=tectonic "$script" "$symlink_dir_failure" >"$symlink_dir_log" 2>&1; then
  printf '%s\n' 'symlink-directory publication was accepted' >&2
  exit 1
fi
[[ -L "$symlink_dir_failure/report.pdf" ]]
[[ ! -e "$tmp/external-publication/report.pdf" ]]
grep -F "$symlink_dir_failure/report.tex" "$symlink_dir_log" >/dev/null

printf '%s\n' 'visual evidence wrapper regression tests passed'
