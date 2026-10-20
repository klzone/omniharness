#!/usr/bin/env bash
# OmniDesk 正式环境启动器（真实端点 / 正式数据根）
#
# 与 scripts/dev-ui.sh 的分工：
#   * dev-ui.sh   —— **隔离合成环境**：一次性数据根 + 回环合成端点 + 零凭证，用于开发与自动化验收。
#                    它按 OD-ARCH-004 的硬边界**故意不提供**真实端点开关，本脚本不改变该边界。
#   * omnidesk.sh —— **正式环境**：使用正式数据根与真实模型配置，供用户本人做真实使用与人工审核。
#
# 本脚本做什么：
#   * 启动前端 dev server（5273）与桌面 App；
#   * **不设置**任何 OMNIDESK_* 隔离变量，宿主按默认解析到正式根：
#       应用数据根  $HOME/OmniDesk/app              （设置、引擎台账、使用记录 usage.jsonl）
#       成果目录    $HOME/OmniDesk/workspace/artifacts
#       DSH 数据根  $HOME/OmniDesk/runtime/dsh-home  （真实模型配置与凭证；本脚本不读取它们）
#       Profile     acp（默认）
#   * **不设置** OMNIDESK_DSH_PATCHES / OMNI_FAKE_KEY → 不注入合成 provider 覆盖。
#
# 本脚本不做什么（边界）：
#   * 不开启、不代开任何模型权限；权限开关默认关闭，须由用户在设置页自行确认端点后开启。
#   * 不改模型路由、不写凭证、不改 runtime/、不提交 Git。
#   * 不读、不打印、不复制 ~/.credentials.yaml 等任何凭证文件。
#
# 用法：
#   scripts/omnidesk.sh                 # 启动（正式环境）
#   scripts/omnidesk.sh --no-build      # 跳过 cargo build（已有二进制时更快）
#   scripts/omnidesk.sh --status        # 查看状态
#   scripts/omnidesk.sh --stop          # 停止（先核对进程身份）
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPDIR="$REPO/desktop"
BIN="$APPDIR/src-tauri/target/debug/omnidesk-desktop"
DEV_PORT=5273
RUNDIR="${OMNIDESK_LIVE_ROOT:-$HOME/.local/state/omnidesk}/live"
PID_DIR="$RUNDIR/pids"

NO_BUILD=0
MODE=start
for arg in "$@"; do
  case "$arg" in
    --no-build) NO_BUILD=1 ;;
    --stop) MODE=stop ;;
    --status) MODE=status ;;
    -h|--help)
      sed -n '1,32p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'
      exit 0 ;;
    *)
      echo "未知参数：$arg" >&2
      exit 2 ;;
  esac
done

mkdir -p "$PID_DIR"

# --- 进程身份核对：停止前必须确认命令行匹配，绝不误杀别的实例 --------------------
pid_cmdline() { tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null || true; }
pid_alive() { [ -n "${1:-}" ] && [ -d "/proc/$1" ]; }

read_pid() {
  local f="$PID_DIR/$1.pid"
  [ -f "$f" ] || return 1
  cat "$f"
}

# 校验：进程存在、命令行符合预期指纹、且与记录 PID 一致
verify_pid() {
  local name="$1" fingerprint="$2" pid
  pid="$(read_pid "$name" 2>/dev/null || true)"
  [ -n "$pid" ] || return 1
  pid_alive "$pid" || return 1
  case "$(pid_cmdline "$pid")" in
    *"$fingerprint"*) return 0 ;;
    *) return 1 ;;
  esac
}

stop_one() {
  local name="$1" fingerprint="$2" pid
  pid="$(read_pid "$name" 2>/dev/null || true)"
  if [ -z "$pid" ] || ! pid_alive "$pid"; then
    echo "  $name：未在运行"
    rm -f "$PID_DIR/$name.pid"
    return 0
  fi
  case "$(pid_cmdline "$pid")" in
    *"$fingerprint"*)
      kill -TERM "$pid" 2>/dev/null || true
      for _ in $(seq 1 40); do
        pid_alive "$pid" || break
        sleep 0.25
      done
      pid_alive "$pid" && kill -KILL "$pid" 2>/dev/null || true
      echo "  $name：已停止（pid $pid）"
      ;;
    *)
      echo "  $name：命令行与记录不符（pid $pid），不回收；请人工确认" >&2 ;;
  esac
  rm -f "$PID_DIR/$name.pid"
}

