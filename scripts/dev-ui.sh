#!/usr/bin/env bash
# OmniDesk UI v2 本地开发启动器（仅隔离合成环境）
#
# 作用：一条命令拉起「前端 dev server + 隔离数据根 + 合成模型端点 + 桌面 App」，
#       用于在真实窗口里查看 UI、验证合成样例流程。
#
# 硬边界（OD-ARCH-004）：
#   * 只用本机回环「合成」模型端点（零凭证、零外网）；不提供接入真实端点的开关。
#   * 数据根固定在 $OMNIDESK_DEV_ROOT（默认 /tmp/omnidesk-dev/<运行名>），不触碰真实应用数据。
#   * 任何失败都清楚退出并回收本次已启动的进程，不留半启动实例。
#   * 不创建、不修改仓库 runtime/ 下的任何内容；底座缺失时直接报错退出。
#   * PID 文件按运行目录独立；停止前逐个核对进程身份，绝不误杀别的实例。
#
# 用法：
#   scripts/dev-ui.sh                 # 启动（合成端点）
#   scripts/dev-ui.sh --no-build      # 跳过 cargo build（已有二进制时更快）
#   scripts/dev-ui.sh --status        # 查看本次启动状态
#   scripts/dev-ui.sh --stop          # 停止本次启动的进程（先核对身份）
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPDIR="$REPO/desktop"
# 运行目录名不得使用仓库目录名：宿主会拒绝含 "omniharness" / "myworkstation"
# 等片段的本地数据根（防止写回仓库或原版 DSH 目录），而本仓库目录名正好命中，
# 会让“阶段 5 · 保存”永远被拒。改用不含禁入词的稳定前缀 + 工作区路径短哈希，
# 既避开禁入词，又让同一台机器上的多个 worktree 各自独立、不会互相覆盖。
WORKSPACE_HASH="$(printf '%s' "$REPO" | sha256sum | cut -c1-8)"
RUN_NAME="od-dev-$WORKSPACE_HASH"
RUNDIR="${OMNIDESK_DEV_ROOT:-/tmp/omnidesk-dev}/$RUN_NAME"
BIN="$APPDIR/src-tauri/target/debug/omnidesk-desktop"
DSH_BIN="$REPO/runtime/dsh-runtime/node_modules/@deepseek-ai/dsh/lib/bin.js"
MOCK_HELPER="$REPO/tests/verification/engine-interface/adapter"
DEV_PORT=5273
PID_DIR="$RUNDIR/pids"

NO_BUILD=0
MODE=start
for arg in "$@"; do
  case "$arg" in
    --no-build) NO_BUILD=1 ;;
    --stop) MODE=stop ;;
    --status) MODE=status ;;
    --real)
      echo "拒绝执行：本启动器只支持隔离合成端点，不提供 --real。" >&2
      echo "如需其他端点，请先完成端点准入与隔离的独立评审，再另行实现。" >&2
      exit 2 ;;
    -h|--help)
      sed -n '1,20p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      echo "未知参数：$arg" >&2
      exit 2 ;;
  esac
done

# --- 共用进程/端口判定库（--stop 与 --status 也必须能用到）--------------------
PORT_LIB="$REPO/scripts/lib/port-owner.sh"
if [ ! -f "$PORT_LIB" ]; then
  echo "缺少端口归属判定脚本：$PORT_LIB" >&2
  exit 1
fi
# shellcheck source=lib/port-owner.sh
. "$PORT_LIB"

# --- 进程身份：PID + 启动时刻 + 命令行指纹 --------------------------------------
# /proc/<pid>/stat 第 22 字段是启动时刻（jiffies），可区分 PID 重用。
proc_start_time() {
  local pid="$1" stat
  stat="$(cat "/proc/$pid/stat" 2>/dev/null)" || return 1
  # comm 字段可能含空格，先跳到最后一个 ')' 之后再切分。
  local rest="${stat##*\) }"
# shellcheck disable=SC2086
set -- $rest
  # rest 从字段 3（state）开始，故原字段 22（starttime）对应 rest 的第 20 位。
  printf '%s' "${20}"
}

