#!/usr/bin/env bash
set -euo pipefail

ROOT=""
VERBOSE=0

usage() {
  cat <<'USAGE'
Usage:
  validate_harness.sh [--root PATH] [--verbose]
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      ROOT="$2"
      shift 2
      ;;
    --verbose)
      VERBOSE=1
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "error: unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

infer_repo_root() {
  local probe
  if [[ -n "$ROOT" ]]; then
    (cd "$ROOT" && pwd -P)
    return 0
  fi

  probe="$(cd "$(dirname "$0")/../.." && pwd -P)"
  echo "$probe"
}

REPO_ROOT="$(infer_repo_root)"
cd "$REPO_ROOT"

FAILURES=0
WARNINGS=0

log_error() {
  printf 'ERROR: %s\n' "$1"
  FAILURES=$((FAILURES + 1))
}

log_warn() {
  printf 'WARN: %s\n' "$1"
  WARNINGS=$((WARNINGS + 1))
}

log_info() {
  if [[ $VERBOSE -eq 1 ]]; then
    printf 'INFO: %s\n' "$1"
  fi
}

require_file() {
  local path="$1"
  [[ -f "$path" ]] || log_error "missing file: $path"
}

require_dir() {
  local path="$1"
  [[ -d "$path" ]] || log_error "missing directory: $path"
}

find_workspace_markdown_files() {
  find . \
    -type d \( \
      -name .git -o \
      -name .old -o \
      -name node_modules -o \
      -name bower_components -o \
      -name vendor -o \
      -name dist -o \
      -name build -o \
      -name coverage -o \
      -name .next -o \
      -name .nuxt -o \
      -name .svelte-kit -o \
      -name .turbo -o \
      -name .cache -o \
      -name .pnpm-store -o \
      -name .yarn -o \
      -name venv -o \
      -name .venv \
    \) -prune -o \
    -type f -name '*.md' -print | sort
}

check_absent() {
  local path="$1"
  [[ ! -e "$path" ]] || log_error "unexpected legacy path remains: $path"
}

check_required_headings() {
  local file="$1"
  shift
  local previous=0
  local heading
  for heading in "$@"; do
    local line
    line="$(grep -n -F "$heading" "$file" | head -n1 | cut -d: -f1 || true)"
    if [[ -z "$line" ]]; then
      log_error "missing heading '$heading' in $file"
      continue
    fi
    if (( line <= previous )); then
      log_error "heading order is invalid in $file: $heading"
    fi
    previous=$line
  done
}

check_required_layout() {
  require_file "AGENTS.md"
  require_file "README.md"
  require_file ".agents/project.md"
  require_dir ".agents/tasks"
  require_dir ".agents/skills/_shared"
  require_dir ".agents/scripts"

  require_file "docs/project/index.md"
  require_file "docs/project/architecture.md"
  require_file "docs/project/tech-stack.md"
  require_file "docs/project/validation.md"
  require_file "docs/project/glossary/index.md"
  require_file "docs/project/glossary/core.md"
  require_file "docs/project/research/index.md"
  require_file "docs/project/proposals/index.md"
  require_file "docs/project/requirements/index.md"
  require_file "docs/project/specs/index.md"
  require_file "docs/project/specs/shared/index.md"
  require_file "docs/project/patterns/index.md"
  require_file "docs/project/adr/index.md"
  require_file "docs/project/teams/index.md"
  require_file "docs/project/integrations/index.md"

  require_dir "docs/project/proposals/active"
  require_dir "docs/project/proposals/archive"
  require_dir "docs/project/adr/active"
  require_dir "docs/project/adr/archive"
}

extract_index_refs() {
  local file="$1"
  sed -nE 's/^- `([^`]+)`:.*$/\1/p' "$file"
}

