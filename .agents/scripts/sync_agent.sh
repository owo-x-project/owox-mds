#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR_NAME="scripts"
AGENTS_DIR_NAME=".agents"
BACKUP_DIR_NAME=".backup"
BACKUP_LATEST_FILE="LATEST"
AIDER_CONFIG=".aider.conf.yml"

TARGET="${1:-}"
shift || true

DRY_RUN=0
FORCE=0
ROOT=""
SNAPSHOT_NAME=""

usage() {
  cat <<'USAGE'
Usage:
  sync_agent.sh <target> [--root PATH] [--dry-run] [--force] [--snapshot-name NAME]

Targets:
  claude | copilot | cursor | opencode | crush | gemini | aider | kiro
USAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root)
      ROOT="$2"
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --force)
      FORCE=1
      shift
      ;;
    --snapshot-name)
      SNAPSHOT_NAME="$2"
      shift 2
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

if [[ -z "$TARGET" ]]; then
  usage >&2
  exit 2
fi

if ! command -v perl >/dev/null 2>&1; then
  echo "error: perl is required." >&2
  exit 2
fi

if ! command -v find >/dev/null 2>&1; then
  echo "error: find is required." >&2
  exit 2
fi

TARGET_DISPLAY=""
TARGET_SKILL_DIR=""
TARGET_CONTEXT_FILE=""
TARGET_OVERRIDE_FILE=""
TARGET_ASK_TOOL=""
MERGE_OVERRIDE_INTO_CONTEXT=0
SAME_NAME_AGENTS=0
COPY_SKILLS=1
CREATE_ROOT_COPILOT_FILE=0
CREATE_AIDER_CONFIG=0

NOTES=()

case "$TARGET" in
  claude)
    TARGET_DISPLAY="Claude Code"
    TARGET_SKILL_DIR=".claude/skills"
    TARGET_CONTEXT_FILE="CLAUDE.md"
    TARGET_OVERRIDE_FILE="CLAUDE.local.md"
    TARGET_ASK_TOOL="AskUserQuestion"
    ;;
  copilot)
    TARGET_DISPLAY="GitHub Copilot CLI"
    TARGET_SKILL_DIR=".github/skills"
    TARGET_CONTEXT_FILE="AGENTS.md"
    TARGET_OVERRIDE_FILE="AGENTS.override.md"
    TARGET_ASK_TOOL="ask_user"
    SAME_NAME_AGENTS=1
    CREATE_ROOT_COPILOT_FILE=1
    NOTES+=("AGENTS.md is rewritten in place for Copilot-compatible tool names.")
    NOTES+=("A merged root .github/copilot-instructions.md is also generated.")
    ;;
  cursor)
    TARGET_DISPLAY="Cursor CLI"
    TARGET_SKILL_DIR=".cursor/skills"
    TARGET_CONTEXT_FILE="AGENTS.md"
    TARGET_OVERRIDE_FILE="AGENTS.override.md"
    TARGET_ASK_TOOL="ask question tool"
    SAME_NAME_AGENTS=1
    NOTES+=("AGENTS.md is rewritten in place for Cursor-compatible tool names.")
    ;;
  opencode)
    TARGET_DISPLAY="OpenCode"
    TARGET_SKILL_DIR=".opencode/skills"
    TARGET_CONTEXT_FILE="AGENTS.md"
    TARGET_OVERRIDE_FILE="AGENTS.override.md"
    TARGET_ASK_TOOL="question"
    SAME_NAME_AGENTS=1
    NOTES+=("AGENTS.md is rewritten in place for OpenCode-compatible tool names.")
    ;;
  crush)
    TARGET_DISPLAY="Crush"
    TARGET_SKILL_DIR=".crush/skills"
    TARGET_CONTEXT_FILE="AGENTS.md"
    TARGET_OVERRIDE_FILE="AGENTS.override.md"
    TARGET_ASK_TOOL="Questions"
    SAME_NAME_AGENTS=1
    NOTES+=("Crush uses AGENTS.md by default; files are rewritten in place.")
    NOTES+=("request_user_input is mapped to Questions because a documented ask-user tool name was not found.")
    ;;
  gemini)
    TARGET_DISPLAY="Gemini CLI"
    TARGET_SKILL_DIR=".gemini/skills"
    TARGET_CONTEXT_FILE="GEMINI.md"
    TARGET_OVERRIDE_FILE=""
    TARGET_ASK_TOOL="ask_user"
    MERGE_OVERRIDE_INTO_CONTEXT=1
    ;;
  aider)
    TARGET_DISPLAY="Aider"
    TARGET_SKILL_DIR=""
    TARGET_CONTEXT_FILE="CONVENTIONS.md"
    TARGET_OVERRIDE_FILE=""
    TARGET_ASK_TOOL="Questions"
    MERGE_OVERRIDE_INTO_CONTEXT=1
    COPY_SKILLS=0
    CREATE_AIDER_CONFIG=1
    NOTES+=("Aider does not natively use AGENTS.md or Agent Skills as a project standard; CONVENTIONS.md files are generated instead.")
    NOTES+=("request_user_input is mapped to Questions because a documented ask-user tool name was not found.")
    ;;
  kiro)
    TARGET_DISPLAY="Kiro CLI"
    TARGET_SKILL_DIR=".kiro/skills"
    TARGET_CONTEXT_FILE="AGENTS.md"
    TARGET_OVERRIDE_FILE="AGENTS.override.md"
    TARGET_ASK_TOOL="Questions"
    SAME_NAME_AGENTS=1
    NOTES+=("Kiro supports AGENTS.md, so files are rewritten in place.")
    NOTES+=("request_user_input is mapped to Questions because a documented ask-user tool name was not found.")
    ;;
  *)
    echo "error: unsupported target: $TARGET" >&2
    exit 2
    ;;
