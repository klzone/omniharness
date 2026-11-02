#!/usr/bin/env bash
# 组装"解压即用"发布包（免安装目录形态）。
#
# 产出目录结构（macOS 示例）：
#   OmniDesk/
#     OmniDesk.app                  桌面程序（前端资源已内嵌）
#     engine/                       Python 引擎（仅标准库依赖）
#     runtime/RUNTIME.lock          DSH 底座版本锁
#     runtime/dsh-runtime/          该平台现装的 DSH CLI（含原生模块）
#     runtime/dsh-home-seed/        首次启动时铺到 ~/OmniDesk/runtime/dsh-home
#     skills/ profiles/ samples/ config/ scripts/
#     启动说明.txt
#
# 前置条件：desktop 已构建完成（tauri build）。
#   macOS：desktop/src-tauri/target/release/bundle/macos/OmniDesk.app
#   Windows：desktop/src-tauri/target/release/omnidesk-desktop.exe
#
# 用法：scripts/build-portable-package.sh [--no-zip]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(dirname "$SCRIPT_DIR")"
VERSION="$(tr -d '[:space:]' < "$ROOT/VERSION")"
OUT_DIR="$ROOT/dist-package"

ZIP=1
for arg in "$@"; do
  case "$arg" in
    --no-zip) ZIP=0 ;;
    *) printf '未知参数：%s\n' "$arg" >&2; exit 2 ;;
  esac
done

# PLATFORM 可由环境变量指定（例如 CI 里打通用包：PLATFORM=macos-universal）
if [ -z "${PLATFORM:-}" ]; then
  case "$(uname -s)" in
    Darwin)
      if [ "$(uname -m)" = "arm64" ]; then PLATFORM="macos-arm64"; else PLATFORM="macos-x64"; fi
      ;;
    MINGW*|MSYS*|CYGWIN*) PLATFORM="windows-x64" ;;
    Linux) PLATFORM="linux-x64" ;;
    *) printf '不认识的系统：%s\n' "$(uname -s)" >&2; exit 1 ;;
  esac
fi

STAGE="$OUT_DIR/.stage-$PLATFORM"
PKG="$STAGE/OmniDesk"
# 每次打包都从干净 stage 开始：残留的旧 stage 会让 cp -R 变成嵌套复制
# （runtime/dsh-runtime/dsh-runtime/...），文件数翻倍且混入旧版本内容。
rm -rf "$STAGE"
mkdir -p "$PKG/runtime"

say() { printf '[package] %s\n' "$*"; }

say "版本 $VERSION / 平台 $PLATFORM"

# 1) 桌面程序
case "$PLATFORM" in
  macos-*) IS_MAC=1 ;;
  *) IS_MAC=0 ;;
esac

if [ "$IS_MAC" -eq 1 ]; then
  APP_SRC=""
  for candidate in \
    "$ROOT/desktop/src-tauri/target/universal-apple-darwin/release/bundle/macos/OmniDesk.app" \
    "$ROOT/desktop/src-tauri/target/release/bundle/macos/OmniDesk.app"
  do
    if [ -d "$candidate" ]; then APP_SRC="$candidate"; break; fi
  done
  [ -n "$APP_SRC" ] || { printf '缺少桌面程序 OmniDesk.app（先跑 tauri build）\n' >&2; exit 1; }
  cp -R "$APP_SRC" "$PKG/"
else
  EXE_SRC=""
  for candidate in \
    "$ROOT/desktop/src-tauri/target/x86_64-pc-windows-msvc/release/omnidesk-desktop.exe" \
    "$ROOT/desktop/src-tauri/target/release/omnidesk-desktop.exe" \
    "$ROOT/desktop/src-tauri/target/release/omnidesk-desktop"
  do
    if [ -f "$candidate" ]; then EXE_SRC="$candidate"; break; fi
  done
  [ -n "$EXE_SRC" ] || { printf '缺少桌面程序可执行文件（先跑 tauri build）\n' >&2; exit 1; }
  if [ "$PLATFORM" = "windows-x64" ]; then
    cp "$EXE_SRC" "$PKG/OmniDesk.exe"
  else
    cp "$EXE_SRC" "$PKG/OmniDesk"
  fi
fi

