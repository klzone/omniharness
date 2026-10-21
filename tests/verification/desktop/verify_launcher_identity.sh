#!/usr/bin/env bash
# OD-ARCH-005：dev-ui.sh 停止进程时的身份核验（PID + 启动时刻 + 命令行）。
#
# 只操作本脚本自己拉起的 sleep 子进程，不触碰任何其他实例。
# 用法：tests/verification/desktop/verify_launcher_identity.sh [证据输出文件]
set -u

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
OUT="${1:-}"
RUN="$(mktemp -d /tmp/od-launcher-identity-XXXXXX)"
# 运行目录必须与 scripts/dev-ui.sh 的命名完全一致：仓库目录名已不再用于运行根
# （其中含宿主禁入词 "omniharness"），改用前缀 + 工作区路径短哈希。
RUN_NAME="od-dev-$(printf '%s' "$REPO" | sha256sum | cut -c1-8)"
PIDDIR="$RUN/$RUN_NAME/pids"
mkdir -p "$PIDDIR"
export OMNIDESK_DEV_ROOT="$RUN"

if [ -n "$OUT" ]; then
  : > "$OUT"
  log() { printf '%s\n' "$*" | tee -a "$OUT"; }
else
log() { printf '%s\n' "$*"; }
fi

cleanup() {
  for p in "${p1:-}" "${p2:-}" "${p3:-}"; do
    [ -n "$p" ] && kill "$p" 2>/dev/null
  done
  rm -rf "$RUN" "${run:-}"
}
trap cleanup EXIT

starttime() { awk '{print $22}' "/proc/$1/stat" 2>/dev/null; }

cmdline() { tr '\0' ' ' < "/proc/$1/cmdline" 2>/dev/null | sed 's/[[:space:]]*$//'; }

fail=0
check() {
  local name="$1" expect="$2" actual="$3"
  if [ "$expect" = "$actual" ]; then
    log "PASS  $name（$actual）"
  else
    log "FAIL  $name（期望 $expect，实际 $actual）"
    fail=1
  fi
}

alive() { kill -0 "$1" 2>/dev/null && echo alive || echo dead; }

# 场景 1：启动时刻一致，但命令行与记录不符 → 必须跳过，不发信号。
sleep 300 &
p1=$!
sleep 0.2
printf '%s\t%s\t%s\n' "$p1" "$(starttime "$p1")" "sleep 300 --impostor" > "$PIDDIR/app"
out1="$(bash "$REPO/scripts/dev-ui.sh" --stop 2>&1)"
check "command-mismatch: target survives" alive "$(alive "$p1")"
check "command-mismatch: launcher reports skip" yes "$(printf '%s' "$out1" | grep -q '命令行与记录不符' && echo yes || echo no)"
log "  dev-ui --stop 输出：$(printf '%s' "$out1" | tr '\n' ' ')"
kill "$p1" 2>/dev/null
wait "$p1" 2>/dev/null

# 场景 2：完整身份一致 → 必须停止。
mkdir -p "$PIDDIR"
sleep 301 &
p2=$!
sleep 0.2
printf '%s\t%s\t%s\n' "$p2" "$(starttime "$p2")" "$(cmdline "$p2")" > "$PIDDIR/app"
out2="$(bash "$REPO/scripts/dev-ui.sh" --stop 2>&1)"

# 停止是 SIGTERM 异步生效，短暂等待后判定。
for _ in 1 2 3 4 5 6 7 8 9 10; do [ "$(alive "$p2")" = dead ] && break; sleep 0.1; done
check "identity-match: target stopped" dead "$(alive "$p2")"
log "  dev-ui --stop 输出：$(printf '%s' "$out2" | tr '\n' ' ')"

# 场景 3：启动时刻不符（PID 被复用）→ 必须跳过。
mkdir -p "$PIDDIR"
sleep 302 &
p3=$!
sleep 0.2
printf '%s\t%s\t%s\n' "$p3" "0" "$(cmdline "$p3")" > "$PIDDIR/app"
out3="$(bash "$REPO/scripts/dev-ui.sh" --stop 2>&1)"
check "starttime-mismatch: target survives" alive "$(alive "$p3")"
check "starttime-mismatch: launcher reports skip" yes "$(printf '%s' "$out3" | grep -q '进程身份不符' && echo yes || echo no)"
kill "$p3" 2>/dev/null
wait "$p3" 2>/dev/null

# 场景 4：无记录 → 不得报错。
run="$(mktemp -d /tmp/od-launcher-empty-XXXXXX)"
out4="$(OMNIDESK_DEV_ROOT="$run" bash "$REPO/scripts/dev-ui.sh" --stop 2>&1)"; code4=$?
check "no-record: exit 0" 0 "$code4"

log "launcher_identity_passed=$([ "$fail" = 0 ] && echo true || echo false)"
exit "$fail"
