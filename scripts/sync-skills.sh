#!/usr/bin/env bash
# ==============================================================================
# OmniDesk Skill 同步（隔离安全版 · Phase 2A）
#
# 权威源（唯一）：仓库 skills/ 目录。本脚本把 skills/ 同步到 OmniDesk 隔离数据根
# 的 skills 目录，并把 Skill 依赖的写作资产（模板 / 风格规则 / 历史范例）物化到
# 该 Skill 自己的 references/ 子目录中，使 Skill 可以按 DSH 注入的
# "Base directory for this skill: <skill dir>" 提示，用相对路径稳定定位资产。
#
# 设计要点：
#   1. 目标永远由已校验的 OmniDesk 隔离根推导，不接受任意目标路径。
#   2. 任何情况下都拒绝写入 ~/.dsh/skills 或 ~/.agents/skills（原版 DSH 数据）。
#   3. 目标为空、解析异常、或不在允许范围内 -> 立即退出，不做任何写入。
#   4. rsync --delete 只作用于已通过校验的 OmniDesk 目标目录。
#   5. 支持 --dry-run 预览、--check 漂移检查（只读，不写入）。
#   6. 同步前打印源/目标清单，缺 SKILL.md 或使用废弃 frontmatter 键会被标出。
#
# 用法:
#   scripts/sync-skills.sh              # 同步到隔离根的 user-dsh skills
#   scripts/sync-skills.sh --check      # 只比对本仓与隔离副本是否一致（不写入）
#   scripts/sync-skills.sh --dry-run    # 只预览将要发生的改动
#   scripts/sync-skills.sh --to-agents  # 改同步到隔离根的 agents skills
#
# 退出码：0 = 成功（或 --check 未发现漂移）；1 = 失败或发现漂移；2 = 参数错误。
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
SOURCE_DIR="$PROJECT_ROOT/skills"

# 写作资产权威源：仓库内的 templates/ 与 knowledge/。
ASSET_TEMPLATE_SRC="$PROJECT_ROOT/templates/discipline"
ASSET_STYLE_SRC="$PROJECT_ROOT/knowledge/writing-style"
ASSET_CASES_SRC="$PROJECT_ROOT/knowledge/excellent-cases"
# 需要挂载写作资产的 Skill（在各自目标目录内生成 references/）。
ASSET_BOUND_SKILLS=('material-writing' 'document-review')

OMNIDESK_ROOT="${OMNIDESK_ROOT:-$HOME/OmniDesk}"
OMNIDESK_DSH_HOME="$OMNIDESK_ROOT/runtime/dsh-home"
OMNIDESK_AGENTS_HOME="$OMNIDESK_ROOT/runtime/agents-home"

STOCK_DSH_HOME="$HOME/.dsh"
STOCK_AGENTS_HOME="$HOME/.agents"

info() { printf '[OmniDesk][sync] %s\n' "$*"; }
warn() { printf '[OmniDesk][sync][警告] %s\n' "$*" >&2; }
die()  { printf '[OmniDesk][sync][错误] %s\n' "$*" >&2; exit 1; }

# 同步失败保护：只有真正进入同步阶段、且未正常收尾时才提示半更新状态。
SYNC_STARTED=0
SYNC_OK=0
on_exit() {
  local rc=$?
  if [ "$SYNC_STARTED" -eq 1 ] && [ "$SYNC_OK" -eq 0 ] && [ "$DRY_RUN" -eq 0 ]; then
    warn "本次同步未正常完成（退出码 $rc）：运行时副本可能处于半更新状态，请不要依赖当前的局部结果，重新执行 scripts/sync-skills.sh 直至成功。"
  fi
}
trap on_exit EXIT

abspath() {
  if command -v realpath >/dev/null 2>&1; then
    realpath -m -- "$1"
  else
    printf '%s\n' "$1"
  fi
}