# --- status / stop --------------------------------------------------------------
if [ "$MODE" = status ]; then
  echo "正式环境实例状态（run dir: $RUNDIR）"
  if verify_pid app "omnidesk-desktop"; then
    echo "  app：运行中（pid $(read_pid app)）"
  else
    echo "  app：未运行"
  fi
  if verify_pid vite "vite"; then
    echo "  前端 dev server：运行中（pid $(read_pid vite)）"
  else
    echo "  前端 dev server：未运行"
  fi
  exit 0
fi

if [ "$MODE" = stop ]; then
  echo "停止正式环境实例…"
  stop_one app "omnidesk-desktop"
  stop_one vite "vite"
  exit 0
fi

# --- start ----------------------------------------------------------------------
if verify_pid app "omnidesk-desktop"; then
  echo "已有正式环境实例在运行；先 scripts/omnidesk.sh --stop 再启动。" >&2
  exit 1
fi

echo "== OmniDesk 正式环境 =="
echo "  应用数据根 : $HOME/OmniDesk/app              （设置 / 引擎台账 / 使用记录）"
echo "  成果目录   : $HOME/OmniDesk/workspace/artifacts"
echo "  DSH 数据根 : $HOME/OmniDesk/runtime/dsh-home  （真实模型配置与凭证，脚本不读取）"
echo "  模型权限   : 默认全部关闭；请在设置页确认端点与类别后，由你自行开启"
echo

# 1) 二进制
if [ ! -x "$BIN" ]; then
  echo "未找到桌面二进制，先构建…"
  ( cd "$REPO" && eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)" \
    && cd desktop/src-tauri && cargo build --offline )
elif [ "$NO_BUILD" -eq 0 ]; then
  echo "检查二进制是否与源码同步…"
  ( cd "$REPO" && eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)" \
    && cd desktop/src-tauri && cargo build --offline )
fi
[ -x "$BIN" ] || { echo "构建后仍找不到二进制：$BIN" >&2; exit 1; }

# 2) 前端 dev server（直接跑 vite 的 node 入口：单进程，停止时不会留下孤儿）
if ! (exec 3<>"/dev/tcp/127.0.0.1/$DEV_PORT") 2>/dev/null; then
  echo "启动前端 dev server（: $DEV_PORT）…"
  setsid bash -c 'cd "$1" && exec node node_modules/vite/bin/vite.js' _ "$APPDIR" \
    </dev/null >"$RUNDIR/vite.log" 2>&1 &
  echo $! >"$PID_DIR/vite.pid"
  READY=0
  for _ in $(seq 1 60); do
    if (exec 3<>"/dev/tcp/127.0.0.1/$DEV_PORT") 2>/dev/null; then READY=1; break; fi
    sleep 0.5
  done
  [ "$READY" = 1 ] || { echo "前端 dev server 未就绪，见 $RUNDIR/vite.log" >&2; tail -n 20 "$RUNDIR/vite.log" >&2 || true; exit 1; }
else
  echo "端口 $DEV_PORT 已被占用：若那是本项目的 dev server 可直接复用，否则请先处理。"
fi

# 3) 桌面 App：正式数据根（不设置任何隔离变量）
echo "启动 OmniDesk（正式环境）…"
setsid bash -c '
  unset OMNIDESK_APP_DATA_ROOT OMNIDESK_LOCAL_DATA_ROOT OMNIDESK_DSH_HOME \
        OMNIDESK_AGENTS_HOME OMNIDESK_DSH_PATCHES OMNI_FAKE_KEY DSH_HOME DSH_AGENTS_HOME
  export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-acp}"
  cd "$1"
  exec "$2"
' _ "$REPO" "$BIN" </dev/null >>"$RUNDIR/app.log" 2>&1 &
echo $! >"$PID_DIR/app.pid"

sleep 3
if ! pid_alive "$(cat "$PID_DIR/app.pid")"; then
  echo "App 启动后立即退出，见 $RUNDIR/app.log" >&2
  tail -n 30 "$RUNDIR/app.log" >&2 || true
  exit 1
fi

echo
echo "已启动。app pid $(cat "$PID_DIR/app.pid")，日志 $RUNDIR/app.log"
echo "查看状态：scripts/omnidesk.sh --status      停止：scripts/omnidesk.sh --stop"
