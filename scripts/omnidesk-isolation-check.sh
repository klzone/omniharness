#!/usr/bin/env bash
# ==============================================================================
# OmniDesk 隔离边界自动化测试（静态 + 启动器自检）
#
# 目标：在不启动 DSH、不触碰原版数据的前提下，自动验证隔离护栏是否真实生效。
#      这是回归测试脚本：每次改动 omnidesk-dev.sh / sync-skills.sh 后都应运行。
#
# 覆盖：
#   1. 两个脚本通过 bash 语法检查；
#   2. 启动器 --check 在默认配置下通过；
#   3. 隔离根落在 ~/.dsh / ~/.agents 时被拒绝；
#   4. 两个隔离根相同时被拒绝；
#   5. 非法/保留 Profile 名被拒绝（../evil、desktop、node_modules、空值）；
#   6. 可覆盖隔离边界的透传参数被拒绝（--host/--profile/--patch/--dump-config 等）；
#   7. --dry-run 输出的命令行确实包含隔离根、回环地址与 yvan-work Profile；
#   8. sync-skills.sh 拒绝写入 ~/.dsh/skills 与 ~/.agents/skills；
#   9. sync-skills.sh 的 dry-run 不产生任何写入；
#  10. 脚本自身绝不向原版数据根写入（执行前后对原环境做哈希比对）。
#
# 用法:
#   scripts/omnidesk-isolation-check.sh
#   scripts/omnidesk-isolation-check.sh --keep-snapshots   # 保留快照目录
#
# 退出码：0 = 全部通过；1 = 存在失败项。
# ==============================================================================
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

KEEP_SNAPSHOTS=0
while [ $# -gt 0 ]; do
  case "$1" in
    --keep-snapshots) KEEP_SNAPSHOTS=1; shift ;;
    --help|-h)
      sed -n '2,24p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
      exit 0
      ;;
    *) printf '未知参数：%s\n' "$1" >&2; exit 2 ;;
  esac
done

PASS=0
FAIL=0

ok()   { PASS=$((PASS + 1)); printf '  [PASS] %s\n' "$*"; }
bad()  { FAIL=$((FAIL + 1)); printf '  [FAIL] %s\n' "$*"; }
head1() { printf '\n== %s ==\n' "$*"; }

# 断言：命令必须以非 0 退出，且输出命中给定关键字
expect_reject() {
  local label="$1"; shift
  local needle="$1"; shift
  local out rc
  out="$("$@" 2>&1)"; rc=$?
  if [ "$rc" -eq 0 ]; then
    bad "$label —— 预期被拒绝，实际退出码 0"
  elif ! printf '%s' "$out" | grep -qF -- "$needle"; then
    bad "$label —— 被拒绝但输出未包含关键字「$needle」"
    printf '%s\n' "$out" | tail -3 | sed 's/^/         /'
  else
    ok "$label"
  fi
}

# 断言：命令必须成功退出
expect_pass() {
  local label="$1"; shift
  local out rc
  out="$("$@" 2>&1)"; rc=$?
  if [ "$rc" -eq 0 ]; then
    ok "$label"
  else
    bad "$label —— 预期成功，实际退出码 $rc"
    printf '%s\n' "$out" | tail -5 | sed 's/^/         /'
  fi
}

DEV="$PROJECT_ROOT/scripts/omnidesk-dev.sh"
SYNC="$PROJECT_ROOT/scripts/sync-skills.sh"

head1 "0. 前置检查"
[ -f "$DEV" ]  && ok "存在 scripts/omnidesk-dev.sh"  || bad "缺少 scripts/omnidesk-dev.sh"
[ -f "$SYNC" ] && ok "存在 scripts/sync-skills.sh"   || bad "缺少 scripts/sync-skills.sh"
if [ "$FAIL" -gt 0 ]; then
  printf '\n前置检查未通过，终止。\n'
  exit 1
fi

# --------------------------------------------------------------------------
# 1. 语法检查
# --------------------------------------------------------------------------
head1 "1. Bash 语法检查"
expect_pass "omnidesk-dev.sh 语法正确" bash -n "$DEV"
expect_pass "sync-skills.sh 语法正确"  bash -n "$SYNC"

# --------------------------------------------------------------------------
# 2. 原环境基线快照（用于最终比对）
# --------------------------------------------------------------------------
head1 "2. 记录原环境基线"
SNAP_DIR="$(mktemp -d "/tmp/omnidesk-isolation-check-XXXXXX")"
STOCK_DSH="$HOME/.dsh"
STOCK_AGENTS="$HOME/.agents"