esac

infer_repo_root() {
  local probe
  if [[ -n "$ROOT" ]]; then
    probe="$(cd "$ROOT" && pwd -P)"
    echo "$probe"
    return 0
  fi

  probe="$(cd "$(dirname "$0")" && pwd -P)"
  while [[ "$probe" != "/" ]]; do
    if [[ -d "$probe/$AGENTS_DIR_NAME/$SCRIPT_DIR_NAME" ]]; then
      echo "$probe"
      return 0
    fi
    probe="$(dirname "$probe")"
  done

  probe="$(pwd -P)"
  while [[ "$probe" != "/" ]]; do
    if [[ -d "$probe/$AGENTS_DIR_NAME/$SCRIPT_DIR_NAME" ]]; then
      echo "$probe"
      return 0
    fi
    probe="$(dirname "$probe")"
  done

  echo "error: could not infer repository root. Pass --root explicitly." >&2
  exit 2
}

REPO_ROOT="$(infer_repo_root)"
BACKUP_BASE="$REPO_ROOT/$AGENTS_DIR_NAME/$BACKUP_DIR_NAME"

if [[ -z "$SNAPSHOT_NAME" ]]; then
  SNAPSHOT_NAME="$(date +%Y%m%d-%H%M%S)-$TARGET"
fi

BACKUP_ROOT="$BACKUP_BASE/$SNAPSHOT_NAME"
ORIGINAL_BACKUP_ROOT="$BACKUP_ROOT/original"
TARGET_BACKUP_ROOT="$BACKUP_ROOT/overwritten-targets"

SKILL_HOME=""
if [[ -n "$TARGET_SKILL_DIR" ]]; then
  SKILL_HOME="~/${TARGET_SKILL_DIR#./}"
fi

TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

AGENT_FILES_LIST="$TMP_ROOT/agent_files.txt"
: > "$AGENT_FILES_LIST"

declare -A HAS_AGENT

declare -A HAS_OVERRIDE

declare -A BACKED_UP_ORIGINALS

declare -A BACKED_UP_TARGETS

declare -A CREATED_FILE_SET

declare -A CREATED_DIR_SET

declare -A OVERWRITTEN_SET

declare -A SOURCE_FILE_SET

declare -A SOURCE_DIR_SET

CREATED_FILES=()
CREATED_DIRS=()
OVERWRITTEN_TARGETS=()
SOURCE_FILES=()
SOURCE_DIRS=()

add_unique_array() {
  local arr_name="$1"
  local set_name="$2"
  local value="$3"
  [[ -z "$value" ]] && return 0
  local -n arr_ref="$arr_name"
  local -n set_ref="$set_name"
  if [[ -z "${set_ref[$value]:-}" ]]; then
    arr_ref+=("$value")
    set_ref["$value"]=1
  fi
}

