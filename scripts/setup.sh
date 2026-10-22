#!/usr/bin/env bash
# ==============================================================================
# OmniDesk 一键初始化（download-and-run）
#
# 目的：克隆本仓库后，用一条命令完成「准备底座 -> 初始化数据骨架 -> 同步技能」，
#       得到一个可启动、与你验证版本一致的 OmniDesk 环境。
#
# 设计约束：
#   1. 只在仓库内与 ~/YvanAI（外部数据）写入，绝不触碰 ~/.dsh、~/.agents 或系统 DSH。
#   2. 底座版本来自 runtime/RUNTIME.lock，不追「最新版」。
#   3. 任一步失败即停止，并说明如何手动重跑该步。
#   4. 不询问、不删除、不迁移任何已有 DSH 数据。
#
# 关于 ~/YvanAI（历史数据根，口径定案见 OD-ARCH-056）：
#   ~/YvanAI 是本项目早期的个人数据根。那里的既有数据继续被**只读**使用：
#   不迁移、不改名、不覆盖、不删除。新部署仍然初始化到该路径，是为了让
#   init-yvan-data.sh 与既有安装保持同一行为（改了默认路径会让老机器被
#   当成新机器，产生第二套骨架）。
#   **桌面端成果不写这个目录**：OmniDesk 的新成果落在
#   ~/OmniDesk/workspace/artifacts（或用户在设置页绑定的 vault 目录），
#   见 docs/OmniDesk-Desktop-Data-Layout.md 第 5 节。
#
# 用法:
#   scripts/setup.sh            # 完整初始化
#   scripts/setup.sh --check    # 只检查现状，不写入
# ==============================================================================
set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

info() { printf '\n[OmniDesk][setup] %s\n' "$*"; }
die()  { printf '[OmniDesk][setup][错误] %s\n' "$*" >&2; exit 1; }

CHECK_ONLY=0
[ "${1:-}" = "--check" ] && CHECK_ONLY=1
CHECK_FAILED=0

run_check() {
  local label="$1"
  shift
  info "检查：$label"
  if "$@"; then
    return 0
  else
    local rc=$?
    CHECK_FAILED=1
    printf "[OmniDesk][setup][错误] %s 检查失败（退出码 %s）。\n" "$label" "$rc" >&2
    return 0
  fi
}

info "1/3 校验并准备运行底座（锁定版本）"
if [ "$CHECK_ONLY" -eq 1 ]; then
  run_check "运行底座" bash "$SCRIPT_DIR/bootstrap-runtime.sh" --check
else
  bash "$SCRIPT_DIR/bootstrap-runtime.sh" || die "底座准备失败；可手动重跑：scripts/bootstrap-runtime.sh"
fi

info "2/3 初始化外部数据骨架（~/YvanAI）"
if [ "$CHECK_ONLY" -eq 1 ]; then
  info "（--check）跳过数据骨架初始化。"
else
  [ -x "$SCRIPT_DIR/init-yvan-data.sh" ] && bash "$SCRIPT_DIR/init-yvan-data.sh" || info "跳过（未找到 init-yvan-data.sh）。"
fi

info "3/3 同步技能到隔离运行时"
if [ "$CHECK_ONLY" -eq 1 ]; then
  run_check "Skill 同步状态" bash "$SCRIPT_DIR/sync-skills.sh" --check
else
  bash "$SCRIPT_DIR/sync-skills.sh" || die "技能同步失败；可手动重跑：scripts/sync-skills.sh"
fi

if [ "$CHECK_ONLY" -eq 1 ] && [ "$CHECK_FAILED" -ne 0 ]; then
  die "只读检查未通过；请根据上方错误修复后重新运行 scripts/setup.sh --check。"
fi

if [ "$CHECK_ONLY" -eq 1 ]; then
  info "只读检查完成。"
else
  info "初始化完成。启动隔离实例：scripts/omnidesk-dev.sh"
fi