# 2) 引擎与业务资产（发布包根 = 程序认识的 repo_root）
for dir in engine skills profiles samples config scripts; do
  if [ -d "$ROOT/$dir" ]; then cp -R "$ROOT/$dir" "$PKG/"; fi
done

# 3) DSH 底座
if [ ! -d "$ROOT/runtime/dsh-runtime" ]; then
  printf '缺少 runtime/dsh-runtime（先跑 scripts/bootstrap-runtime.sh）\n' >&2
  exit 1
fi
cp "$ROOT/runtime/RUNTIME.lock" "$PKG/runtime/"
cp -R "$ROOT/runtime/dsh-runtime" "$PKG/runtime/dsh-runtime"

# 3.5) 裁剪 DSH 底座中的非运行时文件。
# npm 安装的包带有 .d.ts / .ts / sourcemap / 测试目录，运行时都不会被加载
# （已实测：裁剪后 dsh --version、--dump-config、真实启动均正常）。
# 裁剪后文件数从约 6.4 万降到约 3 万，免安装包解压不再是"几万个文件"。
say "裁剪 runtime/dsh-runtime 中的非运行时文件..."
PRUNED_PKG="$PKG/runtime/dsh-runtime"
find "$PRUNED_PKG" -name '*.ts' -type f -delete
find "$PRUNED_PKG" -name '*.map' -type f -delete
find "$PRUNED_PKG" -type d \( -name test -o -name tests -o -name __tests__ -o -name coverage \) -prune -exec rm -rf {} + 2>/dev/null || true
say "裁剪完成：$(find "$PRUNED_PKG" -type f | wc -l | tr -d ' ') 个文件"

# 4) 首次启动 seed：Profile + Skill，避免新机器上还要手工 npm
SEED="$PKG/runtime/dsh-home-seed"
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
# seed 里的 profile 依赖与底座同理：运行时只加载 JS，类型/源图/测试目录一并裁剪
find "$SEED" -name '*.ts' -type f -delete
find "$SEED" -name '*.map' -type f -delete
find "$SEED" -type d \( -name test -o -name tests -o -name __tests__ -o -name coverage \) -prune -exec rm -rf {} + 2>/dev/null || true

# 5) 启动说明
cat > "$PKG/启动说明.txt" <<'TXT'
OmniDesk 免安装版
=================

一、运行前需要装两样东西（只需一次）
  1. Node.js 22.19 以上：https://nodejs.org
  2. Python 3.10 以上：https://www.python.org

二、启动
  macOS：双击 OmniDesk.app
         若提示“无法验证开发者”，右键点它 → 打开；或在终端执行一次：
         xattr -dr com.apple.quarantine "/Applications/OmniDesk.app"
  Windows：双击 OmniDesk.exe

三、第一次启动会做什么
  程序会把 runtime/dsh-home-seed 里的 Profile 与 Skill 复制到
  ~/OmniDesk/runtime/dsh-home（Windows 是 %USERPROFILE%\OmniDesk\...）。
  已经存在就完全不覆盖，升级版本时你的数据不受影响。

四、你的数据在哪
  ~/OmniDesk/app          设置、会话、引擎台账
  ~/OmniDesk/runtime      隔离的 DSH 数据根（模型配置与凭据）
  ~/OmniDesk/workspace    成果目录

五、这个包不含任何密钥
  模型 API Key 需要在“设置 → 模型服务”里自己填，第一次对话前必须先填。
TXT

say "组装完成：$PKG"

# 6) 压缩
if [ "$ZIP" -eq 1 ]; then
  OUT="$OUT_DIR/OmniDesk-$VERSION-$PLATFORM.zip"
  if [ "$IS_MAC" -eq 1 ]; then
    ditto -c -k --sequesterRsrc --keepParent "$PKG" "$OUT"
    say "已生成：$OUT"
  elif [ "$PLATFORM" = "windows-x64" ]; then
    WIN_PKG="$(cygpath -w "$PKG")"
    WIN_OUT="$(cygpath -w "$OUT")"
    powershell.exe -NoProfile -Command "Compress-Archive -Path '$WIN_PKG' -DestinationPath '$WIN_OUT' -CompressionLevel Optimal -Force"
    say "已生成：$OUT"
  else
    say "当前系统不自动压缩，请手工把 $PKG 打包为 $OUT"
  fi
fi

