#!/usr/bin/env bash
# ==============================================================================
# OmniDesk 运行底座引导（download-and-run）
#
# 目的：让任何人克隆本仓库后，用一条命令得到与维护者「经过测试的同一版本」DSH
#       可运行 CLI，无需另外去官网找安装包，也不会碰系统里已有的 DSH。
#
# 设计约束：
#   1. 只写仓库内 runtime/dsh-runtime（已被 .gitignore 排除），不写系统目录。
#   2. 版本来自 runtime/RUNTIME.lock，而不是「最新版」，保证可复现。
#   3. 绝不触碰 ~/.dsh、~/.agents 或系统 Desktop 安装。
#   4. 已安装且版本一致时直接跳过，不重复下载。
#   5. 需要网络与 node（>=22.19）；不满足时明确报错，不做任何降级猜测。
#
# 用法:
#   scripts/bootstrap-runtime.sh            # 安装/校验锁定版本底座
#   scripts/bootstrap-runtime.sh --check    # 只校验，不安装
#   scripts/bootstrap-runtime.sh --force    # 重新安装
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
LOCK_FILE="$PROJECT_ROOT/runtime/RUNTIME.lock"
INSTALL_DIR="$PROJECT_ROOT/runtime/dsh-runtime"

info() { printf '[OmniDesk][bootstrap] %s\n' "$*"; }
warn() { printf '[OmniDesk][bootstrap][警告] %s\n' "$*" >&2; }
die()  { printf '[OmniDesk][bootstrap][错误] %s\n' "$*" >&2; exit 1; }

CHECK_ONLY=0
FORCE=0
while [ $# -gt 0 ]; do
  case "$1" in
    --check) CHECK_ONLY=1; shift ;;
    --force) FORCE=1; shift ;;
    --help|-h) sed -n '2,24p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'; exit 0 ;;
    *) printf '未知参数：%s\n' "$1" >&2; exit 2 ;;
  esac
done

[ -f "$LOCK_FILE" ] || die "缺少锁定文件：$LOCK_FILE"
command -v node >/dev/null 2>&1 || die "未找到 node（需要 ^22.19.0 或 >=24）。"
command -v npm  >/dev/null 2>&1 || die "未找到 npm。"

# 从 RUNTIME.lock 读取版本（只取 runtime: 段的 version:）
LOCK_VERSION="$(awk '/^runtime:/{f=1;next} /^[a-z]/{if(f)f=0} f && /^[[:space:]]+version:/{gsub(/["'"'"' ]/,"",$2); print $2; exit}' "$LOCK_FILE")"
[ -n "$LOCK_VERSION" ] || die "无法从 RUNTIME.lock 解析版本号。"

info "锁定版本 : $LOCK_VERSION"
info "安装目录 : $INSTALL_DIR"

bin_js="$INSTALL_DIR/node_modules/@deepseek-ai/dsh/lib/bin.js"

installed_version() {
  [ -f "$bin_js" ] || { printf ''; return; }
  node "$bin_js" --version 2>/dev/null | head -1 | tr -d '\r'
}

CURRENT="$(installed_version)"
if [ -n "$CURRENT" ] && [ "$CURRENT" = "$LOCK_VERSION" ] && [ "$FORCE" -eq 0 ]; then
  info "已安装且版本一致（$CURRENT），无需重新安装。"
  exit 0
fi

if [ "$CHECK_ONLY" -eq 1 ]; then
  if [ "$CURRENT" = "$LOCK_VERSION" ]; then
    info "--check：底座版本一致（$CURRENT）。"
    exit 0
  fi
  warn "--check：底座未安装或版本不符（当前：${CURRENT:-未安装}，期望：$LOCK_VERSION）。"
  exit 1
fi

mkdir -p "$INSTALL_DIR"
if [ ! -f "$INSTALL_DIR/package.json" ]; then
  ( cd "$INSTALL_DIR" && npm init -y >/dev/null 2>&1 )
fi
info "正在安装 @deepseek-ai/dsh@$LOCK_VERSION（首次约需数分钟）..."
( cd "$INSTALL_DIR" && npm install --no-audit --no-fund "@deepseek-ai/dsh@$LOCK_VERSION" )

FINAL="$(installed_version)"
[ "$FINAL" = "$LOCK_VERSION" ] || die "安装后版本校验失败：得到 '${FINAL:-空}'，期望 '$LOCK_VERSION'。"
info "安装完成并通过版本校验：$FINAL"
info "启动隔离实例：scripts/omnidesk-dev.sh"
