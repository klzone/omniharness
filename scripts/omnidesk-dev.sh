#!/usr/bin/env bash
# ==============================================================================
# OmniDesk 独立 DSH 测试环境启动器 (Phase 1)
#
# 目的：在不影响日常使用的原版 DSH（~/.dsh、~/.agents、原 Desktop）的前提下，
#       以独立数据根启动一套 DSH CLI/Web 实例，用于 OmniDesk 的开发与测试。
#
# 设计约束（对应 docs/OmniDesk-DSH-Isolation-Audit.md 与用户 Plan）：
#   1. 不修改 runtime/deepseek-harness，不修改系统安装包、原 DSH shim、原 Desktop 配置。
#   2. 不复制、不迁移、不清理、不删除原 DSH 数据。
#   3. 绕过会把 DSH_HOME 强制重设为 ~/.dsh 的用户 shim（~/.local/bin/dsh）。
#   4. 显式设置 DSH_HOME / DSH_AGENTS_HOME，并在启动前做硬性校验；
#      任一校验失败立即退出，绝不带着可疑路径启动。
#   5. 只监听 127.0.0.1，默认使用系统分配端口（--port 0），默认不自动打开浏览器。
#   6. 不自动启动 Desktop、不自动安装插件、不读取原环境凭证。
#
# 用法:
#   scripts/omnidesk-dev.sh                 # 启动隔离 Web UI（推荐，动态端口）
#   scripts/omnidesk-dev.sh --check         # 只做隔离校验，不启动任何进程
#   scripts/omnidesk-dev.sh --dry-run       # 打印将要执行的完整命令
#   scripts/omnidesk-dev.sh web             # 同上（web 为显式写法）
#   scripts/omnidesk-dev.sh web --port 3090 # 指定端口启动 Web UI
#   scripts/omnidesk-dev.sh --profile yvan-work --no-open
#
# 说明：DSH 的 `dsh <name>` 等价于 `dsh --profile <name>`，`web` 本身就是一个
#       Profile 名而不是子命令。yvan-work Profile 已经挂载了 dsh-web-app bundle，
#       因此 Web 相关旗标直接跟在 `--profile yvan-work` 之后。
# ==============================================================================
set -euo pipefail

# --------------------------------------------------------------------------
# 0. 常量
# --------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"

OMNIDESK_ROOT="${OMNIDESK_ROOT:-$HOME/OmniDesk}"
RUNTIME_ROOT="$OMNIDESK_ROOT/runtime"
OMNIDESK_DSH_HOME="$RUNTIME_ROOT/dsh-home"
OMNIDESK_AGENTS_HOME="$RUNTIME_ROOT/agents-home"
OMNIDESK_DESKTOP_USER_DATA="$RUNTIME_ROOT/desktop-user-data"

STOCK_DSH_HOME="$HOME/.dsh"
STOCK_AGENTS_HOME="$HOME/.agents"

OMNIDESK_PROFILE="${OMNIDESK_PROFILE:-yvan-work}"

OMNIDESK_ELECTRON_BIN='/opt/DeepSeek Harness/deepseek-harness'
OMNIDESK_ASAR_FILE='/opt/DeepSeek Harness/resources/app.asar'
# asar 归档内的路径无法用 bash 的 -f 检测，只能交给 Electron 的 asar 感知 fs 读取。
OMNIDESK_CLI_ENTRY_1='/opt/DeepSeek Harness/resources/app.asar/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js'
OMNIDESK_CLI_ENTRY_2="$HOME/dsh-asar/dsh/node_modules/@deepseek-ai/dsh/lib/bin.js"
# 隔离实例允许的入口白名单（防止被环境变量或参数改写为任意脚本）。

# --------------------------------------------------------------------------
# 1. 辅助函数
# --------------------------------------------------------------------------
info() { printf '[OmniDesk] %s\n' "$*"; }
die()  { printf '[OmniDesk][错误] %s\n' "$*" >&2; exit 1; }

abspath() {
  if command -v realpath >/dev/null 2>&1; then
    realpath -m -- "$1"
  else
    printf '%s\n' "$1"
  fi
}

usage() {
  sed -n '2,24p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
}

# --------------------------------------------------------------------------
# 2. 参数解析：分离“启动器自身旗标”与“透传给 DSH 的参数”
# --------------------------------------------------------------------------
CHECK_ONLY=0
DRY_RUN=0
PASSTHROUGH=()