snapshot_tree() {
  local root="$1" out="$2"
  : > "$out"
  [ -e "$root" ] || return 0
  ( cd "$root" 2>/dev/null && find . -type f -print0 2>/dev/null \
      | sort -z | xargs -0 sha256sum 2>/dev/null ) >> "$out" || true
}

for pair in "$STOCK_DSH:dsh" "$STOCK_AGENTS:agents"; do
  root="${pair%%:*}"; name="${pair##*:}"
  snapshot_tree "$root" "$SNAP_DIR/$name.before.txt"
done
ok "已记录基线：$SNAP_DIR"

# --------------------------------------------------------------------------
# 3. 启动器默认自检
# --------------------------------------------------------------------------
head1 "3. 启动器默认隔离边界"
expect_pass "omnidesk-dev.sh --check 通过" bash "$DEV" --check

dry_out="$(bash "$DEV" --dry-run 2>&1)"
# 只对“即将执行的命令行”做断言；摘要里会显式打印原版数据根作为“本次不访问”的提示。
dry_cmd="$(printf '%s\n' "$dry_out" | grep -F 'env ELECTRON_RUN_AS_NODE' | tail -1)"
if printf '%s' "$dry_out" | grep -qF "DSH_HOME=$HOME/OmniDesk/runtime/dsh-home"; then
  ok "--dry-run 使用独立 DSH_HOME"
else
  bad "--dry-run 未使用独立 DSH_HOME"
fi
if printf '%s' "$dry_out" | grep -qF "DSH_AGENTS_HOME=$HOME/OmniDesk/runtime/agents-home"; then
  ok "--dry-run 使用独立 DSH_AGENTS_HOME"
else
  bad "--dry-run 未使用独立 DSH_AGENTS_HOME"
fi
if printf '%s' "$dry_out" | grep -q -- '--profile yvan-work'; then
  ok "--dry-run 固定 yvan-work Profile"
else
  bad "--dry-run 未固定 yvan-work Profile"
fi
if printf '%s' "$dry_out" | grep -q -- '--host 127.0.0.1'; then
  ok "--dry-run 只绑定回环地址"
else
  bad "--dry-run 未绑定 127.0.0.1"
fi
if printf '%s' "$dry_out" | grep -q -- '--port 0'; then
  ok "--dry-run 请求系统动态端口"
else
  bad "--dry-run 未使用动态端口"
fi
if [ -z "$dry_cmd" ]; then
  bad "--dry-run 未打印将要执行的命令行"
elif printf '%s' "$dry_cmd" | grep -qE '(^|[ =])/home/[^ ]*/\.dsh([ /]|$)'; then
  bad "--dry-run 命令行中出现原版 ~/.dsh 路径"
elif printf '%s' "$dry_cmd" | grep -qE '(^|[ =])/home/[^ ]*/\.agents([ /]|$)'; then
  bad "--dry-run 命令行中出现原版 ~/.agents 路径"
else
  ok "--dry-run 命令行只引用隔离根，不含原版数据路径"
fi

# --------------------------------------------------------------------------
# 4. 危险隔离根必须被拒绝
# --------------------------------------------------------------------------
head1 "4. 危险隔离根拒绝"
expect_reject "拒绝 OMNIDESK_ROOT=~\.dsh"  "拒绝启动" \
  env OMNIDESK_ROOT="$HOME/.dsh" bash "$DEV" --check
expect_reject "拒绝 OMNIDESK_ROOT=~\.agents" "拒绝启动" \
  env OMNIDESK_ROOT="$HOME/.agents" bash "$DEV" --check
expect_reject "拒绝 OMNIDESK_ROOT 位于 ~/.dsh 之内" "拒绝启动" \
  env OMNIDESK_ROOT="$HOME/.dsh/sub" bash "$DEV" --check
expect_reject "拒绝 OMNIDESK_ROOT 位于 ~/.agents 之内" "拒绝启动" \
  env OMNIDESK_ROOT="$HOME/.agents/sub" bash "$DEV" --check

# --------------------------------------------------------------------------
# 5. Profile 名合法性
# --------------------------------------------------------------------------
head1 "5. Profile 名校验"
expect_reject "拒绝 Profile=../evil"        "非法或被保留" \
  env OMNIDESK_PROFILE='../evil' bash "$DEV" --check
