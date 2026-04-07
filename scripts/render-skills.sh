#!/usr/bin/env bash
# Render skill templates to published skills directory.
# This script reads the CLI program name from config/cli.toml and renders
# all templates in skills-src/ to skills/, replacing {{CLI_PROGRAM}} with
# the configured binary name.
#
# Usage:
#   render-skills.sh [--check]
#
# With --check, validates that skills/ matches the rendered output without
# modifying files. Useful for CI/CD.

set -euo pipefail

# Find the repository root from this script's location.
script_dir="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
repo_root="$(dirname "$script_dir")"
cd "$repo_root"

# Parse arguments
check_mode=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check) check_mode=1; shift ;;
    -h|--help)
      printf 'Usage: %s [--check]\n' "$0"
      printf '\nRender skill templates to published skills directory.\n'
      printf '\nOptions:\n'
      printf '  --check    Validate without modifying files\n'
      printf '  -h, --help Show this help message\n'
      exit 0
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 1
      ;;
  esac
done

# Extract the CLI program name from config/cli.toml
if [[ ! -f "config/cli.toml" ]]; then
  printf 'ERROR: config/cli.toml not found\n' >&2
  exit 1
fi

cli_program=$(grep '^program_name = ' config/cli.toml | sed 's/.*= "\(.*\)"/\1/')
if [[ -z "$cli_program" ]]; then
  printf 'ERROR: Could not extract program_name from config/cli.toml\n' >&2
  exit 1
fi

printf 'Rendering skills with CLI program name: %s\n' "$cli_program"

# Create temporary directory for rendered output
tmpdir=$(mktemp -d)
trap "rm -rf '$tmpdir'" EXIT

# Render all templates
rendered_count=0
while IFS= read -r template_file; do
  skill_name=$(basename "$(dirname "$template_file")")
  output_file="skills/$skill_name/SKILL.md"
  temp_output="$tmpdir/$skill_name.md"
  
  # Render template by replacing {{CLI_PROGRAM}} with the configured name
  sed "s/{{CLI_PROGRAM}}/$cli_program/g" "$template_file" > "$temp_output"
  
  if [[ $check_mode -eq 1 ]]; then
    # In check mode, verify the rendered output matches the published file
    if [[ ! -f "$output_file" ]]; then
      printf 'ERROR: Missing published skill: %s\n' "$output_file" >&2
      exit 1
    fi
    
    if ! diff -q "$temp_output" "$output_file" > /dev/null 2>&1; then
      printf 'ERROR: Published skill does not match template: %s\n' "$output_file" >&2
      printf '       Run: render-skills.sh (without --check) to update\n' >&2
      exit 1
    fi
  else
    # In render mode, write the output
    mkdir -p "skills/$skill_name"
    cp "$temp_output" "$output_file"
  fi
  
  rendered_count=$((rendered_count + 1))
done < <(find skills-src -name "SKILL.md" -type f | sort)

while IFS= read -r published_file; do
  skill_name=$(basename "$(dirname "$published_file")")
  template_file="skills-src/$skill_name/SKILL.md"

  if [[ ! -f "$template_file" ]]; then
    printf 'ERROR: Published skill has no template: %s\n' "$published_file" >&2
    exit 1
  fi
done < <(find skills -name "SKILL.md" -type f | sort)

if [[ $check_mode -eq 1 ]]; then
  printf 'All %d published skills match templates\n' "$rendered_count"
else
  printf 'Rendered %d skill(s) to skills/\n' "$rendered_count"
fi

exit 0
