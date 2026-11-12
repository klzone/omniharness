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
if [ -x "$SCRIPT_DIR/runtime-prune-packages.sh" ]; then
  say "裁剪第一批/第二批未使用插件包…"
  bash "$SCRIPT_DIR/runtime-prune-packages.sh" "$RUNTIME" >&2
fi
say "裁剪完成：$(find "$RUNTIME" -type f | wc -l | tr -d ' ') 个文件"

# seed 每次重建：Profile 的 node_modules 要按当前清单重装，残留的旧依赖会让人
# 以为包已经更新，实际跑的还是上一版插件。
rm -rf "$SEED"
for profile in omnidesk-base web discipline; do
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
# Python 模块内置（OD-SYS-704）
#
# anysearch CLI 需要 requests + certifi。发布包不再要求用户往系统 Python
# 里 pip install：构建时把纯 Python 依赖装进 runtime/python-vendor，运行时由
# 宿主把该目录加进 PYTHONPATH，引擎与它拉起的检索子进程都能直接 import。
# ---------------------------------------------------------------------------
PY_REQS="$ROOT/scripts/python-requirements.txt"
VENDOR="$TARGET/python-vendor"
if [ ! -f "$PY_REQS" ]; then
  say "警告：缺少 $PY_REQS，跳过 Python 模块内置"
elif [ -d "$ROOT/runtime/python-vendor" ] && [ "$TARGET" != "$ROOT/runtime" ]; then
  say "复用已构建的 python-vendor：$ROOT/runtime/python-vendor"
  rm -rf "$VENDOR"
  cp -R "$ROOT/runtime/python-vendor" "$VENDOR"
else
  PY_HOST="$(command -v python3 || command -v python || true)"
  if [ -z "$PY_HOST" ]; then
    say "警告：构建机没有 python3，无法内置 requests/certifi"
  else
    say "内置 Python 模块（requests/certifi + python-docx/lxml）→ $VENDOR"
    rm -rf "$VENDOR"
    mkdir -p "$VENDOR"
    if ! "$PY_HOST" -m pip install --no-input --disable-pip-version-check \
      --no-compile --target "$VENDOR" -r "$PY_REQS" >/dev/null 2>&1; then
      say "警告：Python 模块内置失败，anysearch 联网检索可能不可用"
    else
      # universal macOS 包会同时含 arm64/x86_64 两个切片；只删除 charset_normalizer
      # 的架构相关扩展，让它走纯 Python 回退。lxml 的 C 扩展必须保留，否则
      # python-docx 无法 import。
      find "$VENDOR/charset_normalizer" -name '*.so' -type f -delete 2>/dev/null || true
      find "$VENDOR/charset_normalizer" -name '*.pyd' -type f -delete 2>/dev/null || true
    fi
  fi
fi

say "seed 就绪：$(find "$SEED" -type f | wc -l | tr -d ' ') 个文件"

# ---------------------------------------------------------------------------
# 插件通道的 pnpm 内置（OD-ARCH-150）
#
# DSH 的 `plugin --profile <p> add <pkg>` 把参数原样转发给 pnpm，并按 PATH
# 解析 `pnpm`。GUI 启动不继承 shell PATH，发布包必须自带一份，否则用户点
# 「安装插件」只会看到 exit 127「pnpm was not found」。
# ---------------------------------------------------------------------------
PNPM_DIR="$TARGET/pnpm"
PNPM_BIN="$PNPM_DIR/node_modules/.bin/pnpm"
if [ -f "$PNPM_BIN" ] || [ -f "$PNPM_BIN.cmd" ]; then
  say "复用已构建的 pnpm：$PNPM_DIR"
else
  say "内置 pnpm（插件安装通道）→ $PNPM_DIR"
  rm -rf "$PNPM_DIR"
  mkdir -p "$PNPM_DIR"
  printf '{ "name": "omnidesk-runtime-pnpm", "private": true, "version": "0.0.0" }\n' > "$PNPM_DIR/package.json"
  if ! ( cd "$PNPM_DIR" && npm install --no-audit --no-fund "pnpm@11" >/dev/null 2>&1 ); then
    say "警告：pnpm 内置失败，插件安装通道将不可用（基础对话不受影响）"
  fi
fi
