# 阶段一补充任务卡

- 状态：**DSH-P1-A1 已完成并经研究负责人复核**

## 执行修订（研究负责人，2026-11-12）

首批执行证明：对真实 `~/OmniDesk` 数据根运行 DSH `--dump-config` 会刷新生成文件 `~/OmniDesk/runtime/dsh-home/profiles/discipline/cordis.yml` 的 mtime。虽然文件仍是标准空列表 `[]`，但预执行哈希未采集，不能证明内容未变。

后续规则：

- 不得再对真实 `~/OmniDesk` 数据根运行 `--dump-config`。
- 如需组装结果，只能在 `/tmp` 创建 Profile 副本和一次性 `DSH_HOME`，或在已捕获且哈希固定的输出上分析。
- 不尝试恢复该用户文件，不编辑该用户文件，不追加无关写入。

## DSH-P1-A1：修正 P1-05 的 REVIEW.md 引用事实

- **Step ID**：`DSH-P1-A1`
- **单一动作**：只修正 `05-evidence-applicability.md` 中关于当前 `REVIEW.md` 是否包含 E17/E20 回归记录的错误陈述。
- **输入来源**：当前 `REVIEW.md`、`git show HEAD:REVIEW.md`、`d846af3a89:REVIEW.md`、现有 `05-evidence-applicability.md`。
- **输出目标**：更新 `docs/research/dsh-runtime-audit/evidence/phase1/05-evidence-applicability.md`，并加入“研究负责人复核修订”小节。
- **涉及文件**：只读 `REVIEW.md` 和 Git 对象；只写上述一个 Markdown 文件。
- **前置依赖**：DSH-P1-05 已完成。
- **执行范围**：仅修正“当前工作区 `REVIEW.md` 已不含这段历史记录”的错句，以及依赖该错句的“发现的反证”行。
- **明确禁止事项**：不运行任何 `--dump-config`；不启动引擎；不读取凭证、会话正文或任务正文；不修改 `REVIEW.md`、产品源码、`runtime/`、配置或其他证据文件；不改写已验证的版本结论。
- **验收命令或验证方法**：`rg -n 'E17/E20|0\.2\.1-rc\.5' REVIEW.md`；`python3` 读取修正后的 Markdown 并检查关键句；`git diff -- docs/research/dsh-runtime-audit/evidence/phase1/05-evidence-applicability.md`。
- **可自动判断的验收标准**：修正后文件同时包含“当前 `REVIEW.md` 第 1366 行”和“`d846af3a89` 历史提交”；不得再含“当前工作区 `REVIEW.md` 已不含”或“当前 `REVIEW.md` 没有这段”；`rg -n 'E17/E20' REVIEW.md` 必须至少返回第 1366 行；`git diff --check` 通过。
- **必须提交的证据**：修正后的 Markdown、`rg` 输出、`git diff` 摘要、实际改动文件清单和 Git 状态。
