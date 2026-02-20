#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

FAILED=0

note_fail() {
  echo "FAIL: $1"
  FAILED=1
}

# 1) Required sections for llm docs.
required_sections=(
  "## Status Snapshot"
  "## Purpose"
  "## Scope"
  "## Source Anchors"
  "## Verification"
  "## Related Docs"
)

while IFS= read -r file; do
  for section in "${required_sections[@]}"; do
    if ! rg -q "^${section}$" "$file"; then
      note_fail "$file missing required section: $section"
    fi
  done
done < <(find llm/context llm/implementation llm/workflow -type f -name '*.md' | sort)

# 2) Markdown link/path integrity.
check_link_paths_for_file() {
  local file="$1"
  local dir
  dir="$(dirname "$file")"

  while IFS= read -r link; do
    case "$link" in
      http*|mailto:*|'#'*) continue ;;
    esac
    local resolved="$dir/$link"
    if [[ ! -e "$resolved" ]]; then
      note_fail "$file has broken link target: $link"
    fi
  done < <(rg -o "\]\([^)]*\)" "$file" | sed -E 's/^\]\((.*)\)$/\1/')
}

while IFS= read -r file; do
  check_link_paths_for_file "$file"
done < <(find llm docs -type f -name '*.md' | sort)
check_link_paths_for_file "README.md"

# 3) bun run command references must exist in package scripts.
declared_scripts=()
while IFS= read -r script_name; do
  declared_scripts+=("$script_name")
done < <(bun -e 'const p=require("./package.json"); Object.keys(p.scripts||{}).sort().forEach(k=>console.log(k));')

has_declared_script() {
  local target="$1"
  local item
  for item in "${declared_scripts[@]}"; do
    if [[ "$item" == "$target" ]]; then
      return 0
    fi
  done
  return 1
}

while IFS= read -r cmd; do
  if ! has_declared_script "$cmd"; then
    note_fail "Referenced command 'bun run $cmd' not found in package.json scripts"
  fi
done < <(rg -o "bun run [a-zA-Z0-9:_-]+" README.md llm/*.md llm/context/*.md llm/implementation/*.md llm/workflow/*.md docs/*.md | awk '{print $3}' | sort -u)

# 4) Absolute local source anchors must exist.
while IFS= read -r anchor; do
  if [[ "$anchor" == /Users/plebdev/Desktop/code/dicktaint* ]] && [[ ! -e "$anchor" ]]; then
    note_fail "Missing absolute source anchor path: $anchor"
  fi
done < <(rg -o '/Users/plebdev/Desktop/code/dicktaint[^` )"]+' README.md llm/*.md llm/context/*.md llm/implementation/*.md llm/workflow/*.md docs/*.md | sort -u)

if [[ "$FAILED" -ne 0 ]]; then
  echo "Documentation verification failed."
  exit 1
fi

echo "Documentation verification passed."
