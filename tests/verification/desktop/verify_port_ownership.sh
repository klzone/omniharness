#!/usr/bin/env bash
# OD-ARCH-006：开发端口归属与前端进程树回收的定向验证。
#
# 只使用独立网络命名空间与一次性临时目录；不打开 Tauri 窗口，不接触用户其他进程。
# 用法：bash tests/verification/desktop/verify_port_ownership.sh <证据输出文件>
set -uo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
EVIDENCE="${1:-/dev/stdout}"
. "$REPO/scripts/lib/port-owner.sh"

# 运行目录名必须与 scripts/dev-ui.sh 完全一致（同样的哈希与前缀）。
# 早期版本在这里写死了仓库目录名，启动器改名后 C/D 段就找不到 PID 记录，
# 会把「没找到记录」误当成「停止失败」。
RUN_NAME="od-dev-$(printf '%s' "$REPO" | sha256sum | cut -c1-8)"
export REPO RUN_NAME

FAILED=0
say() { printf '%s\n' "$*"; }
check() {
  local name="$1"; shift
  if "$@" >/dev/null 2>&1; then say "PASS  $name"; else say "FAIL  $name"; FAILED=1; fi
}

say "# OD-ARCH-006 端口归属与进程树回收验证"
say "# 日期: $(date -Is)"
say "# 仓库: $REPO"

say ""
say "## A. 归属判定（独立网络命名空间，受控进程）"
unshare -rn bash -c '
  ip link set lo up
  . "'"$REPO"'/scripts/lib/port-owner.sh"
  say() { printf "%s\n" "$*"; }
  fail=0
  sh -c "cd \"\$REPO/desktop\" && exec python3 -m http.server 5273 --bind 127.0.0.1 >/dev/null 2>&1" &
  launcher=$!
  sh -c "sleep 8 & echo \$!" > /tmp/od-sibling.$$ 2>/dev/null
  sleep 0.5
  sibling=$(cat /tmp/od-sibling.$$ 2>/dev/null || true)
  for _ in $(seq 1 40); do port_listening 5273 && break; sleep 0.1; done
  owner=$(port_owner_pid 5273)
  say "launcher=$launcher owner=$owner unrelated=$sibling"
  if port_owned_by_launcher "$owner" "$launcher" "$REPO/desktop"; then say "PASS  后代进程 + 正确工作目录：放行"; else say "FAIL  应放行"; fail=1; fi
  if port_owned_by_launcher "$owner" "$sibling" "$REPO/desktop" 2>/dev/null; then say "FAIL  无关进程被放行"; fail=1; else say "PASS  无关进程：拒绝"; fi
  if port_owned_by_launcher "$owner" "" "$REPO/desktop" 2>/dev/null; then say "FAIL  空父进程被放行"; fail=1; else say "PASS  空父进程：拒绝"; fi
  if port_owned_by_launcher "$owner" "$launcher" /tmp 2>/dev/null; then say "FAIL  错误工作目录被放行"; fail=1; else say "PASS  错误工作目录：拒绝"; fi
  kill $launcher 2>/dev/null || true
  kill "$sibling" 2>/dev/null || true
  rm -f "/tmp/od-sibling.$$"
  [ "$fail" = 0 ]
' 2>&1 | sed 's/^/  /'
[ "${PIPESTATUS[0]}" = 0 ] || FAILED=1

say ""
say "## B. 真实 npm 链：仅杀 npm 会留下 vite（缺陷复现）"
unshare -rn bash -c '
  ip link set lo up
  . "'"$REPO"'/scripts/lib/port-owner.sh"
  # 用 setsid 让 npm/sh/vite 独立成组：清理时只回收本组，绝不沿父链向上发信号。
  setsid bash -c "cd \"\$REPO/desktop\" && exec npm run dev >/tmp/od-npm-repro.log 2>&1" &
  p=$!
  sleep 0.3
  pgid=$(ps -o pgid= -p "$p" 2>/dev/null | tr -d " ")
  own_pgid=$(ps -o pgid= -p $$ 2>/dev/null | tr -d " ")
  if [ -z "$pgid" ] || [ "$pgid" = "$own_pgid" ] || [ "$pgid" = "1" ]; then
    echo "  ABORT  无法取得安全的独立进程组（pgid=${pgid:-?} own=${own_pgid:-?}），不发送任何信号"
    exit 3
  fi
  for _ in $(seq 1 80); do port_listening 5273 && break; sleep 0.5; done
  echo "  listening=$(port_listening 5273 && echo yes || echo no) npm=$p pgid=$pgid owner=$(port_owner_pid 5273)"
  kill -TERM "$p" 2>/dev/null || true
  sleep 3
  if port_listening 5273; then echo "  REPRO  只杀 npm 后端口仍被占（owner=$(port_owner_pid 5273)）"; res=0; else echo "  NOTE   该环境下 npm 已自行回收子树，无法复现"; res=2; fi
  # 清理：只对本测试创建的进程组发信号（负 PID），不触碰测试 shell 或宿主进程。
  kill -TERM -- "-$pgid" 2>/dev/null || true
  for _ in $(seq 1 20); do port_listening 5273 || break; sleep 0.2; done
  if port_listening 5273; then
    echo "  CLEANUP  组内仍有监听，升级为 KILL（pgid=$pgid）"
    kill -KILL -- "-$pgid" 2>/dev/null || true
    sleep 1
  fi
  if port_listening 5273; then echo "  CLEANUP_FAIL  端口仍被占用（owner=$(port_owner_pid 5273)）"; exit 4; fi
  exit $res
