#!/usr/bin/env bash
# 第一批/第二批运行时插件剪枝。
#
# 只移动包目录，不删除文件：默认备份到 /tmp，随后由 Tauri 只复制
# runtime/dsh-runtime 进 App。保留 web-frontend 和目录选择器原生桥接包，
# 因为 dsh-web-app 与 directory-picker-auto 对它们有硬依赖。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RUNTIME="${1:?用法: $0 <dsh-runtime 目录> [备份目录]}"
BACKUP="${2:-/tmp/omnidesk-dsh-pruned-$(date '+%Y%m%dT%H%M%S')}"

[ -d "$RUNTIME/node_modules" ] || {
  echo "[prune] 缺少 $RUNTIME/node_modules" >&2
  exit 1
}
mkdir -p "$BACKUP"

move_package() {
  local path="$1" rel dest
  [ -d "$path" ] || return 0
  rel="${path#"$RUNTIME"/}"
  dest="$BACKUP/$rel"
  mkdir -p "$(dirname "$dest")"
  mv "$path" "$dest"
  echo "[prune] moved $rel"
}

# 顶层原生/终端/语音包。
for path in "$RUNTIME"/node_modules/sherpa-onnx-* \
            "$RUNTIME"/node_modules/node-pty; do
  [ -e "$path" ] && move_package "$path"
done

# dsh-experimental 包全部不进入交付底座。
for path in "$RUNTIME"/node_modules/@deepseek-ai/dsh-experimental-*; do
  [ -e "$path" ] && move_package "$path"
done

# client UI 只保留 directory-picker-native；其余 UI 不随自有 Tauri UI 交付。
for path in "$RUNTIME"/node_modules/@deepseek-ai/dsh-client-ui-*; do
  [ -e "$path" ] || continue
  case "$(basename "$path")" in
    dsh-client-ui-directory-picker-native) continue ;;
  esac
  move_package "$path"
done

# 终端/终端控制器的非 UI 包。
for name in dsh-terminal dsh-terminal-bash dsh-api-terminal-controller; do
  move_package "$RUNTIME/node_modules/@deepseek-ai/$name"
done

echo "[prune] 备份目录：$BACKUP"
echo "[prune] 完成"