proc_cmdline() {
  # 先判存在：进程可能在探测期间退出，缺失的输入重定向会由 shell 直接报错。
  [ -r "/proc/$1/cmdline" ] || return 1
  tr '\0' ' ' 2>/dev/null < "/proc/$1/cmdline" | sed 's/[[:space:]]*$//'
}

# 等进程的 cmdline 稳定后再记录身份。
# 实测：exec 后极短时间内 /proc/<pid>/cmdline 可能仍为空或残留旧值，此时记下的
# 指纹会让后续 --stop 判定“命令行不符”而拒绝回收，留下占端口的孤儿进程。
proc_cmdline_settled() {
  local pid="$1" prev="" now="" i
  for i in $(seq 1 30); do
    now="$(proc_cmdline "$pid" || true)"
    if [ -n "$now" ] && [ "$now" = "$prev" ]; then
      printf '%s' "$now"
      return 0
    fi
    prev="$now"
    sleep 0.05
  done
  printf '%s' "${now}"
}

# 直接子进程列表（只读 /proc）。
proc_children() {
  local parent="$1" d pid
  for d in /proc/[0-9]*; do
    pid="${d#/proc/}"
    [ "$pid" = "$parent" ] && continue
    [ "$(proc_parent_pid "$pid" || true)" = "$parent" ] && printf '%s\n' "$pid"
  done
}

# 后序输出以 $1 为根的进程树（先子后父）。
proc_tree() {
  local pid="$1" child
  for child in $(proc_children "$pid"); do
    proc_tree "$child"
  done
  printf '%s\n' "$pid"
}

# 记录一条身份：pid、启动时刻、命令行摘要
record_pid() {
  local name="$1" pid="$2" started cmd
  started="$(proc_start_time "$pid" || true)"
  cmd="$(proc_cmdline_settled "$pid" || true)"
  [ -n "$started" ] || return 0
  if [ -z "$cmd" ]; then
    echo "无法读取进程 $pid 的命令行，拒绝写入身份记录并回收该进程：$name" >&2
    kill "$pid" 2>/dev/null || true
    return 1
  fi
  printf '%s\t%s\t%s\n' "$pid" "$started" "$cmd" >> "$PID_DIR/$name"
}

# 停止一组已记录进程：身份不符的一律不动。
stop_group() {
  local name="$1" file="$PID_DIR/$1" pid started cmd now_start skipped=0 stopped=0
  [ -f "$file" ] || return 0
  while IFS=$'\t' read -r pid started cmd; do
    [ -n "${pid:-}" ] || continue
    if ! kill -0 "$pid" 2>/dev/null; then
      continue
    fi
    now_start="$(proc_start_time "$pid" || true)"
    if [ -z "$now_start" ] || [ "$now_start" != "$started" ]; then
      echo "  跳过 $name PID $pid：进程身份不符（可能已被复用）。" >&2
      skipped=$((skipped + 1))
      continue
    fi
    # 启动时刻相同还不足以证明是同一进程：必须命令行完全一致才允许发信号。
    now_cmd="$(proc_cmdline "$pid" || true)"
    if [ -z "$cmd" ] || [ -z "$now_cmd" ] || [ "$now_cmd" != "$cmd" ]; then
      echo "  跳过 $name PID $pid：命令行与记录不符，未发送信号。" >&2
      skipped=$((skipped + 1))
      continue
    fi
    if kill "$pid" 2>/dev/null; then
      stopped=$((stopped + 1))
    else
      skipped=$((skipped + 1))
    fi
  done < "$file"
  python3 - "$file" <<'PY' 2>/dev/null || true
import sys, os
os.remove(sys.argv[1])
PY
  echo "  $name：已停止 $stopped 个，跳过 $skipped 个。"
}