expect_reject "拒绝 Profile=a/b"            "非法或被保留" \
  env OMNIDESK_PROFILE='a/b' bash "$DEV" --check
expect_reject "拒绝 Profile=desktop（保留名）" "非法或被保留" \
  env OMNIDESK_PROFILE='desktop' bash "$DEV" --check
expect_reject "拒绝 Profile=node_modules"   "非法或被保留" \
  env OMNIDESK_PROFILE='node_modules' bash "$DEV" --check
expect_reject "拒绝 Profile=.."             "非法或被保留" \
  env OMNIDESK_PROFILE='..' bash "$DEV" --check

# --------------------------------------------------------------------------
# 6. 越权透传参数必须被拒绝
# --------------------------------------------------------------------------
head1 "6. 透传参数白名单"
for arg in "--host=0.0.0.0" "--profile desktop" "--patch /tmp/x.yml" \
           "--from-default-profile desktop" "--public-url http://evil" \
           "--trusted-host evil.com" "--dump-config"; do
  # shellcheck disable=SC2086
  expect_reject "拒绝透传 $arg" "不允许透传" bash "$DEV" $arg
done

head1 "7. 允许的透传参数"
expect_pass "允许 --port 3099" bash "$DEV" --dry-run --port 3099
expect_pass "允许 --no-open"   bash "$DEV" --dry-run --no-open
web_out="$(bash "$DEV" --dry-run web --port 3088 2>&1)"
if printf '%s' "$web_out" | grep -q -- '--port 3088' \
   && ! printf '%s' "$web_out" | grep -qE -- '--port 3088 web'; then
  ok "web 别名被正确归一化"
else
  bad "web 别名归一化异常"
fi

# --------------------------------------------------------------------------
# 8. sync-skills 目标校验
# --------------------------------------------------------------------------
head1 "8. Skill 同步目标校验"
expect_reject "拒绝目标落在 ~/.dsh" "原版 DSH 数据根" \
  env OMNIDESK_ROOT="$HOME/.dsh/rogue" bash "$SYNC" --dry-run
expect_reject "拒绝目标落在 ~/.agents" "原版 Agent 数据根" \
  env OMNIDESK_ROOT="$HOME/.agents/rogue" bash "$SYNC" --dry-run

head1 "9. Skill 同步 dry-run 无写入"
skills_before="$(find "$HOME/OmniDesk/runtime/dsh-home/skills" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort)"
expect_pass "sync-skills.sh --dry-run 成功" bash "$SYNC" --dry-run
skills_after="$(find "$HOME/OmniDesk/runtime/dsh-home/skills" -mindepth 1 -maxdepth 1 -type d 2>/dev/null | sort)"
if [ "$skills_before" = "$skills_after" ]; then
  ok "dry-run 未改变隔离根 skills 目录结构"
else
  bad "dry-run 意外改变了隔离根 skills 目录结构"
fi

# --------------------------------------------------------------------------
# 10. 原环境零变化复核
# --------------------------------------------------------------------------
head1 "10. 原环境零变化复核"
for pair in "$STOCK_DSH:dsh" "$STOCK_AGENTS:agents"; do
  root="${pair%%:*}"; name="${pair##*:}"
  snapshot_tree "$root" "$SNAP_DIR/$name.after.txt"
  if diff -q "$SNAP_DIR/$name.before.txt" "$SNAP_DIR/$name.after.txt" >/dev/null 2>&1; then
    ok "原环境 $root 在本轮测试中零变化"
  else
    bad "原环境 $root 发生变化"
    diff "$SNAP_DIR/$name.before.txt" "$SNAP_DIR/$name.after.txt" | head -10 | sed 's/^/         /'
  fi
done

if [ "$KEEP_SNAPSHOTS" -eq 1 ]; then
  printf '\n快照保留于：%s\n' "$SNAP_DIR"
else
  rm -f "$SNAP_DIR"/*.txt 2>/dev/null || true
  rmdir "$SNAP_DIR" 2>/dev/null || true
fi

# --------------------------------------------------------------------------
# 汇总
# --------------------------------------------------------------------------
printf '\n==============================================================\n'
printf ' OmniDesk 隔离自检结果：通过 %d 项，失败 %d 项\n' "$PASS" "$FAIL"
printf '==============================================================\n'
[ "$FAIL" -eq 0 ] || exit 1
exit 0
