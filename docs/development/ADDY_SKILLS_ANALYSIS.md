# Addy Osmani agent-skills 分析与 DeepSeek 执行规范适配

> 研究对象：https://github.com/addyosmani/agent-skills（2026-11-04 克隆 main 快照到 /private/tmp/addy-research，仅阅读研究，未安装、未引入）。
> 本文区分：**已验证事实**、**研究结论**、**建议**。

## 一、仓库真实能力（已验证事实）

- `skills/` 下共 40 个技能目录。本次指定的 8 个研究对象中：spec-driven-development、planning-and-task-breakdown、constraint-driven-development、test-driven-development、code-review-and-quality、context-engineering、code-simplification 均存在；**`diagnosing-bugs` 不存在**，实际对应技能是 `debugging-and-error-recovery`（仓库另有 doubt-driven-development、incremental-implementation、source-driven-development 等，不在本次指定范围）。
- 全部为提示词型技能，无运行时强制；与 Superpowers 同类，质量取决于流程设计本身。
- 部分技能依赖仓库内的 hooks / commands / evals 配套，OmniDesk 一概不引入，只吸收 SKILL.md 中的流程规则。

## 二、逐技能分析

### 1. spec-driven-development

**事实：** SPECIFY → PLAN → TASKS → IMPLEMENT 四阶段，每阶段经人工把关才进下一阶段；一个请求捆绑多个可独立验收的能力时，先出"能力地图"（模块表：职责 + 依赖 + 构建顺序），模块 id 稳定后按依赖序逐模块走四阶段。

**研究结论：** OmniDesk 已有 GOALS.md → PLANS.md 对应此法的前两阶段；"能力地图"对大型功能立项（如知识库、Profile 体系）有用。

**建议：** 吸收能力地图用于大功能立项：模块表直接用 OD-ARCH 编号做 id，画进 PLANS.md 对应小节，不新建文件体系。

### 2. planning-and-task-breakdown

**事实：** 先进入只读计划模式（读 spec 与相关代码、识别既有模式、映射依赖），输出任务清单；每任务小到"单次专注会话可完成、可实施、可测试、可验证"，且带明确验收标准；绘制依赖图确定顺序。

**研究结论：** 与 Superpowers writing-plans 互补：这个更轻量，适合中小任务；writing-plans 更重，适合多任务特性。

**建议：** 作为 GLM 拆任务的粒度下限来源：单任务以"一次 DeepSeek 会话可完成"为上限。

### 3. constraint-driven-development

**事实：** 把项目质量底线写成带数字的书面合同（CONSTRAINTS.md）；提供机械检查思路：监视 diff 中降低门槛的迹象（新增 lint/test 豁免、跳过或删除测试、删断言、改低阈值）；缺失时按四问（每问带默认值）建立。

**研究结论：** 价值不在 CONSTRAINTS.md 这个文件，而在"质量底线可机械检查 + 禁止悄悄降门槛"两条。OmniDesk 不缺检查体系，缺的是把门禁组合写成明确分级。

**建议：** 不新增 CONSTRAINTS.md；把等价内容并入工程规范「质量门禁」节：(a) 按任务类型分级的门禁组合；(b) 红线——不得为绿灯新增豁免、跳过或删除既有测试、改低阈值；DeepSeek 报告须声明未新增豁免。

### 4. test-driven-development

**事实：** 红绿循环；先发现本仓库的真实测试命令（读 package.json / Cargo.toml / CI），不假设默认命令；修 bug 用 Prove-It 模式——先写复现测试证明缺陷存在。

**研究结论：** "先发现仓库命令"是普适且低成本的一条，OmniDesk 的真实命令分散在 STATUS.md 各轮记录中，应集中登记一次。

**建议：** 并入执行规范：集中登记仓库验证命令清单（cargo test --offline --lib、npm --prefix desktop run build、pytest、verify 脚本）；缺陷修复复现先行（与 Superpowers 结论一致，取并集）。

### 5. code-review-and-quality

**事实：** 五轴审查：正确性 / 可读性与简洁 / 架构 / 安全 / 性能；审批标准「明确改善整体代码健康度即通过，不求完美」；架构轴关注"重构是消解了复杂度还是只是搬移了复杂度"。

**研究结论：** 给 GLM 审查 REVIEW.md 提供了现成的检查轴；"不求完美"的审批标准可防止审查环节的自我膨胀。

**建议：** 吸收为 GLM 审查五轴 + 审批标准；写入工程规范。

### 6. context-engineering

**事实：** 五层上下文金字塔：常驻规则文件 → 每特性架构文档 → 每任务相关源文件 → 每迭代错误输出/测试结果 → 会话历史（累积、压缩）；规则文件内容：技术栈、命令、约定、边界、模式。

**研究结论：** 这是本次上下文研究的骨架。OmniDesk 现有资产可直接对号入座：AGENTS.md + 工程规范 = 层 1；GOALS/PLANS/decisions = 层 2；任务卡点名的文件与行号 = 层 3；STATUS 证据 = 层 4；历史会话 = 层 5（可丢弃）。

**建议：** 作为 CONTEXT_MANAGEMENT_STANDARD.md 的分层框架，不引入其 CLAUDE.md 模板细节。

### 7. debugging-and-error-recovery（替代任务书中的 diagnosing-bugs）

**事实：** 停线规则：出现意外失败即停止推进新功能、保留证据、按分诊清单走；六步分诊：复现 → 定位 → 最小化 → 根因 → 修复 → 防复发，顺序不可跳。

**研究结论：** 与 Superpowers systematic-debugging 高度重叠（先复现、先根因、停线）。取其重合核心即可，不必双轨。

**建议：** 并入执行规范错误排查节；技能层面两个来源只登记一条合并规则。

### 8. code-simplification

**事实：** 五原则，第一原则「行为完全不变」；明确的不适用场景：代码已干净、还没理解代码、性能关键路径、即将整体重写。

**研究结论：** OmniDesk 现行约束是"不做无关重构"，主动简化只应由计划发起；该技能的"何时不用"清单正好用来约束 DeepSeek 不自发简化。

**建议：** 暂不作为常规流程；专项债务清理任务由 GLM 在计划中明确列为任务后，DeepSeek 才执行简化，且简化任务的行为等价验证必须有既有测试覆盖。

## 三、对重点研究问题的回答（执行规范视角）

| 问题 | 回答（详见工程规范草案） |
| --- | --- |
| 如何规范 DeepSeek 实施？ | 任务卡（边界 + 验收命令）+ 修改前检查 + 固定报告模板 |
| 避免无关重构与过度设计？ | 红线条款 + "简化只由计划发起" |
| 控制修改文件范围？ | 任务卡列允许文件清单；超出即停（Superpowers 停下条件之一） |
| 符合既有架构？ | 层 2 架构文档 + 任务卡"架构契合说明"一行 |
| 测试先行？ | 缺陷复现先行；新行为任务卡带验收测试 |
| 自动化验收与门禁？ | 按任务类型分级的门禁组合，证据落盘 tests/verification/ |
| 避免技术债？ | 报告必列"未做与已知限制"；GLM 按五轴审查 |
| 避免无价值输出？ | 报告模板定字段；工具调用间叙述限一行 |
| 控制上下文膨胀？ | 见 CONTEXT_MANAGEMENT_STANDARD.md |