' > /tmp/od-section-b.txt 2>&1
repro=$?
sed 's/^/  /' /tmp/od-section-b.txt
[ "$repro" = 0 ] && say "  结果：缺陷可复现（说明整树回收必要）" || say "  结果：本环境未能复现，仍保留整树回收"

say ""
say "## C. 启动器 --stop 回收整棵前端进程树"
unshare -rn bash -c '
  ip link set lo up
  . "'"$REPO"'/scripts/lib/port-owner.sh"
  tmp=$(mktemp -d /tmp/od-stop-tree-XXXXXX)
  piddir="$tmp/'"$RUN_NAME"'/pids"; mkdir -p "$piddir"
  ( cd "$REPO/desktop" && exec npm run dev >/tmp/od-stop-tree.log 2>&1 ) &
  p=$!
  for _ in $(seq 1 80); do port_listening 5273 && break; sleep 0.5; done
  stat=$(cat /proc/$p/stat); rest=${stat##*) }; set -- $rest; started=${20}
  cmd=$(tr "\0" " " < /proc/$p/cmdline | sed "s/[[:space:]]*$//")
  printf "%s\t%s\t%s\n" "$p" "$started" "$cmd" > "$piddir/vite"
  echo "  录制 npm PID=$p，端口占用者=$(port_owner_pid 5273)"
  OMNIDESK_DEV_ROOT="$tmp" bash scripts/dev-ui.sh --stop 2>&1 | sed "s/^/  /"
  sleep 1
  if port_listening 5273; then echo "  FAIL  停止后端口仍被占用（owner=$(port_owner_pid 5273)）"; exit 1; fi
  left=$(ps -eo cmd | grep -c "[n]ode_modules/.bin/vite")
  echo "  vite 残留数=$left"
  [ "$left" = 0 ]
' 2>&1
[ $? = 0 ] && say "PASS  端口释放且无 vite 残留" || { say "FAIL  停止后仍有残留或端口被占"; FAILED=1; }

say ""
say "## D. 身份不符时不连坐杀子进程"
unshare -rn bash -c '
  ip link set lo up
  tmp=$(mktemp -d /tmp/od-identity-tree-XXXXXX)
  piddir="$tmp/'"$RUN_NAME"'/pids"; mkdir -p "$piddir"
  sh -c "sleep 30 & echo \$! > /tmp/od-child.$$; wait" &
  parent=$!
  sleep 1; child=$(cat /tmp/od-child.$$ 2>/dev/null || true)
  stat=$(cat /proc/$parent/stat); rest=${stat##*) }; set -- $rest; started=${20}
  printf "%s\t%s\t%s\n" "$parent" "$started" "/bin/false 伪造的命令行" > "$piddir/vite"
  out=$(OMNIDESK_DEV_ROOT="$tmp" bash scripts/dev-ui.sh --stop 2>&1)
  echo "$out" | sed "s/^/  /"
  alive_parent=$(kill -0 "$parent" 2>/dev/null && echo yes || echo no)
  alive_child=$(kill -0 "$child" 2>/dev/null && echo yes || echo no)
  echo "  parent_alive=$alive_parent child_alive=$alive_child"
  kill "$parent" "$child" 2>/dev/null || true
  rm -f "/tmp/od-child.$$"
  [ "$alive_parent" = yes ] && [ "$alive_child" = yes ]
' 2>&1
[ $? = 0 ] && say "PASS  身份不符时父子进程均未被触碰" || { say "FAIL  身份不符仍被发信号"; FAILED=1; }

say ""
say "port_ownership_passed=$([ "$FAILED" = 0 ] && echo true || echo false)"

if [ "$EVIDENCE" != "/dev/stdout" ]; then
  exit "$FAILED"
fi
exit "$FAILED"
