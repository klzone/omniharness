#!/usr/bin/env bash
set -euo pipefail

BASE_DIR="${1:-$HOME/YvanAI}"

echo "=== Initializing YvanAI data structure at $BASE_DIR ==="
mkdir -p "$BASE_DIR"/{knowledge,memory,projects,templates,profiles,backups}
mkdir -p "$BASE_DIR/memory"/{daily,decisions,summaries}
mkdir -p "$BASE_DIR/projects"/{V2T,smart-inspection,website,ai-tools}
mkdir -p "$BASE_DIR/knowledge"/{obsidian-symlinks,references,notes}

echo "Created directories:"
tree -L 2 "$BASE_DIR" || ls -la "$BASE_DIR"