check_index_reference_lists() {
  local index dir
  while IFS= read -r index; do
    if ! rg -q '^## 参照$' "$index"; then
      log_error "missing '## 参照' section in $index"
      continue
    fi

    dir="$(dirname "$index")"
    local refs
    refs="$(extract_index_refs "$index")"

    while IFS= read -r ref; do
      [[ -n "$ref" ]] || continue
      if [[ "$ref" == */ ]]; then
        [[ -d "$dir/$ref" ]] || log_error "index references missing directory: $index -> $ref"
      else
        [[ -e "$dir/$ref" ]] || log_error "index references missing path: $index -> $ref"
      fi
    done <<< "$refs"

    case "$index" in
      docs/project/adr/index.md|docs/project/proposals/index.md)
        local bucket
        for bucket in active archive; do
          if [[ -d "$dir/$bucket" ]]; then
            if ! grep -Fqx "$bucket/" <<< "$refs"; then
              log_error "missing directory reference in $index -> $bucket/"
            fi
          fi
        done
        while IFS= read -r file; do
          [[ -n "$file" ]] || continue
          local rel
          rel="${file#"$dir/"}"
          if ! grep -Fqx "$rel" <<< "$refs"; then
            log_error "missing file reference in $index -> $rel"
          fi
        done < <(find "$dir"/active "$dir"/archive -maxdepth 1 -type f -name '*.md' -not -name 'index.md' 2>/dev/null | sort)
        ;;
      *)
        while IFS= read -r file; do
          [[ -n "$file" ]] || continue
          local rel
          rel="${file#"$dir/"}"
          if ! grep -Fqx "$rel" <<< "$refs"; then
            log_error "missing file reference in $index -> $rel"
          fi
        done < <(find "$dir" -maxdepth 1 -type f -name '*.md' -not -name 'index.md' | sort)

        while IFS= read -r subindex; do
          [[ -n "$subindex" ]] || continue
          local rel
          rel="${subindex#"$dir/"}"
          if ! grep -Fqx "$rel" <<< "$refs"; then
            log_error "missing child index reference in $index -> $rel"
          fi
        done < <(find "$dir" -mindepth 2 -maxdepth 2 -type f -name 'index.md' | sort)
        ;;
    esac
  done < <(find docs/project -type f -name 'index.md' | sort)
}

check_required_skills() {
  local skills=(
    task-prepare
    task-discovery
    task-implementation
    task-fix
    task-review
    task-validation
    task-close
    docs-update-requirement
    docs-update-spec
    docs-update-adr
    docs-update-patterns
    docs-update-architecture
    docs-update-glossary
    docs-update-integrations
    docs-update-team-guide
    docs-update-tech-stack
    docs-update-validation
    docs-update-research
    docs-update-proposal
    harness-init
    harness-update-project
    harness-create-skill
    harness-validation
  )
  local skill
  for skill in "${skills[@]}"; do
    require_file ".agents/skills/$skill/SKILL.md"
  done
}

check_legacy_artifacts() {
  check_absent ".agents/project.yaml"
  check_absent ".agents/index.md"
  check_absent ".agents/skills/index.md"
  check_absent ".agents/skills/harness-update-project-yaml"

  if find docs/project -name 'README.md' -print | grep -q .; then
    log_error "legacy README.md files remain under docs/project"
  fi

  if rg -n --hidden \
    --glob '!.git/**' \
    --glob '!.old/**' \
    --glob '!PLAN.md' \
    --glob '!.agents/scripts/validate_harness.sh' \
    'harness-update-project-yaml|project\.yaml|docs/project/glossary\.md' \
    AGENTS.md README.md .agents docs >/dev/null; then
    log_error "legacy references remain in current workspace"
  fi

  if rg -n --hidden \
    --pcre2 \
    --glob '!.git/**' \
    --glob '!.old/**' \
    --glob '!PLAN.md' \
    --glob '!.agents/scripts/validate_harness.sh' \
    '(?<!\.agents/)tasks/task-' \
    AGENTS.md README.md .agents docs >/dev/null; then
    log_error "found legacy task path without .agents/ prefix"
  fi
}

