#!/usr/bin/env bash
set -euo pipefail

# 同步项目内的 skills 到 ~/.dsh/skills，使得正在运行或重启的 DSH 可以立刻无缝加载这些自定义能力
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DSH_SKILLS_DIR="$HOME/.dsh/skills"

mkdir -p "$DSH_SKILLS_DIR"
echo "=== Syncing skills from $PROJECT_ROOT/skills to $DSH_SKILLS_DIR ==="

for skill_dir in "$PROJECT_ROOT"/skills/*; do
  if [ -d "$skill_dir" ]; then
    skill_name="$(basename "$skill_dir")"
    echo "Syncing skill: $skill_name"
    rsync -av --delete "$skill_dir/" "$DSH_SKILLS_DIR/$skill_name/"
  fi
done

echo "Skills sync complete."

