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

# OD-SLIM-022（0.2.2 二次精简）：以下为升级新增、产品未使用的模块。
# 0.2.1 已有的 subagent / schedule / Office 能力按用户要求一并移出（策略 B），
# 对应 profile patch 中的启用条目已同步删除。

# Office 转换：OmniDesk 阅读用 Python、写作用系统 soffice/textutil，不加载 DSH office 工具。
for path in "$RUNTIME"/node_modules/@deepseek-ai/libreoffice-kit-*; do
  [ -e "$path" ] && move_package "$path"
done
for name in dsh-office-kit dsh-tool-office dsh-skill-office; do
  move_package "$RUNTIME/node_modules/@deepseek-ai/$name"
done

# 子代理与 AgentTeam。
for name in dsh-subagent dsh-subagent-fork-in-process dsh-subagent-spawn-in-process \
            dsh-tool-subagent dsh-tool-subagent-control \
            dsh-agent-team dsh-tool-agent-team; do
  move_package "$RUNTIME/node_modules/@deepseek-ai/$name"
done

# DSH 调度：OmniDesk 的定时任务在 Rust scheduler 里，不用 DSH schedule。
for name in dsh-schedule dsh-tool-schedule; do
  move_package "$RUNTIME/node_modules/@deepseek-ai/$name"
done

# 0.2.2 新增的 extras。
for name in dsh-cua dsh-cua-bundle dsh-computer-use dsh-deepseek-chat-import \
            dsh-creation-recommendations dsh-personalized-recommendations dsh-plugin-recommendations \
            dsh-terminal-panel dsh-action-cards dsh-plugin-detail dsh-mcp-manager \
            dsh-plugin-manager dsh-plugin-market; do
  [ -e "$RUNTIME/node_modules/@deepseek-ai/$name" ] && move_package "$RUNTIME/node_modules/@deepseek-ai/$name"
done

echo "[prune] 备份目录：$BACKUP"
echo "[prune] 完成"
