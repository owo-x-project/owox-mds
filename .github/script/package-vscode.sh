#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
SOURCE_DIR="$ROOT/editors/vscode"
BUILD_DIR="$ROOT/.build/node/vscode"
PACKAGE_DIR="$BUILD_DIR/package"
PRE_RELEASE=false
TARGET=""
LSP_BINARY=""
PACKAGE_VERSION=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --pre-release)
      PRE_RELEASE=true
      shift
      ;;
    --target)
      TARGET="${2:-}"
      if [[ -z "$TARGET" ]]; then
        echo "--target requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --lsp-binary)
      LSP_BINARY="${2:-}"
      if [[ -z "$LSP_BINARY" ]]; then
        echo "--lsp-binary requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    --version)
      PACKAGE_VERSION="${2:-}"
      if [[ -z "$PACKAGE_VERSION" ]]; then
        echo "--version requires a value" >&2
        exit 1
      fi
      shift 2
      ;;
    *) echo "unknown flag: $1" >&2; exit 1 ;;
  esac
done

rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR" "$BUILD_DIR"

(cd "$SOURCE_DIR" && npm run compile)

cp "$SOURCE_DIR/package.json" "$PACKAGE_DIR/package.json"
PACKAGE_VERSION="$PACKAGE_VERSION" node -e "const fs=require('fs'); const p='${PACKAGE_DIR}/package.json'; const explicitVersion=process.env.PACKAGE_VERSION; const pkg=require(p); if (pkg.scripts) delete pkg.scripts.vscode_prepublish; if (pkg.scripts) delete pkg.scripts['vscode:prepublish']; const rawVersion=String(explicitVersion || pkg.version).replace(/^v/, ''); const versionMatch=rawVersion.match(/^(\d+\.\d+\.\d+)/); if (!versionMatch) { throw new Error('Invalid VS Code extension version source: ' + rawVersion); } pkg.version = versionMatch[1]; fs.writeFileSync(p, JSON.stringify(pkg, null, 2) + '\n');"
cp "$SOURCE_DIR/README.md" "$PACKAGE_DIR/README.md"
cp "$SOURCE_DIR/CHANGELOG.md" "$PACKAGE_DIR/CHANGELOG.md"
cp "$SOURCE_DIR/LICENSE" "$PACKAGE_DIR/LICENSE"
cp "$SOURCE_DIR/language-configuration.json" "$PACKAGE_DIR/language-configuration.json"
cp "$SOURCE_DIR/.vscodeignore" "$PACKAGE_DIR/.vscodeignore"
cp -R "$SOURCE_DIR/snippets" "$PACKAGE_DIR/snippets"
cp -R "$SOURCE_DIR/syntaxes" "$PACKAGE_DIR/syntaxes"
cp -R "$SOURCE_DIR/out" "$PACKAGE_DIR/out"

if [[ -n "$LSP_BINARY" ]]; then
  if [[ -z "$TARGET" ]]; then
    echo "--lsp-binary requires --target" >&2
    exit 1
  fi
  if [[ ! -f "$LSP_BINARY" ]]; then
    echo "LSP binary not found: $LSP_BINARY" >&2
    exit 1
  fi
  mkdir -p "$PACKAGE_DIR/server/$TARGET"
  cp "$LSP_BINARY" "$PACKAGE_DIR/server/$TARGET/$(basename "$LSP_BINARY")"
  if [[ "$TARGET" != win32-* ]]; then
    chmod +x "$PACKAGE_DIR/server/$TARGET/$(basename "$LSP_BINARY")"
  fi
fi

(cd "$PACKAGE_DIR" && npm install --omit=dev --ignore-scripts --package-lock=false)

VSCE_ARGS=(package --out "$BUILD_DIR")
if [[ -n "$TARGET" ]]; then
  VSCE_ARGS+=(--target "$TARGET")
fi
if [[ "$PRE_RELEASE" == "true" ]]; then
  VSCE_ARGS+=(--pre-release)
fi

(cd "$PACKAGE_DIR" && npx @vscode/vsce "${VSCE_ARGS[@]}")