# 停止一整棵已核实身份的进程树（先回收后代，再回收根进程）。
# 仅用于由 npm 派生 sh / vite 的前端进程：只杀 npm 会让 vite 残留并继续占用端口。
stop_tree() {
  local name="$1" file="$PID_DIR/$1" pid started cmd now_start now_cmd stopped=0 skipped=0
  [ -f "$file" ] || return 0
  while IFS=$'\t' read -r pid started cmd; do
    [ -n "${pid:-}" ] || continue
    if ! kill -0 "$pid" 2>/dev/null; then
      continue
    fi
    now_start="$(proc_start_time "$pid" || true)"
    now_cmd="$(proc_cmdline "$pid" || true)"
    if [ -z "$now_start" ] || [ "$now_start" != "$started" ] \
      || [ -z "$cmd" ] || [ -z "$now_cmd" ] || [ "$now_cmd" != "$cmd" ]; then
      echo "  跳过 $name PID $pid：进程身份不符，未发送信号。" >&2
      skipped=$((skipped + 1))
      continue
    fi
    # 身份已核实：此刻枚举到的后代一定属于该进程，不存在 PID 复用歧义。
    local target
    for target in $(proc_tree "$pid"); do
      kill "$target" 2>/dev/null && stopped=$((stopped + 1))
    done
  done < "$file"
  python3 - "$file" <<'PY' 2>/dev/null || true
import sys, os
os.remove(sys.argv[1])
PY
  echo "  $name：已停止 $stopped 个（含子进程），跳过 $skipped 个。"
}

stop_all() {
  if [ ! -d "$PID_DIR" ]; then
    echo "没有找到本次运行的进程记录：$PID_DIR"
    return 0
  fi
  echo "停止 $RUNDIR 下由本脚本记录的进程："
  stop_group app
  stop_group mock
  stop_tree vite
  python3 - "$PID_DIR" <<'PY' 2>/dev/null || true
import sys, os
root = sys.argv[1]
try:
    os.rmdir(root)
except OSError:
    pass
PY
}

show_status() {
  if [ ! -d "$PID_DIR" ]; then
    echo "未启动（无记录目录 $PID_DIR）。"
    return 0
  fi
  local name file pid started cmd alive
  for name in vite mock app; do
    file="$PID_DIR/$name"
    [ -f "$file" ] || continue
    while IFS=$'\t' read -r pid started cmd; do
      [ -n "${pid:-}" ] || continue
      alive="dead"
      if kill -0 "$pid" 2>/dev/null && [ "$(proc_start_time "$pid" || true)" = "$started" ]; then
        alive="alive"
      fi
      echo "  $name  pid=$pid  $alive  ${cmd:0:80}"
    done < "$file"
  done
}

if [ "$MODE" = stop ]; then
  stop_all
  exit 0
fi
if [ "$MODE" = status ]; then
  show_status
  exit 0
fi

# --- 0) 前置检查：拒绝半启动，任何缺失都先退出 --------------------------------
if [ ! -f "$DSH_BIN" ]; then
  cat >&2 <<EOF
找不到 DSH CLI 底座：$DSH_BIN

本脚本不会创建软链接或修改 runtime/。请先在本仓库运行：
  scripts/bootstrap-runtime.sh
EOF
  exit 1
fi
if [ ! -d "$APPDIR/node_modules" ]; then
  echo "缺少前端依赖：$APPDIR/node_modules。请先在该目录执行 npm ci（或 npm install）。" >&2
  exit 1
fi
if [ ! -d "$MOCK_HELPER" ]; then
  echo "缺少合成端点辅助目录：$MOCK_HELPER" >&2
  exit 1
fi
if [ -d "$PID_DIR" ] && [ -n "$(ls -A "$PID_DIR" 2>/dev/null)" ]; then
  echo "检测到未清理的启动记录：$PID_DIR" >&2
  echo "先运行 scripts/dev-ui.sh --status 查看，或用 --stop 收尾后再启动。" >&2
  exit 1
fi

# --- 0.5) 端口预检（先于构建与任何启动动作）-----------------------------------
# 固定端口 5273 是 Tauri devUrl 的硬依赖。只要该端口已被任何服务占用，本次就
# 直接失败：不复用、不打开窗口、不向占用进程发送信号。否则并行 worktree 的
# 窗口可能加载到另一个工作区的前端，这正是 OD-ARCH-006 要消除的串载风险。
if port_listening "$DEV_PORT"; then
  OWNER_PID="$(port_owner_pid "$DEV_PORT" || true)"
  OWNER_CWD="$(port_owner_cwd "${OWNER_PID:-}")"
  cat >&2 <<EOF
