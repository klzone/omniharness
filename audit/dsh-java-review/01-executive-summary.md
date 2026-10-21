# dsh-java 审计执行摘要（2026-10-21，ARCHITECT）

## 审计对象核实
- 仓库 https://github.com/linlannet/dsh-java ，HEAD 3b9f729（浅克隆），MIT License（Copyright 2026 LINLAN），版本 0.2.1（2026-10-11）。
- 定性：小傅哥 DeepSeek Harness 的 Java 延伸实现，六边形架构，191 个测试类。README 自述"开发者预览阶段，接口与插件协议迭代较快，可能含不兼容变更"。
- 关键定位结论：dsh-java 与 OmniDesk 的 runtime/ DSH 同源同层（都是执行引擎）。OmniDesk 不拥有 ReAct 循环、工具执行器和会话存储——这些在 DSH 内部。因此借鉴对象只能是"设计思想"，不是代码；且凡 DSH 已承担的职责，OmniDesk 不应重复建设。

## 核心结论
1. dsh-java 有真实工程价值（非纸面架构）：191 个测试类、五阶段工具管线、事件溯源会话日志带迁移测试、压缩引擎带裁剪/卸载组件，均有源码与测试证据。
2. 但对 OmniDesk 的可借鉴面很窄：六个审计领域中，A（ReAct 内核）、C（插件三机制）属 DSH/平台职责，OmniDesk 无落点且借鉴会违反"不做插件市场/Registry"既定原则。真正有落点的是 B 的任务链终检模式、D 的日志版本化、E 的 Token 快照投影。
3. 无 P0 问题：本审计未在 OmniDesk 现有实现中发现需立即处理的高风险缺陷。

## 最值得借鉴三项
1. 执行入口 fail-closed 终检（P1，约20行）：任务链在真正执行前再校验一次"无未决审批+分区匹配"，防止未来新增路径绕过写确认制。
2. 会话日志格式版本化+迁移测试（P1，约60行）：给 ~/OmniDesk 线程持久化文件加 version 字段与旧格式迁移测试，直接服务"会话不丢"核心目标。
3. Token 快照持久化投影（P2，约150行）：会话/任务级 token 用量落盘，供设置页与公告栏显示累计消耗。

## 最需要警惕三项
1. 审批默认放行：ToolCallExecutor 的 approvalGate 默认 RuntimeApprovalGate.allowAll()（第 65 行），审批是配置项不是默认行为——OmniDesk 必须反着学：默认拒绝、显式开启。
2. 预览阶段不可依赖：接口快速迭代、无兼容承诺，任何"照抄接口形状"的做法都会变成维护负担。
3. 平台化引力：三插件机制、Codex/ClaudeCode 子代理接入会诱使 OmniDesk 走向通用平台，与既定边界（AGENTS.md、不做 Registry）直接冲突。

## 建议
按 04 的 P1 两项进入小步实施（待用户批准）；P2 待设置页有真实需求再立项；P3 全部不借鉴。预计 P1 合计改动 <100 行 + 2 个测试用例，不动架构、不动 runtime/。

