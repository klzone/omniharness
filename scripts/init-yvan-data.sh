#!/usr/bin/env bash
# ==============================================================================
# OmniDesk / Yvan AI Workbench — 个人数据底座初始化脚本
# 适用环境：Ubuntu / Linux / macOS 新机器或本地初始化
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
CONFIG_SRC="$PROJECT_ROOT/config/yvan-work.yaml"

# 默认目标路径：~/YvanAI，允许通过参数 $1 覆盖
BASE_DIR="${1:-$HOME/YvanAI}"

echo "================================================================="
echo "   OmniDesk / Yvan AI Workbench: 初始化个人数据底座 (Phase 1)    "
echo "================================================================="
echo "目标根路径: $BASE_DIR"

# ------------------------------------------------------------------------------
# 1. 检查执行环境与权限
# ------------------------------------------------------------------------------
echo -n "[1/4] 检查目录创建与写入权限... "
PARENT_DIR="$(dirname "$BASE_DIR")"
if [ ! -d "$PARENT_DIR" ] || [ ! -w "$PARENT_DIR" ]; then
  echo "FAIL"
  echo "错误: 父级目录 $PARENT_DIR 不存在或当前用户无写入权限！"
  exit 1
fi
echo "OK"

# ------------------------------------------------------------------------------
# 2. 依据标准规范建立目录架构
# ------------------------------------------------------------------------------
echo "[2/4] 正在建立 ~/YvanAI 目录层级架构..."

# 知识库体系
mkdir -p "$BASE_DIR/knowledge/obsidian"
mkdir -p "$BASE_DIR/knowledge/documents"
mkdir -p "$BASE_DIR/knowledge/references"

# 长期记忆体系
mkdir -p "$BASE_DIR/memory/decisions"
mkdir -p "$BASE_DIR/memory/conversations"
mkdir -p "$BASE_DIR/memory/summaries"

# 项目管理体系
mkdir -p "$BASE_DIR/projects/V2T"
mkdir -p "$BASE_DIR/projects/Omnidesk"
mkdir -p "$BASE_DIR/projects/Website"

# 基础辅助目录
mkdir -p "$BASE_DIR/templates"
mkdir -p "$BASE_DIR/backups"
mkdir -p "$BASE_DIR/logs"

# 调整目录权限为仅当前用户读写 (0700)
chmod -R 700 "$BASE_DIR" 2>/dev/null || true
echo "      目录层级构建完毕并收紧权限 (0700)。"

# ------------------------------------------------------------------------------
# 3. 安装工作台全局配置文件与说明规范
# ------------------------------------------------------------------------------
echo "[3/4] 部署工作台环境配置文件与各子目录说明..."

if [ -f "$CONFIG_SRC" ]; then
  cp "$CONFIG_SRC" "$BASE_DIR/yvan-work.yaml"
  echo "      已同步主配置文件 -> $BASE_DIR/yvan-work.yaml"
else
  echo "      警告: 未找到源配置文件 $CONFIG_SRC，跳过配置复制。"
fi

# 在根目录生成规范说明 (README.md)
cat > "$BASE_DIR/README.md" << 'README_EOF'
# YvanAI 个人数据底座 (External Data Center)

> 本目录是 **OmniDesk / Yvan AI Workbench** 的终身数字资产中枢，独立于任何特定软件或 Agent 引擎。

## 目录索引

- **knowledge/**：个人结构化知识中枢
  - obsidian/：Obsidian 双链笔记卡片盒
  - documents/：工作材料、调研报告与文档
  - references/：外部技术文章、论文与竞品资料
- **memory/**：AI 长期记忆中枢（通用 Markdown/JSON 存储）
  - decisions/：重大技术选型与业务决策日志
  - conversations/：高价值对话精华与上下文沉淀
  - summaries/：各项目定期进展与复盘简报
- **projects/**：核心项目跟踪
  - V2T/：视频转文本项目进展与技术资产
  - Omnidesk/：个人 AI 工作操作系统规划
  - Website/：个人网站与公开技术输出平台
- **templates/**：各 Agent 调用的标准化文档模版
- **backups/**：DSH 会话与配置定期镜像归档
- **logs/**：数据同步与自动化备份日志
README_EOF

cat > "$BASE_DIR/memory/decisions/README.md" << 'DEC_EOF'
# 决策记录库 (Decisions)
命名格式：YYYY-MM-DD_{topic}.md
由 Memory Agent 在产生重大技术方案或架构取舍时自动录入。
DEC_EOF

cat > "$BASE_DIR/memory/summaries/README.md" << 'SUM_EOF'
# 项目状态总结 (Summaries)
命名格式：{project}_summary.md
由 Project Agent 定期对各工程 README / TODO 巡检后汇总产出。
SUM_EOF

# ------------------------------------------------------------------------------
# 4. 最终自检与结构输出
# ------------------------------------------------------------------------------
echo "[4/4] 执行结构自检..."

if [ -f "$BASE_DIR/yvan-work.yaml" ] && [ -d "$BASE_DIR/knowledge/obsidian" ] && [ -d "$BASE_DIR/memory/decisions" ]; then
  echo "================================================================="
  echo "  ✅ YvanAI 数据底座初始化成功！环境已就绪。"
  echo "================================================================="
  if command -v tree >/dev/null 2>&1; then
    tree -L 2 "$BASE_DIR"
  else
    ls -la "$BASE_DIR"
  fi
else
  echo "❌ 初始化自检未完全通过，请核查权限或路径！"
  exit 1
fi
