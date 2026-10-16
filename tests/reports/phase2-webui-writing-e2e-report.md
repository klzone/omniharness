# OmniDesk 阶段 2：Web UI 真实模型端到端材料写作验证报告

> 日期：2026-10-16
> 执行方式：OmniDesk 隔离实例 Web UI 真实会话（非静态检查，非合成桩）
> 证据等级：真实模型输出 + 工具调用级可观察证据 + 独立逐字复核

## 1. 验证目标

在隔离运行的 OmniDesk 实例中，通过 Web UI 用 `material-writing` 技能，让真实模型读取真实素材，生成一篇可追溯、可人工审核的材料，并核实：

1. 技能资产（模板、风格规则、范例目录）是否被实际读取；
2. 真实法规文档（.docx）能否被解析并逐字引用；
3. 引用是否与原文逐字一致，事实与判断是否分离；
4. 是否如实声明“未使用历史范例”；
5. 是否标注待确认事项，是否声明未编造数据。

## 2. 运行环境

| 项 | 值 |
| --- | --- |
| 实例 | OmniDesk 隔离实例，端口 127.0.0.1:3210 |
| 启动方式 | `scripts/omnidesk-dev.sh --port 3210` |
| DSH_HOME | `/home/kevin/OmniDesk/runtime/dsh-home` |
| DSH_AGENTS_HOME | `/home/kevin/OmniDesk/runtime/agents-home` |
| 模型 | opendesign / deepseek-v4.1-flash，reasoningEffort high |
| 工作区 | /mnt/data/Code/MyWorkStation |
| 访问模式 | 工作区内修改（workspace-write） |

## 3. 任务与素材

- 用途：理论学习「交流研讨发言」，约 1000 字。
- 真实素材（合规公开法规）：knowledge/regulations/处分条例/中国共产党纪律处分条例2023版.docx
- 硬要求：引用条文须给出条号与原文、不得改写；分析判断须与条文原文区分；末尾附六项交付说明。

## 4. 工具调用级可观察证据

以下均来自会话的可访问性（AX）树工具调用记录，非模型自述：

技能与资产读取（真实文件读取事件）：

```text
加载技能 material-writing
读取 references/README.md
读取 references/templates/speech-template.md
读取 references/templates/report-template.md
读取 references/templates/work-summary-template.md
读取 references/writing-style/style-guide.md
读取 references/writing-style/common-phrases.md
读取 references/writing-style/structure-patterns.md
读取 references/writing-style/avoid-list.md
读取 references/excellent-cases/README.md
读取 knowledge/excellent-cases/README.md
运行命令 diff -rq 仓库源与隔离根副本 → "Assets are identical"
```

真实文档解析：运行命令提取 .docx 正文，确认 371 个非空段落、共 158 条；并交叉核对同目录 2018 版（142 条）确认版本未混用。

落盘：`workstations/discipline/drafts/2026-10-16_学习中国共产党纪律处分条例2023版交流研讨发言稿.md`（9881 字节）。

模型用量：1 轮 40 步，约 1.3M tok，缓存命中 97%，用时 4 分 5 秒。

## 5. 独立逐字复核（本报告作者执行，非模型自述）

复核脚本从 .docx 提取正文，与文稿中的引用逐字比对：

| 项 | 结果 |
| --- | --- |
| 引用框条文块逐字比对 | 6/6 通过 |
| 行内引用（第三条、第十九条第二款、第一百三十九条第一款、第一百四十条第一款、第一百五十八条） | 全部命中原文 |
| ASCII 引号残留 | 0（全文中文引号 41 对，配对一致） |
| 正文汉字数 | 1402；其中引用条文 310，本人行文 1092 |
| 禁用语（“深刻的不仅是”“全面赋能”“四种方式”）出现在正文 | 否（仅出现在交付说明的“已避免”清单） |
| 声明“未使用历史范例” | 是 |
| 含“待确认事项”清单 | 是（6 项） |
| 声明未编造工作成效/数据/文号 | 是 |
| 声明不含案件材料、案件线索、未公开调查信息 | 是 |

## 6. 隔离与数据安全核验

| 检查 | 结果 |
| --- | --- |
| 原版 `~/.dsh/.credentials.yaml` mtime/大小 | 未变化（2026-10-15 14:09:14，1206B） |
| 原版 `~/.dsh/storages/workspace.json` mtime/大小 | 未变化（2026-10-15 23:54，3762B） |
| 原版 `~/.dsh/sessions/` 最新目录 | 未变化（2026-10-15 23:54） |
| 原版 DSH Desktop 进程 | 存活，未受影响 |
| DSH Runtime 源码 | 未修改 |
| 原始 .docx 素材 | 未覆盖（新建文件） |
| 临时提取文件 | 已清理，仓库无残留 .tmp* |

## 7. 已知限制

- 本报告验证的是隔离实例 Web UI 中的真实模型输出，不等于 OmniDesk 自研桌面应用的端到端能力。
- 素材为合规公开法规，未验证敏感数据阻断路径（该路径此前以合成占位内容在 tests/verification/phase1/ 中验证）。
- 范例目录 knowledge/excellent-cases/ 目前仅有 README，无范文；模型已如实声明未使用范例。放入脱敏范文可提升少样本效果。

## 8. 结论

材料写作技能在隔离实例中完成了真实模型、真实素材、逐字可核的端到端链路验证：技能被发现、资产被实际读取、真实 .docx 被解析、引用与原文逐字一致、事实与判断分离、缺失信息被标注、数据边界被声明，且原版 DSH 数据与 Runtime 均未受影响。