while [ $# -gt 0 ]; do
  case "$1" in
    --check)
      CHECK_ONLY=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    --)
      shift
      while [ $# -gt 0 ]; do PASSTHROUGH+=("$1"); shift; done
      ;;
    --patch|--patch=*|--from-default-profile|--from-default-profile=*)
      die "不允许透传 $1：该参数可覆盖 Profile 补丁层或来源，会破坏隔离边界。"
      ;;
    --profile|--profile=*)
      die "不允许透传 $1：本启动器固定使用隔离 Profile '$OMNIDESK_PROFILE'，请勿覆盖。"
      ;;
    --host|--host=*)
      die "不允许透传 $1：隔离实例固定绑定 127.0.0.1，避免暴露到网络。"
      ;;
    --public-url|--public-url=*|--trusted-host|--trusted-host=*)
      die "不允许透传 $1：该参数用于对外暴露与信任配置，不属于本地隔离测试范围。"
      ;;
    *--dump-config*|*--dump-default-config*|*--dump-config-schema*)
      die "不允许透传 $1：配置导出会读取 Profile 组合树，请改用 DSH 入口直接执行。"
      ;;
    *)
      PASSTHROUGH+=("$1")
      shift
      ;;
  esac
done

# --------------------------------------------------------------------------
# 3. 硬性校验
# --------------------------------------------------------------------------
if [ -z "${OMNIDESK_DSH_HOME}" ]; then
  die "DSH 隔离根为空，拒绝启动。"
fi
if [ -z "${OMNIDESK_AGENTS_HOME}" ]; then
  die "Agent 隔离根为空，拒绝启动。"
fi

OMNIDESK_ROOT_ABS="$(abspath "$OMNIDESK_ROOT")"
DSH_HOME_ABS="$(abspath "${OMNIDESK_DSH_HOME}")"
AGENTS_HOME_ABS="$(abspath "${OMNIDESK_AGENTS_HOME}")"
STOCK_DSH_ABS="$(abspath "$STOCK_DSH_HOME")"
STOCK_AGENTS_ABS="$(abspath "$STOCK_AGENTS_HOME")"

# 3.1 两个隔离根必须位于 OmniDesk 根之下
case "$DSH_HOME_ABS/" in
  "$OMNIDESK_ROOT_ABS"/*) ;;
  *) die "DSH 隔离根 $DSH_HOME_ABS 不在 OmniDesk 根 $OMNIDESK_ROOT_ABS 之下，拒绝启动。" ;;
esac
case "$AGENTS_HOME_ABS/" in
  "$OMNIDESK_ROOT_ABS"/*) ;;
  *) die "Agent 隔离根 $AGENTS_HOME_ABS 不在 OmniDesk 根 $OMNIDESK_ROOT_ABS 之下，拒绝启动。" ;;
esac

# 3.2 解析结果绝不能落入原版数据根
case "$DSH_HOME_ABS/" in
  "$STOCK_DSH_ABS"/*) die "DSH 隔离根落入了原版数据根 $STOCK_DSH_ABS，拒绝启动。" ;;
esac
case "$AGENTS_HOME_ABS/" in
  "$STOCK_AGENTS_ABS"/*) die "Agent 隔离根落入了原版数据根 $STOCK_AGENTS_ABS，拒绝启动。" ;;
esac
if [ "$DSH_HOME_ABS" = "$AGENTS_HOME_ABS" ]; then
  die "DSH 隔离根与 Agent 隔离根不能相同，拒绝启动。"
fi

# 3.3 Profile 名合法性
case "$OMNIDESK_PROFILE" in
  ''|*/*|*\\*|.|..|node_modules|desktop)
    die "Profile 名 '$OMNIDESK_PROFILE' 非法或被保留，拒绝启动。"
    ;;
esac

# 3.4 隔离根初始化（仅从仓库模板复制，不复用原版 Profile）
mkdir -p "$DSH_HOME_ABS" "$AGENTS_HOME_ABS"
PROFILE_DIR="$DSH_HOME_ABS/profiles/$OMNIDESK_PROFILE"
TEMPLATE_DIR="$PROJECT_ROOT/profiles/$OMNIDESK_PROFILE"

