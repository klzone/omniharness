# Yvan AI Workbench

个人定制的专属 AI 工作台项目。以 [DeepSeek Harness (DSH)](https://github.com/deepseek-ai/deepseek-harness) 为底层 Agent Runtime，面向个人知识管理、内容生产、项目协作和长期记忆积累打造。

---

## 核心设计原则

1. **数据资产独立**：所有知识资产、记忆沉淀存放在 `~/YvanAI/`（通用 Markdown/JSON 格式），与 Agent 软件解耦。
2. **不修改 DSH 核心**：通过 Profile、Bundle、Skill 与外部配置进行渐进式增强，保障未来上游升级无痛无损。
3. **稳定优先**：生产工作环境使用稳定配置，开发探索与版本测试走独立隔离层。

---

## 目录结构说明

```text
MyWorkStation/
├── runtime/              # DSH 核心运行时底座
│   ├── deepseek-harness/ # DSH 官方干净源码 (v0.2.1-alpha.2，独立 git tracking)
│   └── patches/          # 针对 Linux 打包与环境适配的补丁归档
├── profiles/             # Cordis / DSH Profile 定制配置
│   ├── desktop/          # 与当前正在运行的 Desktop 保持同步的配置快照
│   └── yvan-work/        # 面向个人定制工作台扩展的 Profile
├── skills/               # 专属 Agent Skills (以 Markdown 指令驱动)
│   ├── memory-agent/     # 长期记忆与关键决策记录
│   ├── knowledge-agent/  # 知识检索、Obsidian/Hugo 整合
│   ├── project-agent/    # 工程巡检与进度跟踪
│   ├── writing-agent/    # 写作与材料输出
│   └── coding-agent/     # 辅助研发与代码分析
├── data-template/        # ~/YvanAI/ 外部资产目录的基础规范与模板
├── scripts/              # 自动化运维脚本 (配置同步、会话备份等)
└── docs/                 # 架构分析报告与使用指引
```

---

## 常用运维脚本

- `./scripts/init-yvan-data.sh`: 快速初始化本地 `~/YvanAI` 外部存储骨架。
- `./scripts/backup-sessions.sh`: 归档并备份当前 DSH 的会话与索引数据。
- `./scripts/sync-skills.sh`: 将工作台编写的 Agent Skills 挂载/同步至 DSH 搜索目录。

