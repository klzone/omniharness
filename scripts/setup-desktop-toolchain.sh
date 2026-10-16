#!/usr/bin/env bash
# OmniDesk 桌面构建链引导（无 root / 不改系统）
#
# 背景：本机未安装 Rust 工具链，也未安装 Tauri 在 Linux 上的构建期依赖
# （libwebkit2gtk-4.1-dev / libgtk-3-dev / libsoup-3.0-dev /
#   libjavascriptcoregtk-4.1-dev 等 -dev 头文件），且 sudo 需要交互密码。
#
# 本脚本用**纯用户级**方式解决：
#   1) Rust 工具链：rustup 装到 $HOME/.cargo + $HOME/.rustup（不写 /usr，不改 shell 配置）
#   2) 构建期依赖：把所需 -dev 包下载为 .deb 并 dpkg-deb -x 解到
#      $HOME/.local/omnidesk-tauri-deps，再用 PKG_CONFIG_SYSROOT_DIR 让它
#      同时看到这些头文件和系统已装的头文件（symlink 合并）。
#
# 安全性：不安装系统包、不提权、不改 /usr、不改 shell profile、不碰 DSH 数据。
#
# 用法：
#   scripts/setup-desktop-toolchain.sh --check     # 只检查，不下载不安装
#   scripts/setup-desktop-toolchain.sh             # 安装（幂等）
#   scripts/setup-desktop-toolchain.sh --print-env # 打印需导出的环境变量

set -euo pipefail

PREFIX="${OMNIDESK_TAURI_DEPS:-$HOME/.local/omnidesk-tauri-deps}"
RUSTUP_HOME_DIR="$HOME/.rustup"
CARGO_HOME_DIR="$HOME/.cargo"
DEB_CACHE="${OMNIDESK_DEB_CACHE:-/tmp/omnidesk-debs}"

# Tauri 2 在 Linux 上的构建期依赖闭包（直接依赖 + pkg-config 传递依赖）
PKGS=(
  libwebkit2gtk-4.1-dev
  libgtk-3-dev
  libsoup-3.0-dev
  libjavascriptcoregtk-4.1-dev
  x11proto-dev
  libbz2-dev
  libx11-dev
  libxext-dev
  libxrender-dev
  libxau-dev
  libxdmcp-dev
  libxfixes-dev
  libxi-dev
  libxtst-dev
  libepoxy-dev
  libegl-dev
  libgbm-dev
)

MODE="install"
case "${1:-}" in
  --check) MODE="check" ;;
  --print-env) MODE="print-env" ;;
  "") ;;
  *) echo "unknown option: $1" >&2; exit 2 ;;
esac

print_env() {
  echo "export PATH=\"$CARGO_HOME_DIR/bin:\$PATH\""
  echo "export PKG_CONFIG_PATH=\"$PREFIX/usr/lib/x86_64-linux-gnu/pkgconfig:$PREFIX/usr/share/pkgconfig:/usr/lib/x86_64-linux-gnu/pkgconfig:/usr/share/pkgconfig\""
  echo "export PKG_CONFIG_LIBDIR=\"\$PKG_CONFIG_PATH\""
  echo "export PKG_CONFIG_SYSROOT_DIR=\"$PREFIX\""
  echo "export LIBRARY_PATH=\"$PREFIX/usr/lib/x86_64-linux-gnu:/usr/lib/x86_64-linux-gnu\""
}

if [ "$MODE" = "print-env" ]; then
  print_env
  exit 0
fi

have_rust=0
if [ -x "$CARGO_HOME_DIR/bin/cargo" ]; then have_rust=1; fi
have_deps=0
if [ -e "$PREFIX/usr/lib/x86_64-linux-gnu/pkgconfig/webkit2gtk-4.1.pc" ]; then have_deps=1; fi

echo "rust  toolchain : $([ $have_rust -eq 1 ] && echo present || echo MISSING) ($CARGO_HOME_DIR)"
echo "tauri deps head: $([ $have_deps -eq 1 ] && echo present || echo MISSING) ($PREFIX)"

if [ "$MODE" = "check" ]; then
  if [ $have_rust -eq 1 ] && [ $have_deps -eq 1 ]; then echo "RESULT: ready"; exit 0; fi
  echo "RESULT: not-ready"
  exit 1
fi

# --- 1. Rust 工具链 -------------------------------------------------------
if [ $have_rust -eq 0 ]; then
  echo "[1/3] 安装 Rust 工具链到 HOME（不写系统目录）…"
  tmp_rustup="$(mktemp -d)/rustup-init"
  curl -sSf -o "$tmp_rustup" \
    https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
  chmod +x "$tmp_rustup"
  # --no-modify-path: 不改 ~/.bashrc / ~/.profile
  "$tmp_rustup" -y --no-modify-path --profile minimal --default-toolchain stable
  rm -rf "$(dirname "$tmp_rustup")"
else
  echo "[1/3] Rust 工具链已存在，跳过。"
fi

# --- 2. 下载构建期依赖 .deb（不需要 root）---------------------------------
if [ $have_deps -eq 0 ]; then
  echo "[2/3] 下载 Tauri 构建期依赖到 $DEB_CACHE …"
  mkdir -p "$DEB_CACHE/partial"
  apt-get -o Dir::Cache::archives="$DEB_CACHE" -o Debug::NoLocking=1 \
    install -y --download-only -qq "${PKGS[@]}"

  echo "[3/3] 解包到私有前缀 $PREFIX …"
  mkdir -p "$PREFIX"
  for f in "$DEB_CACHE"/*.deb; do
    dpkg-deb -x "$f" "$PREFIX" 2>/dev/null || true
  done

  # 解出的 .so 常是指向同目录版本文件的悬空链接；指回系统真实库。
  repaired=0
  while IFS= read -r link; do
    tgt="$(readlink "$link")"
    base="$(basename "$tgt")"
    if [ -e "/usr/lib/x86_64-linux-gnu/$base" ]; then
      ln -sf "/usr/lib/x86_64-linux-gnu/$base" "$link"
      repaired=$((repaired+1))
    fi
  done < <(find "$PREFIX" -name '*.so' -xtype l 2>/dev/null)
  echo "    修复悬空 .so 链接：$repaired"

  # 把系统已装的头文件/库以 symlink 合并进前缀，使前缀成为完整 sysroot 视图。
  merged=0
  for realdir in /usr/include /usr/lib/x86_64-linux-gnu /usr/share/pkgconfig \
                 /usr/lib/pkgconfig /usr/share/aclocal /usr/bin /usr/share/gir-1.0; do
    pre="$PREFIX$realdir"
    mkdir -p "$pre"
    for e in "$realdir"/*; do
      b="$(basename "$e")"
      if [ ! -e "$pre/$b" ] && [ ! -L "$pre/$b" ]; then
        ln -s "$e" "$pre/$b" 2>/dev/null && merged=$((merged+1)) || true
      fi
    done
  done
  echo "    合并系统路径符号链接：$merged"
else
  echo "[2/3][3/3] 构建期依赖前缀已存在，跳过。"
fi

echo
echo "完成。构建前请先导出环境变量："
echo
print_env