# 托管清单读取：只接受纯名字（不含 / 与 ..），防止清单被篡改后越界删除。
manifest_names() {
  [ -f "$MANAGED_MANIFEST" ] || return 0
  grep -E '^[A-Za-z0-9._-]+$' "$MANAGED_MANIFEST" || true
}
is_managed() {
  local want="$1" n
  while IFS= read -r n; do
    [ "$n" = "$want" ] && return 0
  done < <(manifest_names)
  return 1
}

DRY_RUN=0
CHECK_ONLY=0
TARGET_KIND='dsh'
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run)   DRY_RUN=1; shift ;;
    --check)     CHECK_ONLY=1; shift ;;
    --to-agents) TARGET_KIND='agents'; shift ;;
    --help|-h)
      sed -n '2,26p' "${BASH_SOURCE[0]}" | sed -e 's/^# \{0,1\}//'
      exit 0
      ;;
    *) printf '未知参数：%s\n' "$1" >&2; exit 2 ;;
  esac
done

if [ "$CHECK_ONLY" -eq 1 ] && [ "$DRY_RUN" -eq 1 ]; then
  die "--check 与 --dry-run 不能同时使用。"
fi

command -v rsync >/dev/null 2>&1 || die "未找到 rsync，无法同步。"

# --------------------------------------------------------------------------
# 1. 源目录校验与清单
# --------------------------------------------------------------------------
[ -d "$SOURCE_DIR" ] || die "源 Skills 目录不存在：$SOURCE_DIR"

skill_count=0
while IFS= read -r d; do
  [ -n "$d" ] || continue
  skill_count=$((skill_count + 1))
done < <(find "$SOURCE_DIR" -mindepth 1 -maxdepth 1 -type d -print 2>/dev/null | sort)
[ "$skill_count" -gt 0 ] || die "源 Skills 目录为空：$SOURCE_DIR（拒绝用空源执行 --delete 同步）"

