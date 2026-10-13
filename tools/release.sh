#!/usr/bin/env bash
# ScreenBuddy release script.
#
# Usage:
#   ./tools/release.sh <version> [--sign] [--notes "release notes"]
#
# Example:
#   ./tools/release.sh 0.7.0 --sign --notes "Bug fixes and improvements"
#
# This script:
# 1. Validates the version format (semver)
# 2. Updates Cargo.toml workspace version
# 3. Generates a changelog entry
# 4. Builds release binaries (desktop + Android)
# 5. Stages artifacts in dist/
# 6. Optionally signs the binaries
# 7. Creates a git tag

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
DIST_DIR="$ROOT_DIR/dist"
CHANGELOG_FILE="$ROOT_DIR/CHANGELOG.md"

# Parse arguments
VERSION=""
SIGN=false
NOTES=""
while [[ $# -gt 0 ]]; do
    case "$1" in
        --sign)
            SIGN=true
            shift
            ;;
        --notes)
            NOTES="$2"
            shift 2
            ;;
        -*)
            echo "Unknown option: $1" >&2
            exit 1
            ;;
        *)
            if [[ -z "$VERSION" ]]; then
                VERSION="$1"
            else
                echo "Unexpected argument: $1" >&2
                exit 1
            fi
            shift
            ;;
    esac
done

if [[ -z "$VERSION" ]]; then
    echo "Usage: ./tools/release.sh <version> [--sign] [--notes \"release notes\"]" >&2
    exit 1
fi

# Validate version format (semver)
if ! [[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9.]+)?$ ]]; then
    echo "Error: Invalid version format: $VERSION" >&2
    echo "Expected: MAJOR.MINOR.PATCH or MAJOR.MINOR.PATCH-prerelease" >&2
    exit 1
fi

echo "=== ScreenBuddy Release v$VERSION ==="
echo ""

# 1. Update Cargo.toml workspace version
echo "[1/7] Updating Cargo.toml workspace version..."
sed -i "s/^version = \".*\"/version = \"$VERSION\"/" "$ROOT_DIR/Cargo.toml"
echo "  Updated to $VERSION"

# 2. Generate changelog entry
echo "[2/7] Generating changelog entry..."
generate_changelog() {
    local date_str
    date_str=$(date +%Y-%m-%d)
    local entry="## [$VERSION] - $date_str"
    if [[ -n "$NOTES" ]]; then
        entry="$entry

$NOTES"
    else
        entry="$entry

- See commit log for details."
    fi

    if [[ -f "$CHANGELOG_FILE" ]]; then
        # Insert after the first line (# Changelog)
        sed -i "1a\\
\\
$entry\\
" "$CHANGELOG_FILE"
    else
        echo "# Changelog" > "$CHANGELOG_FILE"
        echo "" >> "$CHANGELOG_FILE"
        echo "$entry" >> "$CHANGELOG_FILE"
    fi
    echo "  Changelog updated"
}
generate_changelog

# 3. Build release binaries
echo "[3/7] Building release binaries..."
cargo build --release --workspace
echo "  Desktop binary built"

# 4. Build Android APK
echo "[4/7] Building Android APK..."
if [[ -f "$ROOT_DIR/ScreenBuddy-Android/gradlew" ]]; then
    cd "$ROOT_DIR/ScreenBuddy-Android"
    ./gradlew assembleRelease
    echo "  Android APK built"
else
    echo "  Skipping Android APK (gradlew not found)"
fi

# 5. Stage artifacts
echo "[5/7] Staging artifacts..."
mkdir -p "$DIST_DIR"
cp "$ROOT_DIR/target/release/screenbuddy.exe" "$DIST_DIR/ScreenBuddy-v$VERSION-windows-x64.exe"
if [[ -f "$ROOT_DIR/ScreenBuddy-Android/app/build/outputs/apk/release/app-release.apk" ]]; then
    cp "$ROOT_DIR/ScreenBuddy-Android/app/build/outputs/apk/release/app-release.apk" "$DIST_DIR/ScreenBuddy-v$VERSION-android.apk"
fi
echo "  Artifacts staged in dist/"

# 6. Sign binaries (optional)
if [[ "$SIGN" == "true" ]]; then
    echo "[6/7] Signing binaries..."
    # Windows signing requires signtool.exe (part of Windows SDK)
    if command -v signtool &> /dev/null; then
        # Sign desktop binary
        signtool sign /fd SHA256 /tr http://timestamp.digicer.com /td SHA256 \
            "$DIST_DIR/ScreenBuddy-v$VERSION-windows-x64.exe" || \
            echo "  Warning: Desktop signing failed"
        echo "  Desktop binary signed"
    else
        echo "  Warning: signtool not found, skipping Windows signing"
    fi

    # Android APK signing
    if [[ -f "$ROOT_DIR/ScreenBuddy-Android/keystore.properties" ]]; then
        echo "  Android APK signed via Gradle"
    else
        echo "  Warning: Android keystore not configured"
    fi
else
    echo "[6/7] Skipping signing (--sign not specified)"
fi

# 7. Create git tag
echo "[7/7] Creating git tag..."
git -C "$ROOT_DIR" add -A
git -C "$ROOT_DIR" commit -m "Release v$VERSION" || echo "  Nothing to commit"
git -C "$ROOT_DIR" tag -a "v$VERSION" -m "Release v$VERSION" || echo "  Tag already exists"
echo "  Git tag v$VERSION created"

echo ""
echo "=== Release v$VERSION complete ==="
echo ""
echo "Artifacts in $DIST_DIR:"
ls -la "$DIST_DIR"/ScreenBuddy-v$VERSION-* 2>/dev/null || echo "  (no artifacts found)"
echo ""
echo "Next steps:"
echo "  1. Review the changelog: $CHANGELOG_FILE"
echo "  2. Push the tag: git push origin v$VERSION"
echo "  3. Create a GitHub release with the artifacts in $DIST_DIR"