端口 $DEV_PORT 已被占用，本次启动已中止（未启动前端、未打开窗口、未发送任何信号）。
  占用 PID : ${OWNER_PID:-<无法确认，可能属于其他用户>}
  工作目录 : ${OWNER_CWD:-<未知>}

本次工作区：$APPDIR
即使占用者看起来属于本工作区，启动器也不会复用既有页面：它无法可靠确认
该服务是否由本次启动创建，复用可能导致窗口加载旧构建或别的分支。
请先自行确认归属并停止该服务，然后重新启动。
EOF
  exit 1
fi

mkdir -p "$RUNDIR" "$PID_DIR"
FAILED=0
cleanup_on_fail() {
  if [ "$FAILED" = 1 ]; then
    echo "启动失败，回收本次已启动的进程…" >&2
    stop_all >&2 || true
  fi
}
trap cleanup_on_fail EXIT

# --- 1) Rust/Tauri 构建链环境（本机用户级工具链）--------------------------------
if [ -f "$REPO/scripts/setup-desktop-toolchain.sh" ]; then
  eval "$(bash "$REPO/scripts/setup-desktop-toolchain.sh" --print-env)"
fi

# --- 2) 构建桌面二进制 ----------------------------------------------------------
if [ "$NO_BUILD" = 0 ] || [ ! -x "$BIN" ]; then
  echo "构建桌面二进制（首次较慢）…"
  if ! (cd "$APPDIR/src-tauri" && cargo build --offline); then
    echo "构建失败，未启动任何服务。" >&2
    exit 1
  fi
fi
[ -x "$BIN" ] || { echo "二进制不存在：$BIN" >&2; exit 1; }

# --- 3) 前端 dev server：由本次启动器独占 -------------------------------------
echo "启动前端 dev server（:$DEV_PORT）…"
(cd "$APPDIR" && exec npm run dev > "$RUNDIR/vite.log" 2>&1) &
VITE_PID=$!
record_pid vite "$VITE_PID"
READY=0
for _ in $(seq 1 60); do
  if curl -fsS -m 2 -o /dev/null "http://127.0.0.1:$DEV_PORT/" 2>/dev/null; then
    READY=1
    break
  fi
  if ! kill -0 "$VITE_PID" 2>/dev/null; then
    break
  fi
  sleep 0.5
done
if [ "$READY" != 1 ]; then
  echo "前端 dev server 未能在 30 秒内就绪，详见 $RUNDIR/vite.log" >&2
  tail -n 20 "$RUNDIR/vite.log" >&2 || true
  FAILED=1
  exit 1
fi
# 就绪后复核端口归属：监听者必须是本次 npm run dev 的后代（npm → sh → vite），
# 且工作目录是本工作区。不能用「端口 PID == npm PID」判断——vite 是孙进程。
PORT_PID="$(port_owner_pid "$DEV_PORT" || true)"
PORT_CWD="$(port_owner_cwd "${PORT_PID:-}")"
if ! port_owned_by_launcher "${PORT_PID:-}" "$VITE_PID" "$APPDIR"; then
  echo "端口 $DEV_PORT 上的服务不是本次启动器拉起的（PID ${PORT_PID:-<未知>}，目录 ${PORT_CWD:-<未知>}），中止。" >&2
  FAILED=1
  exit 1
fi

# --- 4) 数据根与合成模型端点 ----------------------------------------------------
export OMNIDESK_APP_DATA_ROOT="$RUNDIR/app-data"
export OMNIDESK_LOCAL_DATA_ROOT="$RUNDIR/local-data"
export DSH_HOME="$RUNDIR/dsh-home"
export DSH_AGENTS_HOME="$RUNDIR/agents"
# 桌面宿主读的是 OMNIDESK_DSH_HOME / OMNIDESK_AGENTS_HOME，再由宿主显式下发给
# 引擎子进程；只设 DSH_HOME 会让宿主回退到 $HOME/OmniDesk/runtime，从而连到
# 真实开发数据根。两套名字都必须指向本次运行目录。
export OMNIDESK_DSH_HOME="$RUNDIR/dsh-home"
export OMNIDESK_AGENTS_HOME="$RUNDIR/agents"
mkdir -p "$OMNIDESK_APP_DATA_ROOT" "$OMNIDESK_LOCAL_DATA_ROOT" "$DSH_HOME" "$DSH_AGENTS_HOME"
# OD-ARCH-048：默认走 web-api 通道（DSH 本地 Web API，逐字流式）；
# `OMNIDESK_ENGINE_CHANNEL=acp` 可一键切回 ACP（回退路径）。
export OMNIDESK_ENGINE_CHANNEL="${OMNIDESK_ENGINE_CHANNEL:-web-api}"
if [ "$OMNIDESK_ENGINE_CHANNEL" = "acp" ]; then
  export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-acp}"