# 清单：列出每个 Skill，并标出缺失 SKILL.md / 使用废弃 frontmatter 键的情况。
LEGACY_KEYS='modelInvocable|userInvocable|disableModelInvocation'
inventory() {
  local dir name sk flag
  for dir in "$SOURCE_DIR"/*/; do
    [ -d "$dir" ] || continue
    name="$(basename "$dir")"
    sk="$dir/SKILL.md"
    flag=''
    if [ ! -f "$sk" ]; then
      flag=" [缺 SKILL.md，DSH 不会发现该 Skill]"
    elif grep -Eq "^[[:space:]]*($LEGACY_KEYS):" "$sk" 2>/dev/null; then
      flag=" [使用已废弃的 camelCase frontmatter 键，DSH 会忽略整个 Skill]"
    fi
    printf '  - %s%s\n' "$name" "$flag"
  done
}

info "源目录   : $SOURCE_DIR（$skill_count 个 Skill）"
info "源清单："
inventory

# --------------------------------------------------------------------------
# 2. 目标推导与校验
# --------------------------------------------------------------------------
OMNIDESK_ROOT_ABS="$(abspath "$OMNIDESK_ROOT")"
STOCK_DSH_ABS="$(abspath "$STOCK_DSH_HOME")"
STOCK_AGENTS_ABS="$(abspath "$STOCK_AGENTS_HOME")"

if [ "$TARGET_KIND" = 'agents' ]; then
  BASE_HOME_ABS="$(abspath "$OMNIDESK_AGENTS_HOME")"
else
  BASE_HOME_ABS="$(abspath "$OMNIDESK_DSH_HOME")"
fi

[ -n "$BASE_HOME_ABS" ] || die "隔离根为空，拒绝同步。"
TARGET_DIR="$BASE_HOME_ABS/skills"
# 托管清单：记录由本同步器创建/更新的 Skill 名。删除范围以清单为界，
# 清单之外的目标目录一律视为用户自有内容，永不删除。
MANAGED_MANIFEST="$TARGET_DIR/.omnidesk-managed"

# 2.1 目标必须位于 OmniDesk 根之下
case "$TARGET_DIR/" in
  "$OMNIDESK_ROOT_ABS"/*) ;;
  *) die "目标 $TARGET_DIR 不在 OmniDesk 根 $OMNIDESK_ROOT_ABS 之下，拒绝同步。" ;;
esac

# 2.2 目标绝不能位于原版数据根之下
case "$TARGET_DIR/" in
  "$STOCK_DSH_ABS/"*) die "目标落入原版 DSH 数据根 $STOCK_DSH_ABS，拒绝同步。" ;;
esac
case "$TARGET_DIR/" in
  "$STOCK_AGENTS_ABS/"*) die "目标落入原版 Agent 数据根 $STOCK_AGENTS_ABS，拒绝同步。" ;;
esac
if [ "$TARGET_DIR" = "$STOCK_DSH_ABS" ] || [ "$TARGET_DIR" = "$STOCK_AGENTS_ABS" ]; then
  die "目标等于原版数据根，拒绝同步。"
fi

# 2.3 目标不能是软链接（避免穿透到未校验位置）
if [ -L "$TARGET_DIR" ]; then
  die "目标 $TARGET_DIR 是软链接，拒绝同步。"
fi

# 2.4 资产源存在性（只有 user-dsh 目标才挂载 references/）
HAS_ASSETS=0
if [ "$TARGET_KIND" = 'dsh' ] \
   && [ -d "$ASSET_TEMPLATE_SRC" ] && [ -d "$ASSET_STYLE_SRC" ] && [ -d "$ASSET_CASES_SRC" ]; then
  HAS_ASSETS=1
fi

info "目标目录 : $TARGET_DIR"
if [ "$CHECK_ONLY" -eq 1 ]; then
  info "模式     : check（只读比对，不写入）"
elif [ "$DRY_RUN" -eq 1 ]; then
  info "模式     : dry-run（不写入）"
else
  info "模式     : 实际同步"
fi

if [ "$CHECK_ONLY" -eq 0 ] && [ ! -d "$BASE_HOME_ABS" ]; then
  die "隔离根尚未初始化：$BASE_HOME_ABS（请先运行 scripts/omnidesk-dev.sh --check）"
fi

# --------------------------------------------------------------------------
# 3. --check：只读漂移比对
# --------------------------------------------------------------------------
if [ "$CHECK_ONLY" -eq 1 ]; then
  if [ ! -d "$TARGET_DIR" ]; then
    warn "隔离副本不存在：$TARGET_DIR —— 需要执行同步。"
    exit 1
  fi
  drift=0
  for skill_dir in "$SOURCE_DIR"/*/; do
    [ -d "$skill_dir" ] || continue
    name="$(basename "$skill_dir")"
    if ! diff -rq --exclude='references' "$skill_dir" "$TARGET_DIR/$name" >/dev/null 2>&1; then
      drift=1
      printf '  [漂移] %s\n' "$name"
    fi
  done
  # 目标中多出的 Skill 目录：只有「清单内但源已删除」才算漂移；
  # 清单之外的目录是用户自有内容，仅提示，不计入漂移。
  for tdir in "$TARGET_DIR"/*/; do
    [ -d "$tdir" ] || continue
    name="$(basename "$tdir")"
    [ -d "$SOURCE_DIR/$name" ] && continue
    if is_managed "$name"; then
      drift=1
      printf '  [多余] %s（受同步器托管，但源目录中已不存在）\n' "$name"
    else
      printf '  [未托管] %s（用户自有内容，同步器不会删除）\n' "$name"
    fi
  done
  if [ "$HAS_ASSETS" -eq 1 ]; then
    for bound in "${ASSET_BOUND_SKILLS[@]}"; do
      [ -d "$TARGET_DIR/$bound" ] || continue
      ref="$TARGET_DIR/$bound/references"
      if [ ! -d "$ref" ]; then
        drift=1
        printf '  [缺失] %s/references（写作资产未物化）\n' "$bound"
        continue
      fi
      # 资产内容也必须一致：仅检查目录是否存在会漏掉内容漂移。
      for pair in "templates:$ASSET_TEMPLATE_SRC" "writing-style:$ASSET_STYLE_SRC" "excellent-cases:$ASSET_CASES_SRC"; do
        sub="${pair%%:*}"; asrc="${pair##*:}"
        if ! diff -rq "$asrc" "$ref/$sub" >/dev/null 2>&1; then
          drift=1
          printf '  [漂移] %s/references/%s（与仓库权威源不一致）\n' "$bound" "$sub"
        fi
      done
    done
  fi
  if [ "$drift" -eq 0 ]; then
    info "--check：隔离副本与仓库源一致（未发现漂移）。"
    info "（注：未托管目录属于用户自有内容，不参与漂移判定。）"
    exit 0
  fi
  info "--check：发现漂移。执行 scripts/sync-skills.sh 可更新隔离副本。"
  exit 1
fi

# --------------------------------------------------------------------------
# 4. 逐 Skill 同步
# --------------------------------------------------------------------------
RSYNC_FLAGS=(-a --delete --itemize-changes)
if [ "$DRY_RUN" -eq 1 ]; then
  RSYNC_FLAGS+=(--dry-run)
else
  SYNC_STARTED=1
fi

synced=0
new_list=()
upd_list=()
for skill_dir in "$SOURCE_DIR"/*/; do
  [ -d "$skill_dir" ] || continue
  skill_name="$(basename "$skill_dir")"
  if [ -d "$TARGET_DIR/$skill_name" ]; then
    upd_list+=("$skill_name")
  else
    new_list+=("$skill_name")
  fi
  info "同步 Skill: $skill_name"
  rsync "${RSYNC_FLAGS[@]}" "$skill_dir" "$TARGET_DIR/$skill_name/"
  synced=$((synced + 1))
done

# --------------------------------------------------------------------------
# 4.5 清除「受本同步器托管、且源目录中已删除」的 Skill
#
#     删除范围严格以托管清单（$MANAGED_MANIFEST）为界：
#       - 只删除「清单中有、源目录已没有」的目录；
#       - 清单之外的任何目录都视为用户自有内容，一律保留并提示；
#       - 清单不存在时不做任何删除（避免首次同步或清单损坏时误删）。
# --------------------------------------------------------------------------
pruned=0
prune_list=()
skip_list=()
if [ -d "$TARGET_DIR" ] && [ -f "$MANAGED_MANIFEST" ]; then
  for tdir in "$TARGET_DIR"/*/; do
    [ -d "$tdir" ] || continue
    tname="$(basename "$tdir")"
    [ -d "$SOURCE_DIR/$tname" ] && continue
    if ! is_managed "$tname"; then
      info "保留未托管目录：$tname（用户自有内容，不在同步器管理范围）"
      skip_list+=("$tname")
      continue
    fi
    if [ "$DRY_RUN" -eq 1 ]; then
      info "（dry-run）将删除托管 Skill：$tname（源目录中已不存在）"
    else
      info "删除托管 Skill: $tname（受托管且源目录中已不存在）"
      rm -rf -- "$tdir"
    fi
    prune_list+=("$tname")
    pruned=$((pruned + 1))
  done
elif [ -d "$TARGET_DIR" ] && [ "$DRY_RUN" -eq 0 ]; then
  info "托管清单不存在，本次不做任何删除（首次同步或清单缺失时属预期行为）。"
fi

# --------------------------------------------------------------------------
# 5. 物化写作资产到 Skill 的 references/（同步之后执行，避免被 --delete 清掉）
# --------------------------------------------------------------------------
materialize_assets() {
  local bound="$1"
  local ref="$TARGET_DIR/$bound/references"
  info "物化写作资产 -> $ref"
  mkdir -p "$ref"
  rsync -a --delete "$ASSET_TEMPLATE_SRC/" "$ref/templates/"
  rsync -a --delete "$ASSET_STYLE_SRC/" "$ref/writing-style/"
  rsync -a --delete "$ASSET_CASES_SRC/" "$ref/excellent-cases/"
  printf '%s\n' \
    '# 材料写作资产（运行时物化副本）' \
    '' \
    '本目录由 scripts/sync-skills.sh 从仓库权威源自动生成，请勿在此处直接编辑。' \
    '修改请回到仓库源，然后重新执行同步。' \
    '' \
    '| 运行时路径 | 仓库权威源 |' \
    '| --- | --- |' \
    '| references/templates/ | templates/discipline/ |' \
    '| references/writing-style/ | knowledge/writing-style/ |' \
    '| references/excellent-cases/ | knowledge/excellent-cases/ |' \
    '' \
    '历史范例目录若仅有 README.md，说明尚无范例；Skill 必须如实说明未使用历史范例。' \
    > "$ref/README.md"
}

if [ "$HAS_ASSETS" -eq 1 ]; then
  mounted=0
  for bound in "${ASSET_BOUND_SKILLS[@]}"; do
    [ -d "$TARGET_DIR/$bound" ] || continue
    if [ "$DRY_RUN" -eq 1 ]; then
      info "（dry-run）将为 $bound 写入 references/（templates/ writing-style/ excellent-cases/）"
    else
      materialize_assets "$bound"
    fi
    mounted=$((mounted + 1))
  done
  if [ "$mounted" -eq 0 ]; then
    warn "未找到需要挂载写作资产的 Skill 目标目录，跳过 references/ 物化。"
  elif [ "$DRY_RUN" -eq 0 ]; then
    info "写作资产已物化到 $mounted 个 Skill。"
  fi
elif [ "$TARGET_KIND" = 'dsh' ]; then
  warn "未找到完整的写作资产源，跳过 references/ 物化（Skill 将按缺少资产处理）。"
fi

# --------------------------------------------------------------------------
# 6. 收尾：写托管清单 + 汇总
#     清单在"全部同步成功之后"才写入；写入前后任何异常都会触发半更新告警。
# --------------------------------------------------------------------------
join_list() {
  local out="" n
  for n in "$@"; do out+="$n "; done
  if [ -z "$out" ]; then printf "（无）"; else printf "%s" "$out"; fi
}

if [ "$DRY_RUN" -eq 1 ]; then
  printf "\n[OmniDesk][sync] ===== dry-run 汇总（未写入任何内容）=====\n"
  printf "  将新增 Skill : %s\n" "$(join_list ${new_list[@]+"${new_list[@]}"})"
  printf "  将更新 Skill : %s\n" "$(join_list ${upd_list[@]+"${upd_list[@]}"})"
  printf "  将删除 Skill : %s\n" "$(join_list ${prune_list[@]+"${prune_list[@]}"})"
  printf "  保留未托管   : %s\n" "$(join_list ${skip_list[@]+"${skip_list[@]}"})"
  info "完成：共 $synced 个 Skill（dry-run，未写入）-> $TARGET_DIR"
else
  # 托管清单 = 本次由同步器管理的全部 Skill 名（即源目录清单）。
  : > "$MANAGED_MANIFEST"
  for skill_dir in "$SOURCE_DIR"/*/; do
    [ -d "$skill_dir" ] || continue
    printf "%s\n" "$(basename "$skill_dir")" >> "$MANAGED_MANIFEST"
  done
  SYNC_OK=1
  printf "\n[OmniDesk][sync] ===== 同步汇总 =====\n"
  printf "  新增 Skill : %s\n" "$(join_list ${new_list[@]+"${new_list[@]}"})"
  printf "  更新 Skill : %s\n" "$(join_list ${upd_list[@]+"${upd_list[@]}"})"
  printf "  删除托管   : %s\n" "$pruned"
  printf "  保留未托管 : %s\n" "$(join_list ${skip_list[@]+"${skip_list[@]}"})"
  info "完成：共处理 $synced 个 Skill -> $TARGET_DIR"
  info "托管清单已更新：$MANAGED_MANIFEST"
fi
