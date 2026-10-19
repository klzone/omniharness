#!/usr/bin/env bash
# 开发端口归属判定（被 scripts/dev-ui.sh source，也可被验证脚本单独 source）
#
# 只做只读检查：读取 ss 与 /proc，不发信号、不创建文件。
# 依赖：ss、/proc（Linux）。其他平台行为未验证。

# 返回监听指定端口的进程 PID（可能为空：无权限读取其他用户进程时）
port_owner_pid() {
  local port="$1"
  ss -ltnp 2>/dev/null | awk -v port=":$port" '$4 ~ port {print}' \
    | sed -n 's/.*pid=\([0-9]\+\).*/\1/p' | head -1
}

# 读取 /proc/<pid>/stat 中的父进程 PID。
# 注意：第 2 字段 comm 带括号且可能含空格（`npm run dev` 显示为 `(dev)`），
# 直接 awk $4 会取到 comm 里的碎片。必须先跳到最后一个 ')' 之后再切分：
# 之后第 1 位是 state（原字段 3），第 2 位才是 ppid（原字段 4）。
proc_parent_pid() {
  local pid="$1" stat rest
  stat="$(cat "/proc/$pid/stat" 2>/dev/null)" || return 1
  rest="${stat##*\) }"
  [ "$rest" != "$stat" ] || return 1
  # shellcheck disable=SC2086
  set -- $rest
  printf '%s' "${2:-}"
}

port_owner_cwd() {
  local pid="$1"
  [ -n "$pid" ] || return 0
  readlink -f "/proc/$pid/cwd" 2>/dev/null || true
}

port_listening() {
  local port="$1"
  ss -ltn 2>/dev/null | awk -v port=":$port" '$4 ~ port {found=1} END {exit !found}'
}

# 从 pid 向上遍历父进程（含自身），每行一个 PID，最多 24 层，遇到 1 停止。
proc_ancestors() {
  local pid="$1" depth=0 ppid
  while [ -n "$pid" ] && [ "$pid" != "0" ] && [ "$pid" != "1" ] && [ "$depth" -lt 24 ]; do
    printf '%s\n' "$pid"
    ppid="$(proc_parent_pid "$pid" || true)"
    [ -n "$ppid" ] || break
    [ "$ppid" = "$pid" ] && break
    pid="$ppid"
    depth=$((depth + 1))
  done
}

# 判定端口占用者是否由本次启动器拉起：
#   $1 端口占用 PID，$2 本次启动的父进程 PID（npm run dev），$3 期望的工作目录
# 成功返回 0 并打印一行说明；失败返回 1 并打印原因到 stderr。
port_owned_by_launcher() {
  local owner="$1" launcher="$2" expected_cwd="$3" owner_cwd chain
  if [ -z "$owner" ]; then
    echo "无法确认端口占用者 PID（可能属于其他用户或无权限读取 /proc）。" >&2
    return 1
  fi
  if [ -z "$launcher" ]; then
    echo "未记录本次启动的前端进程 PID。" >&2
    return 1
  fi
  # 逐行精确比较，不用 `grep -q`：调用方常启用 `pipefail`，而 `grep -q` 匹配后
  # 立即退出会让上游写端收到 SIGPIPE，整条管道返回 141，把“已匹配”误判成失败。
  chain="$(proc_ancestors "$owner" || true)"
  if ! printf '%s\n' "$chain" | awk -v want="$launcher" '$0 == want { found = 1 } END { exit !found }'; then
    echo "端口占用者 PID $owner 不是本次启动进程 PID $launcher 的后代。" >&2
    return 1
  fi
  owner_cwd="$(port_owner_cwd "$owner")"
  if [ "$owner_cwd" != "$expected_cwd" ]; then
    echo "端口占用者工作目录为 ${owner_cwd:-<未知>}，期望 $expected_cwd。" >&2
    return 1
  fi
  return 0
}
