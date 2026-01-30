#!/bin/bash
# Release script: bump patch version, commit, tag, and push
# Run from project root: ./scripts/release.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_ROOT"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "=== Release Script ==="
echo ""

# Check we're on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${RED}Error: Not on main branch (currently on '$CURRENT_BRANCH')${NC}"
    echo "Please switch to main branch before releasing."
    exit 1
fi
echo -e "${GREEN}✓${NC} On main branch"

# Check working directory is clean
if [ -n "$(git status --porcelain)" ]; then
    echo -e "${RED}Error: Working directory is not clean${NC}"
    echo "Please commit or stash your changes before releasing."
    git status --short
    exit 1
fi
echo -e "${GREEN}✓${NC} Working directory clean"

# Check we're up to date with remote
git fetch origin main --quiet
LOCAL=$(git rev-parse HEAD)
REMOTE=$(git rev-parse origin/main)
if [ "$LOCAL" != "$REMOTE" ]; then
    echo -e "${RED}Error: Local main is not up to date with origin/main${NC}"
    echo "Please pull or push changes before releasing."
    exit 1
fi
echo -e "${GREEN}✓${NC} Up to date with remote"

# Get current version from Cargo.toml
CURRENT_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
echo ""
echo "Current version: $CURRENT_VERSION"

# Parse version components
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Bump patch version
NEW_PATCH=$((PATCH + 1))
NEW_VERSION="$MAJOR.$MINOR.$NEW_PATCH"
echo "New version:     $NEW_VERSION"
echo ""

# Confirm with user
read -p "Proceed with release v$NEW_VERSION? [y/N] " -n 1 -r
echo ""
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Release cancelled."
    exit 0
fi

echo ""
echo "=== Updating versions ==="

# Update Cargo.toml
sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" Cargo.toml
echo -e "${GREEN}✓${NC} Updated Cargo.toml"

# Update pyproject.toml
sed -i '' "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" pyproject.toml
echo -e "${GREEN}✓${NC} Updated pyproject.toml"

# Update Cargo.lock
cargo check --quiet
echo -e "${GREEN}✓${NC} Updated Cargo.lock"

# Verify versions match
CARGO_VERSION=$(grep '^version = ' Cargo.toml | head -1 | sed 's/version = "\(.*\)"/\1/')
PYPROJECT_VERSION=$(grep '^version = ' pyproject.toml | head -1 | sed 's/version = "\(.*\)"/\1/')

if [ "$CARGO_VERSION" != "$NEW_VERSION" ] || [ "$PYPROJECT_VERSION" != "$NEW_VERSION" ]; then
    echo -e "${RED}Error: Version mismatch after update${NC}"
    echo "  Cargo.toml:     $CARGO_VERSION"
    echo "  pyproject.toml: $PYPROJECT_VERSION"
    echo "  Expected:       $NEW_VERSION"
    git checkout Cargo.toml pyproject.toml
    exit 1
fi
echo -e "${GREEN}✓${NC} Versions verified"

echo ""
echo "=== Committing and tagging ==="

# Commit changes
git add Cargo.toml Cargo.lock pyproject.toml
git commit -m "Release v$NEW_VERSION"
echo -e "${GREEN}✓${NC} Committed changes"

# Create tag
git tag -a "v$NEW_VERSION" -m "Release v$NEW_VERSION"
echo -e "${GREEN}✓${NC} Created tag v$NEW_VERSION"

echo ""
echo "=== Pushing to remote ==="

# Push commit and tag
git push origin main
git push origin "v$NEW_VERSION"
echo -e "${GREEN}✓${NC} Pushed to origin"

echo ""
echo -e "${GREEN}=== Release v$NEW_VERSION complete! ===${NC}"