if [ ! -d "$PROFILE_DIR" ] || [ ! -f "$PROFILE_DIR/package.json" ]; then
  if [ ! -f "$TEMPLATE_DIR/package.json" ] || [ ! -f "$TEMPLATE_DIR/cordis.patch.yml" ]; then
    die "Profile 未初始化，且仓库模板不完整：$TEMPLATE_DIR（需含 package.json 与 cordis.patch.yml）"
  fi
  mkdir -p "$PROFILE_DIR"
  cp "$TEMPLATE_DIR/package.json" "$PROFILE_DIR/package.json"
  cp "$TEMPLATE_DIR/cordis.patch.yml" "$PROFILE_DIR/cordis.patch.yml"
  info "已从仓库模板初始化隔离 Profile：$PROFILE_DIR"
fi

# 3.5 Profile 实际路径必须落在隔离根内，且不是软链接到原版数据
PROFILE_REAL="$(realpath -m -- "$PROFILE_DIR")"
case "$PROFILE_REAL/" in
  "$DSH_HOME_ABS"/*) ;;
  *) die "Profile 实际路径 $PROFILE_REAL 不在隔离根之下（可能是软链接），拒绝启动。" ;;
esac
case "$PROFILE_REAL/" in
  "$STOCK_DSH_ABS"/*) die "Profile 实际路径落入了原版 DSH 数据根，拒绝启动。" ;;
esac

# 3.6 选择 CLI 入口
CLI_KIND=''
CLI_ENTRY=''
CLI_NODE=''
CLI_PREFIX=()

# 入口 1：Desktop 自带的打包 CLI 本体。bin.js 位于 asar 归档内，因此必须用真实
# 执行来确认（--version 返回成功即视为可用），而不是用文件存在性判断。
if [ -x "$OMNIDESK_ELECTRON_BIN" ] && [ -f "$OMNIDESK_ASAR_FILE" ]; then
  set +e
  asar_version="$(ELECTRON_RUN_AS_NODE=1 "$OMNIDESK_ELECTRON_BIN" --expose-internals \
    "$OMNIDESK_CLI_ENTRY_1" --version 2>/dev/null)"
  asar_rc=$?
  set -e
  if [ "$asar_rc" -eq 0 ] && [ -n "$asar_version" ]; then
    CLI_KIND='asar'
    CLI_ENTRY="$OMNIDESK_CLI_ENTRY_1"
    CLI_NODE="$OMNIDESK_ELECTRON_BIN"
    CLI_PREFIX=(--expose-internals)
  fi
fi

# 入口 2：从 asar 提取出的 CLI 树，需要系统 node >= 24（与官方 engines 一致）。
if [ -z "$CLI_KIND" ] && [ -f "$OMNIDESK_CLI_ENTRY_2" ] && command -v node >/dev/null 2>&1; then
  node_major="$(node -p 'process.versions.node.split(".")[0]' 2>/dev/null || echo 0)"
  if [ "${node_major:-0}" -ge 24 ] 2>/dev/null; then
    set +e
    ext_version="$(node "$OMNIDESK_CLI_ENTRY_2" --version 2>/dev/null)"
    ext_rc=$?
    set -e
    if [ "$ext_rc" -eq 0 ] && [ -n "$ext_version" ]; then
      CLI_KIND='extracted'
      CLI_ENTRY="$OMNIDESK_CLI_ENTRY_2"
      CLI_NODE='node'
    fi
  fi
fi

DSH_CLI_VERSION="${asar_version:-${ext_version:-未知}}"

if [ -z "$CLI_KIND" ]; then
  die "未找到可用的 DSH CLI 入口。已尝试：
  1) $OMNIDESK_CLI_ENTRY_1（需要 $OMNIDESK_ELECTRON_BIN）
  2) $OMNIDESK_CLI_ENTRY_2（需要 node >= 24）"
fi

# 3.7 探测环境变量在子进程中是否真实生效
PKG_BASE="$(dirname "$(dirname "$(dirname "$CLI_ENTRY")")")"
HOME_PATHS_MODULE="$PKG_BASE/dsh-home-paths/lib/index.js"

if [ -f "$HOME_PATHS_MODULE" ]; then
  PROBE_JS='const m = await import(process.argv[1]);
process.stdout.write(JSON.stringify({h: m.resolveDshHome(), a: m.resolveAgentsHome()}));'
  set +e
  if [ "$CLI_KIND" = 'asar' ]; then
    effective="$(ELECTRON_RUN_AS_NODE=1 DSH_HOME="$DSH_HOME_ABS" DSH_AGENTS_HOME="$AGENTS_HOME_ABS" \
      "$CLI_NODE" --input-type=module -e "$PROBE_JS" "$HOME_PATHS_MODULE" 2>/dev/null)"
  else
    effective="$(DSH_HOME="$DSH_HOME_ABS" DSH_AGENTS_HOME="$AGENTS_HOME_ABS" \
      "$CLI_NODE" --input-type=module -e "$PROBE_JS" "$HOME_PATHS_MODULE" 2>/dev/null)"
  fi
  set -e
  if [ -z "$effective" ]; then
    info "提示：环境变量生效性探测未返回结果，改为依赖 DSH 启动时的自我报告。"
  else
    case "$effective" in
      *"$DSH_HOME_ABS"*"$AGENTS_HOME_ABS"*) ;;
      *"$STOCK_DSH_ABS"*) die "探测显示 DSH_HOME 仍解析到原版数据根：$effective" ;;
      *) die "环境变量探测结果异常：$effective" ;;
    esac
  fi
fi

# --------------------------------------------------------------------------
# 4. 启动前摘要
# --------------------------------------------------------------------------
printf '%s\n' "=============================================================="
printf '%s\n' " OmniDesk 独立 DSH 环境（与原版 DSH 完全隔离）"
printf '%s\n' "--------------------------------------------------------------"
printf ' CLI 入口     : %s\n' "$CLI_ENTRY"
printf ' CLI 运行时   : %s (%s)\n' "$CLI_NODE" "$CLI_KIND"
printf ' DSH_HOME     : %s\n' "$DSH_HOME_ABS"
printf ' AGENT_HOME   : %s\n' "$AGENTS_HOME_ABS"
printf ' Profile      : %s\n' "$OMNIDESK_PROFILE"
printf ' Profile 路径 : %s\n' "$PROFILE_REAL"
printf ' 原版数据根   : %s / %s（本次不访问）\n' "$STOCK_DSH_ABS" "$STOCK_AGENTS_ABS"
printf '%s\n' "=============================================================="

if [ "$CHECK_ONLY" -eq 1 ]; then
  info "--check 通过：隔离边界校验全部成功，未启动任何进程。"
  exit 0
fi

# --------------------------------------------------------------------------
# 5. 组装启动命令
# --------------------------------------------------------------------------
ARGS=(--profile "$OMNIDESK_PROFILE")

if [ "${#PASSTHROUGH[@]}" -eq 0 ]; then
  ARGS+=(--host 127.0.0.1 --port 0 --no-open)
else
  # 允许的简化别名：单独写 `web` 等同于“使用 Web Profile 的默认隔离旗标”，
  # 直接把该 token 替换为 yvan-work Profile 上的 Web 旗标。
  FILTERED=()
  for a in "${PASSTHROUGH[@]}"; do
    if [ "$a" = 'web' ]; then
      continue
    fi
    FILTERED+=("$a")
  done
  ARGS+=("${FILTERED[@]+"${FILTERED[@]}"}")

  # 若用户没有显式给出 Web 相关旗标，则补齐隔离必需的安全默认值。
  has_host=0; has_port=0; has_noopen=0
  for a in "${ARGS[@]}"; do
    case "$a" in
      --host|--host=*)        has_host=1 ;;
      --port|--port=*)        has_port=1 ;;
      --no-open|--no-open=*)  has_noopen=1 ;;
    esac
  done
  if [ "$has_host" -eq 0 ]; then ARGS+=(--host 127.0.0.1); fi
  if [ "$has_port" -eq 0 ]; then ARGS+=(--port 0); fi
  if [ "$has_noopen" -eq 0 ]; then ARGS+=(--no-open); fi
fi

RUN_ENV=(DSH_HOME="$DSH_HOME_ABS" DSH_AGENTS_HOME="$AGENTS_HOME_ABS")
if [ "$CLI_KIND" = 'asar' ]; then
  RUN_ENV=(ELECTRON_RUN_AS_NODE=1 "${RUN_ENV[@]}")
fi

if [ "$DRY_RUN" -eq 1 ]; then
  info "--dry-run：以下命令未执行。"
  printf 'env'
  printf ' %q' "${RUN_ENV[@]}"
  printf ' %q' "$CLI_NODE"
  printf ' %q' ${CLI_PREFIX[@]+"${CLI_PREFIX[@]}"}
  printf ' %q' "$CLI_ENTRY"
  printf ' %q' "${ARGS[@]}"
  printf '\n'
  exit 0
fi

info "启动中……（按 Ctrl+C 结束）"
exec env "${RUN_ENV[@]}" "$CLI_NODE" ${CLI_PREFIX[@]+"${CLI_PREFIX[@]}"} "$CLI_ENTRY" "${ARGS[@]}"
