# 04 GitHub 说明修订稿

## 问题（事实）
- README.md 停在 OD-BUILD-004 时代，与实际（065 完成、可实测）严重不符。
- 仓库公开（private=false），description 为「Unified Agent Runtime & Personal Knowledge Workbench」——偏引擎描述，未体现产品定位。
- Topics 为空。
- knowledge/regulations/ 下 232 份银行内部制度已正确被 .gitignore 排除，git ls-files 验证只跟踪 README.md。但 README 未说明。

## 建议 README 结构（全文约 70 行，见对话记录；ARCHITECT 建议作为下一任务由 BUILDER 落盘）
1. 项目名 + 一句话定位（对话为中枢、场景工作流为骨架、成果档案为沉淀；首个场景：纪检）。
2. 解决什么问题：专业文字工作需要状态、依据、复核、归档，通用聊天工具给不了。
3. 5 条核心特征（对话即工作 / 场景工作流 / 写确认制 / 数据自主 / 权限可控）。
4. 当前状态（可运行、作者实测中、未做数据分析/同事试用/打包分发）。
5. 快速开始（3 行命令）。
6. 架构表（Tauri/React + Rust 宿主 + Python 适配 + DSH 锁定底座）。
7. 仓库边界（真实材料不入库、不改 runtime/）。
8. Apache-2.0。

## 建议 GitHub About 文本（需用户自行更新，gh CLI 未登录）
- Description: Personal AI workspace for professional writing & project tracking — conversation-driven, with scene workflows, verified outputs, and local-first data. 首个场景：纪检工作台。
- Topics: ai-workspace, tauri, personal-knowledge, local-first, deepseek

## 无需修改
- LICENSE（Apache-2.0 已存在）、.gitignore（内部制度已正确排除）、docs/、tests/。

