#!/usr/bin/env bash
# ==============================================================================
# OmniDesk / Yvan AI Workbench — 工作站数据与资产备份脚本
# 功能：对 ~/OmniDesk 或 ~/YvanAI 外部资产以及 ~/.dsh 会话进行完整安全镜像归档
# ==============================================================================
set -euo pipefail

TIMESTAMP=$(date +"%Y%m%d_%H%M%S")
BACKUP_ROOT="${1:-$HOME/OmniDesk/backups}"
BACKUP_DEST="$BACKUP_ROOT/backup_${TIMESTAMP}"
DSH_DIR="$HOME/.dsh"
DATA_DIR="$HOME/YvanAI"
OMNI_DATA_DIR="$HOME/OmniDesk"

mkdir -p "$BACKUP_DEST"

echo "================================================================="
echo "       OmniDesk: 工作站数据与配置安全归档 (Backup)               "
echo "================================================================="
echo "归档输出目录: $BACKUP_DEST"

# 1. 备份 DSH 运行时状态与会话日志
if [ -d "$DSH_DIR/sessions" ]; then
  echo -n "[1/4] 正在归档 DSH 会话记录与检查点... "
  mkdir -p "$BACKUP_DEST/dsh_sessions"
  rsync -a --exclude="*.lock" "$DSH_DIR/sessions/" "$BACKUP_DEST/dsh_sessions/"
  echo "OK"
fi

if [ -f "$DSH_DIR/storages/workspace.json" ]; then
  echo -n "[2/4] 正在归档工作区索引 (workspace.json)... "
  cp "$DSH_DIR/storages/workspace.json" "$BACKUP_DEST/workspace.json"
  echo "OK"
fi

# 2. 备份 Profile 定制层与 Patch
if [ -d "$DSH_DIR/profiles/desktop" ]; then
  echo -n "[3/4] 正在归档 Profile 定制配置... "
  mkdir -p "$BACKUP_DEST/profiles_desktop"
  cp -r "$DSH_DIR/profiles/desktop/cordis.patch.yml" "$BACKUP_DEST/profiles_desktop/" 2>/dev/null || true
  cp -r "$DSH_DIR/profiles/desktop/package.json" "$BACKUP_DEST/profiles_desktop/" 2>/dev/null || true
  echo "OK"
fi

# 3. 备份个人外部资产 (~/YvanAI 或 ~/OmniDesk 中的知识库与记忆库)
echo -n "[4/4] 正在归档核心长期记忆与本地资产... "
if [ -d "$DATA_DIR/memory" ]; then
  mkdir -p "$BACKUP_DEST/yvan_memory"
  rsync -a "$DATA_DIR/memory/" "$BACKUP_DEST/yvan_memory/"
fi
if [ -d "$OMNI_DATA_DIR/memory" ]; then
  mkdir -p "$BACKUP_DEST/omni_memory"
  rsync -a "$OMNI_DATA_DIR/memory/" "$BACKUP_DEST/omni_memory/"
fi
echo "OK"

# 生成归档清单元数据
cat > "$BACKUP_DEST/backup_meta.json" << META_EOF
{
  "timestamp": "$TIMESTAMP",
  "created_at": "$(date -Iseconds)",
  "host": "$(hostname)",
  "user": "$(whoami)",
  "dsh_session_count": $(find "$DSH_DIR/sessions" -name "*.zstd" 2>/dev/null | wc -l || echo 0)
}
META_EOF

chmod -R 700 "$BACKUP_DEST" 2>/dev/null || true

echo "================================================================="
echo "  ✅ 备份成功！归档路径: $BACKUP_DEST"
echo "================================================================="

