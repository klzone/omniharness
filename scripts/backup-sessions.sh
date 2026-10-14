#!/usr/bin/env bash
set -euo pipefail

TARGET_DIR="${1:-$HOME/YvanAI/backups/sessions}"
TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
BACKUP_PATH="${TARGET_DIR}/${TIMESTAMP}"
DSH_SESSIONS_DIR="$HOME/.dsh/sessions"

mkdir -p "$BACKUP_PATH"

echo "=== Yvan AI Workbench: Backing up DSH Sessions ==="
echo "Source: $DSH_SESSIONS_DIR"
echo "Destination: $BACKUP_PATH"

if [ -d "$DSH_SESSIONS_DIR" ]; then
  # 增量或镜像备份 sessions 目录
  rsync -av --exclude="*.lock" "$DSH_SESSIONS_DIR/" "$BACKUP_PATH/raw_sessions/"
  echo "Raw session files backed up."
fi

# 备份 workspace 索引
if [ -f "$HOME/.dsh/storages/workspace.json" ]; then
  cp "$HOME/.dsh/storages/workspace.json" "$BACKUP_PATH/workspace.json"
fi

echo "Backup completed successfully at $BACKUP_PATH"