record_created_dir_tree() {
  local rel="$1"
  [[ -z "$rel" || "$rel" == "." ]] && return 0
  local accum=""
  local part
  IFS='/' read -r -a parts <<< "$rel"
  for part in "${parts[@]}"; do
    [[ -z "$part" || "$part" == "." ]] && continue
    if [[ -z "$accum" ]]; then
      accum="$part"
    else
      accum="$accum/$part"
    fi
    if [[ ! -d "$REPO_ROOT/$accum" ]]; then
      add_unique_array CREATED_DIRS CREATED_DIR_SET "$accum"
    fi
    if [[ $DRY_RUN -eq 0 ]]; then
      mkdir -p "$REPO_ROOT/$accum"
    fi
  done
}

backup_original_path() {
  local rel="$1"
  [[ -n "${BACKED_UP_ORIGINALS[$rel]:-}" ]] && return 0
  BACKED_UP_ORIGINALS["$rel"]=1
  local src="$REPO_ROOT/$rel"
  local dst="$ORIGINAL_BACKUP_ROOT/$rel"
  if [[ $DRY_RUN -eq 0 && -e "$src" ]]; then
    mkdir -p "$(dirname "$dst")"
    if [[ -d "$src" ]]; then
      rm -rf "$dst"
      cp -a "$src" "$dst"
    else
      cp -a "$src" "$dst"
    fi
  fi
}

backup_existing_target() {
  local rel="$1"
  [[ -n "${BACKED_UP_TARGETS[$rel]:-}" ]] && return 0
  local src="$REPO_ROOT/$rel"
  [[ -e "$src" ]] || return 0
  BACKED_UP_TARGETS["$rel"]=1
  local dst="$TARGET_BACKUP_ROOT/$rel"
  if [[ $DRY_RUN -eq 0 ]]; then
    mkdir -p "$(dirname "$dst")"
    cp -a "$src" "$dst"
  fi
}

write_manifest() {
  [[ $DRY_RUN -eq 1 ]] && return 0
  mkdir -p "$BACKUP_ROOT"
  {
    printf 'snapshot_name=%q\n' "$SNAPSHOT_NAME"
    printf 'target=%q\n' "$TARGET"
  } > "$BACKUP_ROOT/manifest.env"

  printf '%s\n' "${CREATED_FILES[@]}" > "$BACKUP_ROOT/created_files.txt"
  printf '%s\n' "${CREATED_DIRS[@]}" > "$BACKUP_ROOT/created_dirs.txt"
  printf '%s\n' "${OVERWRITTEN_TARGETS[@]}" > "$BACKUP_ROOT/overwritten_targets.txt"
  printf '%s\n' "${SOURCE_FILES[@]}" > "$BACKUP_ROOT/source_files.txt"
  printf '%s\n' "${SOURCE_DIRS[@]}" > "$BACKUP_ROOT/source_dirs.txt"
  printf '%s\n' "${NOTES[@]}" > "$BACKUP_ROOT/notes.txt"

  mkdir -p "$BACKUP_BASE"
  printf '%s\n' "$SNAPSHOT_NAME" > "$BACKUP_BASE/$BACKUP_LATEST_FILE"
}

register_source_file() {
  add_unique_array SOURCE_FILES SOURCE_FILE_SET "$1"
}

register_source_dir() {
  add_unique_array SOURCE_DIRS SOURCE_DIR_SET "$1"
}

write_from_temp() {
  local rel="$1"
  local tmp="$2"
  local abs="$REPO_ROOT/$rel"

  if [[ -f "$abs" ]]; then
    if cmp -s "$abs" "$tmp"; then
      return 0
    fi
    backup_existing_target "$rel"
    add_unique_array OVERWRITTEN_TARGETS OVERWRITTEN_SET "$rel"
  fi

  record_created_dir_tree "$(dirname "$rel")"
  add_unique_array CREATED_FILES CREATED_FILE_SET "$rel"

  if [[ $DRY_RUN -eq 0 ]]; then
    mkdir -p "$(dirname "$abs")"
    cp "$tmp" "$abs"
  fi
}

write_text_content() {
  local rel="$1"
  local content_file="$2"
  write_from_temp "$rel" "$content_file"
}