else
  export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-web}"
fi
export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}" # 本机 WebKit 渲染规避

# 用本次启动独有的一次性文件名，避免读到上一次残留的端点地址。
MOCK_URL_FILE="$RUNDIR/mock_url.$$"
python3 - "$MOCK_URL_FILE" "$MOCK_HELPER" <<'PY' &
import sys, time
sys.path.insert(0, sys.argv[2])
from _mock_model_server import start_mock
out = sys.argv[1]
base_url, ctl, server = start_mock()
open(out, "w").write(base_url)
while True:
    time.sleep(3600)
PY
MOCK_PID=$!
record_pid mock "$MOCK_PID"
for _ in $(seq 1 50); do
  [ -s "$MOCK_URL_FILE" ] && break
  sleep 0.2
done
if [ ! -s "$MOCK_URL_FILE" ]; then
  echo "合成模型端点未能启动，详见上方输出。" >&2
  FAILED=1
  exit 1
fi
MOCK_URL="$(cat "$MOCK_URL_FILE")"
case "$MOCK_URL" in
  http://127.0.0.1:*|http://localhost:*) ;;
  *)
    echo "合成端点地址异常（非本机回环）：$MOCK_URL，中止。" >&2
    FAILED=1
    exit 1 ;;
esac

OVERLAY="$RUNDIR/synthetic-provider.yml"
# 覆盖层随通道而变：ACP 通道走 dsh-acp app 的 provider/model 配置；
# web-api 通道走 agent 的默认模型（web app 不加载 dsh-acp）。
if [ "$OMNIDESK_ENGINE_CHANNEL" = "acp" ]; then
  CHANNEL_LAYER='- id: acp
  name: "@deepseek-ai/dsh-acp"
  config:
    provider: fake
    model: synthetic-model'
else
  CHANNEL_LAYER='- id: agent-default-model
  name: "@deepseek-ai/dsh-agent-default-model"
  config:
    provider: fake
    model: synthetic-model'
fi
cat > "$OVERLAY" <<EOF
$CHANNEL_LAYER
- id: llm-pi-ai
  name: "@deepseek-ai/dsh-llm-pi-ai"
  config:
    providers:
      fake:
        displayName: Fake (synthetic)
        apiKeyEnv: OMNI_FAKE_KEY
        api: openai-completions
        baseURL: $MOCK_URL
        models:
          - id: synthetic-model
            name: synthetic-model
EOF
export OMNIDESK_DSH_PATCHES="$OVERLAY"
export OMNI_FAKE_KEY="synthetic-placeholder-not-a-credential"

# --- 5) 启动 App -----------------------------------------------------------------
echo "启动 OmniDesk…（数据根 $RUNDIR，合成端点 $MOCK_URL）"
"$BIN" >> "$RUNDIR/app.log" 2>&1 &
APP_PID=$!
record_pid app "$APP_PID"
sleep 2
if ! kill -0 "$APP_PID" 2>/dev/null; then
  echo "App 启动后立即退出，详见 $RUNDIR/app.log" >&2
  tail -n 30 "$RUNDIR/app.log" >&2 || true
  FAILED=1
  exit 1
fi

cat <<EOF

已启动（仅合成端点）：
  App PID   : $APP_PID
  App 日志   : $RUNDIR/app.log
  前端日志   : $RUNDIR/vite.log
  数据根     : $RUNDIR
  合成端点   : $MOCK_URL
查看状态：scripts/dev-ui.sh --status
停止    ：scripts/dev-ui.sh --stop

若窗口整屏发黑：确认 WEBKIT_DISABLE_DMABUF_RENDERER=1（脚本已默认设置）。
EOF
