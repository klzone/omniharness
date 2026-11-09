#!/usr/bin/env bash
# 准备随包分发的运行时资产（OD-ARCH-135）。
#
# 安装包与免安装包要带同一套东西：DSH CLI 底座、首启 seed（Profile 依赖 + Skill）。
# 两处各写一遍必然漂移——Windows 安装器这次就是因为「只有打包脚本知道这些目录」
# 而只装了一个 exe。这里统一成一份准备逻辑，参数是要准备的 runtime 目录：
#
#   scripts/prepare-runtime-assets.sh [runtime 目录，默认仓库的 runtime/]
#
# 做三件事，全部幂等：
#   1. 裁剪 dsh-runtime：*.ts / *.map / 测试目录运行时不会加载；
#   2. 重建 dsh-home-seed/profiles/<profile>：按 samples 的清单安装依赖；
#   3. 重建 dsh-home-seed/skills：仓库 skills 原样复制。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
TARGET="${1:-$ROOT/runtime}"
RUNTIME="$TARGET/dsh-runtime"
SEED="$TARGET/dsh-home-seed"

say() { printf '[runtime] %s\n' "$*" >&2; }
die() { printf '[runtime] %s\n' "$*" >&2; exit 1; }

[ -d "$RUNTIME" ] || die "缺少 $RUNTIME（先跑 scripts/bootstrap-runtime.sh）"

prune() {
  local dir="$1"
  find "$dir" -name '*.ts' -type f -delete
  find "$dir" -name '*.map' -type f -delete
  find "$dir" -type d \( -name test -o -name tests -o -name __tests__ -o -name coverage \) -prune -exec rm -rf {} + 2>/dev/null || true
}

say "裁剪 $RUNTIME 中的非运行时文件…"
prune "$RUNTIME"
say "裁剪完成：$(find "$RUNTIME" -type f | wc -l | tr -d ' ') 个文件"

# seed 每次重建：Profile 的 node_modules 要按当前清单重装，残留的旧依赖会让人
# 以为包已经更新，实际跑的还是上一版插件。
rm -rf "$SEED"
for profile in omnidesk-base web; do
  mkdir -p "$SEED/profiles/$profile"
  cp "$ROOT/samples/profiles/$profile.profile.json" "$SEED/profiles/$profile/package.json"
  cp "$ROOT/samples/profiles/$profile.profile.patch.yml" "$SEED/profiles/$profile/cordis.patch.yml"
  say "安装 Profile 依赖：$profile"
  if ! ( cd "$SEED/profiles/$profile" && npm install --no-audit --no-fund --omit=dev >/dev/null 2>&1 ); then
    say "警告：$profile 依赖安装失败，该 Profile 插件不可用（基础对话不受影响）"
  fi
done

mkdir -p "$SEED/skills"
for skill_dir in "$ROOT"/skills/*/; do
  if [ -d "$skill_dir" ]; then cp -R "$skill_dir" "$SEED/skills/"; fi
done
prune "$SEED"

# ---------------------------------------------------------------------------
# Python 模块补齐（OD-SYS-702）
#
# 随包的 primary-runtime payload 由 DSH 官方构建器生成，它的 runtime.json
# pythonPackages 是唯一清单来源；本仓库不维护那份清单，也不生成该 payload。
# anysearch CLI 额外需要 requests + certifi：payload 存在时就地补装，
# 不存在时只提示——OmniDesk 当前使用系统 Python，环境检查会给出 pip 命令。
# ---------------------------------------------------------------------------
PY_REQS="$ROOT/scripts/python-requirements.txt"
PR_PY=""
for candidate in \
  "$TARGET/primary-runtime/dependencies/python/bin/python3" \
  "$TARGET/primary-runtime/dependencies/python/bin/python" \
  "$TARGET/primary-runtime/dependencies/python/python.exe"
do
  if [ -f "$candidate" ]; then PR_PY="$candidate"; break; fi
done
if [ ! -f "$PY_REQS" ]; then
  say "警告：缺少 $PY_REQS，跳过 Python 模块补齐"
elif [ -n "$PR_PY" ]; then
  say "补装 Python 模块（requests/certifi）到随包解释器：$PR_PY"
  if ! "$PR_PY" -m pip install --no-input --disable-pip-version-check -r "$PY_REQS" >/dev/null 2>&1; then
    say "警告：Python 模块安装失败，anysearch 联网检索可能不可用（环境检查会标出）"
  fi
else
  say "未发现随包 primary-runtime；当前使用系统 Python，requests 缺失时由环境检查提示 pip install"
fi

say "seed 就绪：$(find "$SEED" -type f | wc -l | tr -d ' ') 个文件"