copy_raw_file() {
  local src="$1"
  local rel_dst="$2"
  local abs_dst="$REPO_ROOT/$rel_dst"

  if [[ -f "$abs_dst" ]]; then
    if cmp -s "$src" "$abs_dst"; then
      return 0
    fi
    backup_existing_target "$rel_dst"
    add_unique_array OVERWRITTEN_TARGETS OVERWRITTEN_SET "$rel_dst"
  fi

  record_created_dir_tree "$(dirname "$rel_dst")"
  add_unique_array CREATED_FILES CREATED_FILE_SET "$rel_dst"

  if [[ $DRY_RUN -eq 0 ]]; then
    mkdir -p "$(dirname "$abs_dst")"
    cp -a "$src" "$abs_dst"
  fi
}

transform_file() {
  local src="$1"
  local dst_tmp="$2"
  DISPLAY_NAME="$TARGET_DISPLAY" \
  SKILL_DIR="$TARGET_SKILL_DIR" \
  SKILL_HOME="$SKILL_HOME" \
  CONTEXT_FILE="$TARGET_CONTEXT_FILE" \
  OVERRIDE_FILE="$TARGET_OVERRIDE_FILE" \
  ASK_TOOL="$TARGET_ASK_TOOL" \
  MERGE_OVERRIDE="$MERGE_OVERRIDE_INTO_CONTEXT" \
  perl -0pe '
    my $display = $ENV{DISPLAY_NAME} // q{};
    my $skill_dir = $ENV{SKILL_DIR} // q{};
    my $skill_home = $ENV{SKILL_HOME} // q{};
    my $context = $ENV{CONTEXT_FILE} // q{};
    my $override = $ENV{OVERRIDE_FILE} // q{};
    my $ask = $ENV{ASK_TOOL} // q{};
    my $merge_override = $ENV{MERGE_OVERRIDE} // q{0};

    if (length $ask) {
      s/\brequest_user_input\b/$ask/g;
    }

    if (length $skill_dir) {
      s{(?<![A-Za-z0-9_./-])\.agents/skills/}{$skill_dir/}g;
      s{(?<![A-Za-z0-9_./-])\.agents/skills\b}{$skill_dir}g;
      s{(?<![A-Za-z0-9_./-])~/.agents/skills/}{$skill_home/}g;
      s{(?<![A-Za-z0-9_./-])~/.agents/skills\b}{$skill_home}g;
      s{(?<![A-Za-z0-9_./-])\.codex/skills/}{$skill_dir/}g;
      s{(?<![A-Za-z0-9_./-])\.codex/skills\b}{$skill_dir}g;
    }

    if (length $context) {
      s/\bAGENTS\.md\b/$context/g;
    }

    if (length $override) {
      s/\bAGENTS\.override\.md\b/$override/g;
    } elsif (length $context && $merge_override eq q{1}) {
      s/\bAGENTS\.override\.md\b/$context/g;
    }

    if (length $display) {
      s/\bCodex CLI\b/$display/g;
      s/\bcodex cli\b/$display/g;
      s/\bCodex\b/$display/g;
    }
  ' "$src" > "$dst_tmp"
}

combine_with_override() {
  local base_file="$1"
  local override_file="$2"
  local out_file="$3"

  cat "$base_file" > "$out_file"
  if [[ -n "$override_file" && -f "$override_file" ]]; then
    printf '\n\n<!-- Added by agent_porter from source override instructions -->\n\n## Local / Override Instructions\n\n' >> "$out_file"
    cat "$override_file" >> "$out_file"
  fi
}

is_text_skill_file() {
  local filename="$1"
  case "$filename" in
    SKILL.md|*.md|*.mdx|*.txt|*.rst)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

rel_from_repo() {
  local abs="$1"
  local rel="${abs#"$REPO_ROOT"/}"
  if [[ "$abs" == "$REPO_ROOT" ]]; then
    rel="."
  fi
  printf '%s' "$rel"
}

find_agent_files() {
  find "$REPO_ROOT" \
    \( -path "$REPO_ROOT/.git" -o -path "$REPO_ROOT/.hg" -o -path "$REPO_ROOT/.svn" \
       -o -path "$REPO_ROOT/node_modules" -o -path "$REPO_ROOT/.next" -o -path "$REPO_ROOT/.turbo" \
       -o -path "$REPO_ROOT/dist" -o -path "$REPO_ROOT/build" -o -path "$REPO_ROOT/coverage" \
       -o -path "$REPO_ROOT/venv" -o -path "$REPO_ROOT/.venv" -o -path "$REPO_ROOT/__pycache__" \
       -o -path "$REPO_ROOT/$AGENTS_DIR_NAME/$BACKUP_DIR_NAME" \) -prune -o \
    -type f \( -name 'AGENTS.md' -o -name 'AGENTS.override.md' \) -print0
}