check_project_md() {
  check_required_headings ".agents/project.md" \
    "## Name" \
    "## Description" \
    "## Language" \
    "## Kind" \
    "## Subprojects" \
    "## Teams" \
    "## Integrations"
}

check_task_files() {
  local task
  while IFS= read -r task; do
    check_required_headings "$task" \
      "## 目的" \
      "## 状態" \
      "## 依頼内容" \
      "## 確定前提" \
      "## 未確定事項" \
      "## 対象範囲" \
      "## 対象外" \
      "## 守るべき不変条件" \
      "## 参照する正本" \
      "## 今回読まなくてよい資料" \
      "## 実施方針" \
      "## 実施手順" \
      "## 検証項目" \
      "## 完了条件" \
      "## 進捗記録" \
      "## 次に読むもの"
  done < <(find .agents/tasks -maxdepth 1 -type f -name 'task-*.md' | sort)
}

check_duplicate_ids() {
  local ids
  ids="$(rg -n '^id:' docs/project --glob '*.md' -g '!**/index.md' | sed -E 's/.*id:[[:space:]]*//' || true)"
  if [[ -n "$ids" ]]; then
    local dupes
    dupes="$(printf '%s\n' "$ids" | sed '/^$/d' | sort | uniq -d || true)"
    if [[ -n "$dupes" ]]; then
      while IFS= read -r dup; do
        [[ -n "$dup" ]] || continue
        log_error "duplicate document id: $dup"
      done <<< "$dupes"
    fi
  fi
}

check_status_values() {
  local invalid
  invalid="$(rg -n '^status:' docs/project --glob '*.md' -g '!**/index.md' | grep -Ev 'status:[[:space:]]*(下書き|提案中|採用|非推奨|保管)$' || true)"
  if [[ -n "$invalid" ]]; then
    log_error "found invalid document status value"
    printf '%s\n' "$invalid"
  fi
}

check_archive_placement() {
  local file status
  while IFS= read -r file; do
    status="$(grep -E '^status:' "$file" | head -n1 | sed -E 's/^status:[[:space:]]*//' || true)"
    if [[ "$file" == *'/archive/'* ]]; then
      if [[ -n "$status" && "$status" != "保管" && "$status" != "非推奨" ]]; then
        log_error "archive file has active-like status: $file ($status)"
      fi
    fi
    if [[ "$file" == *'/active/'* ]]; then
      if [[ "$status" == "保管" ]]; then
        log_error "active file is marked as 保管: $file"
      fi
    fi
  done < <(find docs/project -type f -name '*.md' -not -name 'index.md' | sort)
}

check_markdown_links() {
  local file
  while IFS= read -r file; do
    while IFS= read -r line; do
      local target
      target="$(printf '%s' "$line" | sed -E 's/.*\(([^)#]+)(#[^)]+)?\).*/\1/')"
      [[ -n "$target" ]] || continue
      case "$target" in
        http:*|https:*|mailto:*|\#*)
          continue
          ;;
      esac
      if [[ "$target" == *'<'* || "$target" == *'{'* ]]; then
        continue
      fi
      local resolved
      resolved="$(cd "$(dirname "$file")" && realpath -m "$target")"
      if [[ "$resolved" != "$REPO_ROOT"/* ]]; then
        continue
      fi
      if [[ ! -e "$resolved" ]]; then
        log_error "broken markdown link in $file -> $target"
      fi
    done < <(grep -oE '\[[^]]+\]\(([^)]+)\)' "$file" || true)
  done < <(find_workspace_markdown_files)
}

check_required_layout
check_required_skills
check_legacy_artifacts
check_project_md
check_task_files
check_duplicate_ids
check_status_values
check_archive_placement
check_index_reference_lists
check_markdown_links

if (( FAILURES > 0 )); then
  printf 'validate_harness: FAILED (%d error(s), %d warning(s))\n' "$FAILURES" "$WARNINGS" >&2
  exit 1
fi

printf 'validate_harness: OK (%d warning(s))\n' "$WARNINGS"