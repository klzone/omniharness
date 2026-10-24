#!/usr/bin/env bash
# OmniDesk 正式环境启动器（真实端点 / 正式数据根）
#
# 与 scripts/dev-ui.sh 的分工：
#   * dev-ui.sh   —— **隔离合成环境**：一次性数据根 + 回环合成端点 + 零凭证，用于开发与自动化验收。
#                    它按 OD-ARCH-004 的硬边界**故意不提供**真实端点开关，本脚本不改变该边界。
#   * omnidesk.sh —— **正式环境**：使用正式数据根与真实模型配置，供用户本人做真实使用与人工审核。
#
# 本脚本做什么：
#   * 需要时构建前端与 release 桌面程序，然后启动桌面 App（前端资源已内嵌，不另起 dev server）；
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
# 本机渲染规避（与 dev-ui.sh 同口径）：
#   本机（Wayland + 混合显卡）下 WebKitGTK 的 DMABUF 渲染器会导致窗口整屏全黑；
#   进程仍在运行、窗口也建得出来，所以只看到“黑窗口 + 脚本已返回”。
#   见 STATUS.md「窗口全黑」条，以及 scripts/dev-ui.sh 同名变量。
#   该变量只在未显式设置时才填默认值，用户可以自行覆盖做对照。
#
# 用法：
#   scripts/omnidesk.sh                 # 启动（正式环境）
#   scripts/omnidesk.sh --no-build      # 跳过 cargo build（已有二进制时更快）
#   scripts/omnidesk.sh --status        # 查看状态
#   scripts/omnidesk.sh --stop          # 停止（先核对进程身份）
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPDIR="$REPO/desktop"
# OD-ARCH-072：正式环境用 **release** 二进制（前端资源已内嵌）。
# 此前这里指向 debug 二进制 + 另起 vite dev server：debug 既慢、又依赖 dev server 存活，
# 用户桌面快捷方式若指向旧二进制还会出现「Command not found」（能力清单即如此）。
# 开发态热更新请用 scripts/dev-ui.sh（隔离合成环境 + debug 二进制 + dev server）。
BIN="$APPDIR/src-tauri/target/release/omnidesk-desktop"
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
  # 旧版本会另起 vite dev server；这里只报告残留记录，不主动杀进程。
  if [ -f "$PID_DIR/vite.pid" ]; then
    echo "  前端 dev server：有旧记录（$PID_DIR/vite.pid），可用 --stop 一并回收"
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

# 1) 二进制（前端产物内嵌在 release 二进制里，所以必须先 npm build 再 cargo build）
needs_build() {
  [ -x "$BIN" ] || return 0
  # 任一前端产物或 Rust 源码比二进制新 → 需要重建
  local newer
  newer="$(find "$APPDIR/dist" "$APPDIR/src" "$APPDIR/src-tauri/src" \
    -newer "$BIN" -type f 2>/dev/null | head -n 1)"
  [ -n "$newer" ]
}

if [ "$NO_BUILD" -eq 0 ] && needs_build; then
  echo "构建前端与桌面二进制（release）…"
  ( cd "$APPDIR" && npm run build )
  # Tauri 的前端产物是通过 build.rs 内嵌的，cargo 本身不跟踪 dist/ 的改动。
  # 不碰 build.rs 的话，改完前端再 cargo build 会「秒过」并继续用旧界面。
  touch "$APPDIR/src-tauri/build.rs"
  ( cd "$REPO" && eval "$(bash scripts/setup-desktop-toolchain.sh --print-env)" \
    && cd desktop/src-tauri && cargo build --release --offline )
fi
[ -x "$BIN" ] || { echo "构建后仍找不到二进制：$BIN" >&2; exit 1; }

# 2) 桌面 App：正式数据根（不设置任何隔离变量）
#    release 二进制自带前端资源，不需要 dev server，也没有「端口被占」问题。
echo "启动 OmniDesk（正式环境）…"
# 日志加时间戳表头：否则失败时难以判断哪一次启动写了哪一段（旧日志会混在一起）。
{ echo; echo "===== $(date '+%Y-%m-%d %H:%M:%S') 启动（pid 见 $PID_DIR/app.pid）====="; } >>"$RUNDIR/app.log"
setsid bash -c '
  unset OMNIDESK_APP_DATA_ROOT OMNIDESK_LOCAL_DATA_ROOT OMNIDESK_DSH_HOME \
        OMNIDESK_AGENTS_HOME OMNIDESK_DSH_PATCHES OMNI_FAKE_KEY DSH_HOME DSH_AGENTS_HOME
  # 本机 WebKitGTK 渲染规避：DMABUF 渲染器在 Wayland + 混合显卡下会整屏发黑
  # （进程存活、窗口存在、内容不显示）。可用 WEBKIT_DISABLE_DMABUF_RENDERER=0 做对照。
  export WEBKIT_DISABLE_DMABUF_RENDERER="${WEBKIT_DISABLE_DMABUF_RENDERER:-1}"
  # OD-ARCH-048：默认 web-api 通道；`OMNIDESK_ENGINE_CHANNEL=acp` 一键回退。
  export OMNIDESK_ENGINE_CHANNEL="${OMNIDESK_ENGINE_CHANNEL:-web-api}"
  if [ "$OMNIDESK_ENGINE_CHANNEL" = "acp" ]; then
    export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-acp}"
  else
    export OMNIDESK_DSH_PROFILE="${OMNIDESK_DSH_PROFILE:-web}"
  fi
  cd "$1"
  exec "$2"
' _ "$REPO" "$BIN" </dev/null >>"$RUNDIR/app.log" 2>&1 &
echo $! >"$PID_DIR/app.pid"

sleep 3
if ! pid_alive "$(cat "$PID_DIR/app.pid")"; then
  echo "App 启动后立即退出（未进入界面），见 $RUNDIR/app.log" >&2
  tail -n 30 "$RUNDIR/app.log" >&2 || true
  exit 1
fi

echo
echo "已启动。app pid $(cat "$PID_DIR/app.pid")，日志 $RUNDIR/app.log"
echo "注意：本脚本把 App 放到后台后就会返回，终端关闭不影响 App 运行。"
echo "如果窗口仍是整屏全黑：先 scripts/omnidesk.sh --stop，再用"
echo "  GDK_BACKEND=x11 bash scripts/omnidesk.sh --no-build"
echo "改用 XWayland 渲染（默认已开启本机渲染规避，正常不需要这一步）。"
echo "查看状态：scripts/omnidesk.sh --status      停止：scripts/omnidesk.sh --stop"