while IFS= read -r -d '' file; do
  rel="$(rel_from_repo "$file")"
  printf '%s\n' "$rel" >> "$AGENT_FILES_LIST"
  dir_rel="$(dirname "$rel")"
  base_name="$(basename "$rel")"
  if [[ "$base_name" == "AGENTS.md" ]]; then
    HAS_AGENT["$dir_rel"]="$rel"
  else
    HAS_OVERRIDE["$dir_rel"]="$rel"
  fi
  backup_original_path "$rel"
  register_source_file "$rel"
done < <(find_agent_files)

SKILLS_ROOT="$REPO_ROOT/$AGENTS_DIR_NAME/skills"
if [[ -d "$SKILLS_ROOT" ]]; then
  backup_original_path "$AGENTS_DIR_NAME/skills"
  register_source_dir "$AGENTS_DIR_NAME/skills"
fi

if [[ ! -s "$AGENT_FILES_LIST" && ! -d "$SKILLS_ROOT" ]]; then
  echo "error: no .agents/skills or AGENTS.md files were found." >&2
  exit 2
fi

transform_and_write_single() {
  local src_rel="$1"
  local dst_rel="$2"
  local tmp="$TMP_ROOT/$(basename "$dst_rel").$$.tmp"
  transform_file "$REPO_ROOT/$src_rel" "$tmp"
  write_text_content "$dst_rel" "$tmp"
}

process_agent_files() {
  local dir_rel base_rel override_rel tmp_combined tmp_transformed

  if [[ $SAME_NAME_AGENTS -eq 1 ]]; then
    while IFS= read -r base_rel; do
      [[ -n "$base_rel" ]] || continue
      transform_and_write_single "$base_rel" "$base_rel"
    done < <(printf '%s\n' "${HAS_AGENT[@]:-}")

    while IFS= read -r override_rel; do
      [[ -n "$override_rel" ]] || continue
      transform_and_write_single "$override_rel" "$override_rel"
    done < <(printf '%s\n' "${HAS_OVERRIDE[@]:-}")

    if [[ $CREATE_ROOT_COPILOT_FILE -eq 1 && -n "${HAS_AGENT[.]:-}" ]]; then
      base_rel="${HAS_AGENT[.]:-}"
      override_rel="${HAS_OVERRIDE[.]:-}"
      tmp_combined="$TMP_ROOT/copilot_combined.md"
      tmp_transformed="$TMP_ROOT/copilot_instructions.md"
      combine_with_override "$REPO_ROOT/$base_rel" "${override_rel:+$REPO_ROOT/$override_rel}" "$tmp_combined"
      transform_file "$tmp_combined" "$tmp_transformed"
      write_text_content ".github/copilot-instructions.md" "$tmp_transformed"
    fi
    return 0
  fi

  for dir_rel in "${!HAS_AGENT[@]}"; do
    base_rel="${HAS_AGENT[$dir_rel]}"
    override_rel="${HAS_OVERRIDE[$dir_rel]:-}"
    local dst_rel
    if [[ "$dir_rel" == "." ]]; then
      dst_rel="$TARGET_CONTEXT_FILE"
    else
      dst_rel="$dir_rel/$TARGET_CONTEXT_FILE"
    fi

    if [[ $MERGE_OVERRIDE_INTO_CONTEXT -eq 1 ]]; then
      tmp_combined="$TMP_ROOT/combined-${dir_rel//\//_}.md"
      tmp_transformed="$TMP_ROOT/transformed-${dir_rel//\//_}.md"
      combine_with_override "$REPO_ROOT/$base_rel" "${override_rel:+$REPO_ROOT/$override_rel}" "$tmp_combined"
      transform_file "$tmp_combined" "$tmp_transformed"
      write_text_content "$dst_rel" "$tmp_transformed"
    else
      transform_and_write_single "$base_rel" "$dst_rel"
      if [[ -n "$TARGET_OVERRIDE_FILE" && -n "$override_rel" ]]; then
        if [[ "$dir_rel" == "." ]]; then
          dst_rel="$TARGET_OVERRIDE_FILE"
        else
          dst_rel="$dir_rel/$TARGET_OVERRIDE_FILE"
        fi
        transform_and_write_single "$override_rel" "$dst_rel"
      fi
    fi
  done
}

