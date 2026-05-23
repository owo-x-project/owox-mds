#!/usr/bin/env bash

release_version_from_ref() {
  local ref="${1:-}"
  if [[ "$ref" == v* ]]; then
    printf '%s\n' "${ref#v}"
  else
    printf '\n'
  fi
}

package_semver_from_release_version() {
  local raw="${1:-}"
  raw="${raw#v}"

  if [[ "$raw" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z][0-9A-Za-z.-]*)?$ ]]; then
    printf '%s\n' "$raw"
    return 0
  fi

  if [[ "$raw" =~ ^([0-9]+\.[0-9]+\.[0-9]+)(alpha|beta|rc)([0-9]*)$ ]]; then
    local base="${BASH_REMATCH[1]}"
    local channel="${BASH_REMATCH[2]}"
    local number="${BASH_REMATCH[3]}"
    if [[ -n "$number" ]]; then
      printf '%s-%s.%s\n' "$base" "$channel" "$number"
    else
      printf '%s-%s\n' "$base" "$channel"
    fi
    return 0
  fi

  return 1
}

vscode_package_version_from_release_version() {
  local raw="${1:-}"
  raw="${raw#v}"

  if [[ "$raw" =~ ^([0-9]+\.[0-9]+\.[0-9]+) ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}"
    return 0
  fi

  return 1
}

current_cargo_version() {
  local root="${1:-.}"
  sed -n 's/^version = "\([^"]*\)"/\1/p' "$root/Cargo.toml" | head -n 1
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  ref="${1:-${GITHUB_REF_NAME:-}}"
  release_version="$(release_version_from_ref "$ref")"
  if [[ -z "$release_version" ]]; then
    release_version="${MDS_RELEASE_VERSION:-}"
  fi
  if [[ -z "$release_version" ]]; then
    release_version="$(current_cargo_version "$(pwd)")"
  fi
  package_version="$(package_semver_from_release_version "$release_version")"
  vscode_version="$(vscode_package_version_from_release_version "$release_version")"
  printf 'MDS_RELEASE_VERSION=%s\n' "$release_version"
  printf 'MDS_PACKAGE_VERSION=%s\n' "$package_version"
  printf 'MDS_VSCODE_PACKAGE_VERSION=%s\n' "$vscode_version"
fi
