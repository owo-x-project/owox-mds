#!/usr/bin/env bash
set -euo pipefail

# generate-release-artifacts.sh
#
# Generates release quality gate artifacts for all distribution targets.
# Run after `cargo build --workspace --release && cargo package --allow-dirty -p mds-core && cargo package --allow-dirty -p mds-cli && cargo package --allow-dirty -p mds-lsp`.
#
# Outputs:
#   .build/release/checksums/   — SHA-256 digests
#   .build/release/signatures/  — GPG detached signatures (or placeholder if no key)
#   .build/release/sbom/        — CycloneDX JSON SBOM
#   .build/release/provenance/  — Build provenance attestation (JSONL)
#
# Usage:
#   ./.github/script/generate-release-artifacts.sh [--sign]

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)"; then
  :
else
  ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
fi
VERSION="0.2.1-alpha"
SIGN=false

for arg in "$@"; do
  case "$arg" in
    --sign) SIGN=true ;;
    *) echo "unknown flag: $arg" >&2; exit 1 ;;
  esac
done

RELEASE_DIR="$ROOT/.build/release"
mkdir -p "$RELEASE_DIR"/{checksums,signatures,sbom,provenance}

# ---------- Helper functions ----------

generate_checksum() {
  local file="$1"
  local out="$2"
  if [[ -f "$file" ]]; then
    sha256sum "$file" > "$out"
    echo "  checksum: $out"
  elif [[ -d "$file" ]]; then
    # For directory-based artifacts (VS Code), tar and hash
    tar -cf - -C "$(dirname "$file")" "$(basename "$file")" | sha256sum | sed "s|-|${file}|" > "$out"
    echo "  checksum (dir): $out"
  else
    echo "  WARNING: artifact not found: $file" >&2
  fi
}

generate_signature() {
  local file="$1"
  local out="$2"
  if [[ "$SIGN" == "true" ]] && command -v gpg &>/dev/null; then
    if [[ -f "$file" ]]; then
      gpg --detach-sign --armor --output "$out" "$file"
      echo "  signature: $out"
    elif [[ -d "$file" ]]; then
      # Sign the checksum file instead for directory artifacts
      local checksum_file="${out%.sig}.sha256"
      if [[ -f "$checksum_file" ]]; then
        gpg --detach-sign --armor --output "$out" "$checksum_file"
        echo "  signature (checksum): $out"
      fi
    fi
  else
    # Placeholder for CI signing (alpha: unsigned)
    echo "unsigned-alpha-placeholder: signing deferred to CI pipeline" > "$out"
    echo "  signature (placeholder): $out"
  fi
}

generate_sbom() {
  local name="$1"
  local version="$2"
  local out="$3"
  local component_type="${4:-library}"
  cat > "$out" <<EOF
{
  "bomFormat": "CycloneDX",
  "specVersion": "1.5",
  "serialNumber": "urn:uuid:$(cat /proc/sys/kernel/random/uuid 2>/dev/null || python3 -c 'import uuid; print(uuid.uuid4())')",
  "version": 1,
  "metadata": {
    "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
    "tools": [
      {
        "vendor": "mds",
        "name": "generate-release-artifacts",
        "version": "$VERSION"
      }
    ],
    "component": {
      "type": "$component_type",
      "name": "$name",
      "version": "$version",
      "purl": "pkg:generic/$name@$version"
    }
  },
  "components": []
}
EOF
  echo "  sbom: $out"
}

generate_provenance() {
  local name="$1"
  local version="$2"
  local out="$3"
  cat > "$out" <<EOF
{"_type":"https://in-toto.io/Statement/v0.1","subject":[{"name":"$name","digest":{"sha256":"pending"}}],"predicateType":"https://slsa.dev/provenance/v0.2","predicate":{"builder":{"id":"local"},"buildType":"https://github.com/owo-x-project/owox-mds/build/v1","invocation":{"configSource":{"uri":"https://github.com/owo-x-project/owox-mds","entryPoint":".github/script/generate-release-artifacts.sh"}},"metadata":{"buildStartedOn":"$(date -u +%Y-%m-%dT%H:%M:%SZ)","completeness":{"parameters":true,"environment":false,"materials":false}}}}
EOF
  echo "  provenance: $out"
}

# ---------- Cargo crates ----------

echo "=== Cargo crates ==="

(
  cd "$ROOT"
  cargo build --workspace --release
)
CRATE_DIR="$ROOT/target/package"
for crate in mds-core mds-cli mds-lsp; do
  echo "[$crate]"
  CRATE_FILE="$CRATE_DIR/${crate}-${VERSION}.crate"

  if [[ ! -f "$CRATE_FILE" ]]; then
    # Try to package if not already present.
    echo "  packaging $crate..."
    if ! (cd "$ROOT" && cargo package --allow-dirty --no-verify -p "$crate" >/dev/null); then
      echo "  ERROR: failed to package $crate" >&2
      exit 1
    fi
  fi

  if [[ -f "$CRATE_FILE" ]]; then
    generate_checksum "$CRATE_FILE" "$RELEASE_DIR/checksums/${crate}-${VERSION}.sha256"
  fi
  generate_signature "$CRATE_FILE" "$RELEASE_DIR/signatures/${crate}-${VERSION}.sig"
  generate_sbom "$crate" "$VERSION" "$RELEASE_DIR/sbom/${crate}-${VERSION}.spdx.json" "library"
  generate_provenance "$crate" "$VERSION" "$RELEASE_DIR/provenance/${crate}-${VERSION}.jsonl"
done

# ---------- VS Code extension ----------

echo ""
echo "=== VS Code extension ==="

echo "[mds-vscode]"
"$ROOT/.github/script/package-vscode.sh" --pre-release
VSCODE_DIR="$ROOT/.build/node/vscode"
generate_checksum "$VSCODE_DIR" "$RELEASE_DIR/checksums/mds-vscode-${VERSION}.sha256"
generate_signature "$VSCODE_DIR" "$RELEASE_DIR/signatures/mds-vscode-${VERSION}.sig"
generate_sbom "mds-vscode" "$VERSION" "$RELEASE_DIR/sbom/mds-vscode-${VERSION}.spdx.json" "application"
generate_provenance "mds-vscode" "$VERSION" "$RELEASE_DIR/provenance/mds-vscode-${VERSION}.jsonl"

echo ""
echo "=== Done ==="
echo "Artifacts generated in: .build/release/{checksums,signatures,sbom,provenance}/"

echo ""
echo "Next steps:"
echo "  1. Review generated artifacts"
echo "  2. Run: ./.github/script/release-check.sh --manifest release.mds.toml --verbose"
echo "  3. If --sign was used, verify signatures"