copy_skills_tree() {
  [[ $COPY_SKILLS -eq 1 ]] || return 0
  [[ -d "$SKILLS_ROOT" ]] || return 0
  [[ -n "$TARGET_SKILL_DIR" ]] || return 0

  local src abs rel dst_rel tmp
  while IFS= read -r -d '' abs; do
    src="$abs"
    rel="${src#"$SKILLS_ROOT"/}"
    dst_rel="$TARGET_SKILL_DIR/$rel"
    if is_text_skill_file "$(basename "$src")"; then
      tmp="$TMP_ROOT/skill-$(basename "$src").$$.tmp"
      transform_file "$src" "$tmp"
      write_text_content "$dst_rel" "$tmp"
    else
      copy_raw_file "$src" "$dst_rel"
    fi
  done < <(find "$SKILLS_ROOT" \
      \( -path "$SKILLS_ROOT/.git" -o -path "$SKILLS_ROOT/.hg" -o -path "$SKILLS_ROOT/.svn" \
         -o -path "$SKILLS_ROOT/node_modules" -o -path "$SKILLS_ROOT/.next" -o -path "$SKILLS_ROOT/.turbo" \
         -o -path "$SKILLS_ROOT/dist" -o -path "$SKILLS_ROOT/build" -o -path "$SKILLS_ROOT/coverage" \
         -o -path "$SKILLS_ROOT/venv" -o -path "$SKILLS_ROOT/.venv" -o -path "$SKILLS_ROOT/__pycache__" \) -prune -o \
      -type f -print0)
}

ensure_aider_config() {
  [[ $CREATE_AIDER_CONFIG -eq 1 ]] || return 0
  local rel="$AIDER_CONFIG"
  local abs="$REPO_ROOT/$rel"
  local tmp="$TMP_ROOT/aider-conf.tmp"
  local snippet="# Added by agent_porter for aider conventions loading\nread: CONVENTIONS.md\n"

  if [[ -f "$abs" ]]; then
    if grep -Eq '^[[:space:]]*read[[:space:]]*:[[:space:]]*CONVENTIONS\.md[[:space:]]*$' "$abs"; then
      return 0
    fi
    if grep -Eq '^[[:space:]]*read[[:space:]]*:[[:space:]]*\[.*CONVENTIONS\.md.*\][[:space:]]*$' "$abs"; then
      return 0
    fi
    cp "$abs" "$tmp"
    if grep -Eq '^[[:space:]]*read[[:space:]]*:[[:space:]]*$' "$abs"; then
      printf '\n# Added by agent_porter\n# Ensure CONVENTIONS.md is loaded, for example:\n# read: [CONVENTIONS.md]\n' >> "$tmp"
    else
      printf '\n%s' "$snippet" >> "$tmp"
    fi
  else
    printf '%b' "$snippet" > "$tmp"
  fi

  write_text_content "$rel" "$tmp"
}

print_summary() {
  local mode="DONE"
  [[ $DRY_RUN -eq 1 ]] && mode="DRY-RUN"
  printf '[%s] target=%s snapshot=%s\n' "$mode" "$TARGET" "$SNAPSHOT_NAME"

  if [[ ${#CREATED_FILES[@]} -gt 0 ]]; then
    echo 'created_files:'
    printf '  - %s\n' "${CREATED_FILES[@]}"
  fi
  if [[ ${#CREATED_DIRS[@]} -gt 0 ]]; then
    echo 'created_dirs:'
    printf '  - %s\n' "${CREATED_DIRS[@]}"
  fi
  if [[ ${#OVERWRITTEN_TARGETS[@]} -gt 0 ]]; then
    echo 'overwritten_targets_backed_up:'
    printf '  - %s\n' "${OVERWRITTEN_TARGETS[@]}"
  fi
  if [[ ${#NOTES[@]} -gt 0 ]]; then
    echo 'notes:'
    printf '  - %s\n' "${NOTES[@]}"
  fi
}

process_agent_files
copy_skills_tree
ensure_aider_config
write_manifest
print_summary
