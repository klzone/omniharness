#!/usr/bin/env bash
# ==============================================================================
# OmniDesk Skill 同步（隔离安全版）
#
# 把仓库 skills/ 下的能力同步到 OmniDesk 隔离数据根的 skills 目录，
# 使隔离运行的 OmniDesk 实例可以加载这些自定义 Skill。
#
# 安全约束（相对旧版 sync-skills.sh 的修正）：
#   1. 目标永远由已校验的 OmniDesk 隔离根推导，不接受任意目标路径。
#   2. 任何情况下都拒绝写入 ~/.dsh/skills 或 ~/.agents/skills（原版 DSH 数据）。
#   3. 目标为空、解析异常、或不在允许范围内 -> 立即退出，不做任何写入。
#   4. rsync --delete 只作用于已通过校验的 OmniDesk 目标目录。
#   5. 支持 --dry-run 预览。
#
# 用法:
#   scripts/sync-skills.sh              # 同步到隔离根的 user-dsh skills
#   scripts/sync-skills.sh --dry-run    # 只预览将要发生的改动
#   scripts/sync-skills.sh --to-agents  # 改同步到隔离根的 agents skills
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_DIR="$PROJECT_ROOT/skills"

OMNIDESK_ROOT="${OMNIDESK_ROOT:-$HOME/OmniDesk}"
OMNIDESK_DSH_HOME="$OMNIDESK_ROOT/runtime/dsh-home"
OMNIDESK_AGENTS_HOME="$OMNIDESK_ROOT/runtime/agents-home"

STOCK_DSH_HOME="$HOME/.dsh"
STOCK_AGENTS_HOME="$HOME/.agents"

info() { printf '[OmniDesk][sync] %s\n' "$*"; }
die()  { printf '[OmniDesk][sync][错误] %s\n' "$*" >&2; exit 1; }

abspath() {
  if command -v realpath >/dev/null 2>&1; then
    realpath -m -- "$1"
  else
    printf '%s\n' "$1"
  fi
}

DRY_RUN=0
TARGET_KIND='dsh'
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY_RUN=1; shift ;;
    --to-agents) TARGET_KIND='agents'; shift ;;
    --help|-h)
      sed -n '2,20p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
      exit 0
      ;;
    *) die "未知参数：$1" ;;
  esac
done

command -v rsync >/dev/null 2>&1 || die "未找到 rsync，无法同步。"

# --------------------------------------------------------------------------
# 1. 源目录校验
# --------------------------------------------------------------------------
[ -d "$SOURCE_DIR" ] || die "源 Skills 目录不存在：$SOURCE_DIR"

skill_count=0
for d in "$SOURCE_DIR"/*/; do
  [ -d "$d" ] && skill_count=$((skill_count + 1))
done
[ "$skill_count" -gt 0 ] || die "源 Skills 目录为空：$SOURCE_DIR（拒绝用空源执行 --delete 同步）"

# --------------------------------------------------------------------------
# 2. 目标推导与校验
# --------------------------------------------------------------------------
OMNIDESK_ROOT_ABS="$(abspath "$OMNIDESK_ROOT")"
STOCK_DSH_ABS="$(abspath "$STOCK_DSH_HOME")"
STOCK_AGENTS_ABS="$(abspath "$STOCK_AGENTS_HOME")"

if [ "$TARGET_KIND" = 'agents' ]; then
  BASE_HOME_ABS="$(abspath "$OMNIDESK_AGENTS_HOME")"
else
  BASE_HOME_ABS="$(abspath "$OMNIDESK_DSH_HOME")"
fi

[ -n "$BASE_HOME_ABS" ] || die "隔离根为空，拒绝同步。"
TARGET_DIR="$BASE_HOME_ABS/skills"

# 2.1 目标必须位于 OmniDesk 根之下
case "$TARGET_DIR/" in
  "$OMNIDESK_ROOT_ABS"/*) ;;
  *) die "目标 $TARGET_DIR 不在 OmniDesk 根 $OMNIDESK_ROOT_ABS 之下，拒绝同步。" ;;
esac

# 2.2 目标绝不能位于原版数据根之下
case "$TARGET_DIR/" in
  "$STOCK_DSH_ABS/"*) die "目标落入原版 DSH 数据根 $STOCK_DSH_ABS，拒绝同步。" ;;
esac
case "$TARGET_DIR/" in
  "$STOCK_AGENTS_ABS/"*) die "目标落入原版 Agent 数据根 $STOCK_AGENTS_ABS，拒绝同步。" ;;
esac
if [ "$TARGET_DIR" = "$STOCK_DSH_ABS" ] || [ "$TARGET_DIR" = "$STOCK_AGENTS_ABS" ]; then
  die "目标等于原版数据根，拒绝同步。"
fi

# 2.3 目标不能是软链接（避免穿透到未校验位置）
if [ -L "$TARGET_DIR" ]; then
  die "目标 $TARGET_DIR 是软链接，拒绝同步。"
fi
[ -d "$BASE_HOME_ABS" ] || die "隔离根尚未初始化：$BASE_HOME_ABS（请先运行 scripts/omnidesk-dev.sh --check）"

info "源目录   : $SOURCE_DIR（$skill_count 个 Skill）"
info "目标目录 : $TARGET_DIR"
if [ "$DRY_RUN" -eq 1 ]; then
  info "模式     : dry-run（不写入）"
else
  info "模式     : 实际同步"
fi

mkdir -p "$TARGET_DIR"

# --------------------------------------------------------------------------
# 3. 逐 Skill 同步
# --------------------------------------------------------------------------
RSYNC_FLAGS=(-a --delete --itemize-changes)
if [ "$DRY_RUN" -eq 1 ]; then
  RSYNC_FLAGS+=(--dry-run)
fi

synced=0
for skill_dir in "$SOURCE_DIR"/*/; do
  [ -d "$skill_dir" ] || continue
  skill_name="$(basename "$skill_dir")"
  info "同步 Skill: $skill_name"
  rsync "${RSYNC_FLAGS[@]}" "$skill_dir" "$TARGET_DIR/$skill_name/"
  synced=$((synced + 1))
done

info "完成：共处理 $synced 个 Skill -> $TARGET_DIR"

