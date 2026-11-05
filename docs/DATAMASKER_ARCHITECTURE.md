# DataMasker 架构与技术设计（OD-ARCH-139 · 规划稿，待批准）

> 状态：规划完成，未实施。本文所有技术选型已定死，实施阶段（Task 1，OD-ARCH-140 候选）不再做架构选择。
> 数据口径：本文全部示例数据为虚构，任何真实干部姓名、证件号、卡号不得进入仓库。

## 1. 现状分析（实读结论）

1. Python 侧唯一包是 engine/omnidesk_engine/（10 模块，纯标准库，零第三方依赖）；Python 3.9 兼容为硬约束。任务书建议的 omniharness 包不存在，落地路径改为 engine/omnidesk_engine/security/。
2. 现有脱敏能力是单向打码：Rust profile_rules.rs::mask_privacy（18 位证件 / 17+X / 11 位手机号 → 星号，无映射无还原）；task_shell.rs::red_line_scan 是关键词命中即拒绝。双向可还原脱敏是全新能力，与现有实现不重复；但识别口径必须与两者对齐（同一证件/手机号定义），避免两条链路对同一段文本判定不一致。
3. 配置惯例：仓库 config/ 放样例；用户可改配置在 ~/OmniDesk/app/config/，命名 <name>.v1.json。词典循此例：仓库 config/sensitive-dict.sample.v1.json（全部虚构）；运行时读 <app_root>/config/sensitive-dict.v1.json。
4. 测试惯例：unittest，文件在 tests/verification/engine-interface/adapter/，命令 python3 -m unittest -v test_xxx。
5. LLM 调用链（只读不改）：宿主 lib.rs → 引擎 web_api_adapter.py → DSH Web API（session/prompt）。Task 4 的集成点：引擎出站前 mask、会话事件回程 unmask。

## 2. 核心设计

### 2.1 文件
- engine/omnidesk_engine/security/__init__.py —— 导出 DataMasker、SessionScope、MaskMapping
- engine/omnidesk_engine/security/masker.py —— 核心实现；import 限 re / secrets / hashlib（保留） / dataclasses / typing / json / pathlib
- config/sensitive-dict.sample.v1.json —— 样例词典（全部虚构）

### 2.2 词典（omnidesk.sensitive-dict.v1）
{"schema":"omnidesk.sensitive-dict.v1","entries":[{"type":"PERSON","value":"张三丰","aliases":["张三丰总"]},{"type":"BRANCH","value":"城东支行"}]}
- 类型 Task 1 固定：PERSON / BRANCH / ORG / CUSTOM；内部用注册表 dict(type→compile rule/词表) 预留扩展。
- 同 (value,type) 的 aliases 合并；重复 value+type 去重；非法 type 拒绝加载并报行号。
- 加载失败时 DataMasker 降级为纯规则模式并在 stats.warn 记录原因，不抛异常阻断调用方。

### 2.3 规则识别（正则 + 真实校验，不裸判长度）
| 类型 | 匹配 | 校验 |
| --- | --- | --- |
| IDCARD | 18 位（含 17+X） | GB11643 加权校验码验证，校验失败不算证件 |
| BANKCARD | 13–19 位连续数字（边界防粘连） | Luhn 校验 |
| PHONE | 1[3-9] 开头 11 位，前后数字边界 | 无附加 |
| EMAIL | 简化 RFC 式，≤254 字符 | 无附加 |

匹配优先级：IDCARD > BANKCARD > PHONE > EMAIL > 词典；实现为"全部定位 → (start,-length) 排序 → 先占长、重叠丢弃"，规则与词典统一排序，不存在"先到先得"。

### 2.4 占位符与映射生命周期
- 占位符：[TYPE_XXXXXXXX]，XXXXXXXX = secrets.token_hex(4).upper()（8 hex，32bit 空间）。同作用域内同实体查表复用；生成碰撞（同作用域内占位符串已存在）时重生成，上限 1000 次。
- Request Mapping：mask() 返回的 MaskMapping（dataclass，frozen）；生命周期由调用方持有，DataMasker 实例不保存。
- Session Mapping：SessionScope 由调用方创建并复用传入；同 scope 相同实体占位符一致；不同 scope 天然隔离。Task 1 仅内存态，不持久化。
- Local Dictionary：即 2.2 词典文件，是"识别依据"，不是映射表，不含原文映射关系。
- Identity Vault：明确不做。

### 2.5 同名/重叠
- 同名同 type = 同一实体同一占位符；同名不同 type = 各自占位符，互不干扰。
- 重叠按 (start,-length) 先占长；被丢弃的匹配计入 stats.overlap_dropped。

### 2.6 映射安全（构造性保证）
- MaskMapping 不提供任何导出原文的公开接口；__repr__ 固定为 "<MaskMapping n=N>"，不含原文与占位符对应关系。
- DataMasker 实例零映射留存：所有映射只在返回值/传入的 SessionScope 里，跨请求隔离由构造保证，不靠调用纪律。
- unmask 必须显式传 mapping；不传/传错时未知占位符原样保留并报告，绝不猜测。

### 2.7 还原与完整性
- 占位符整体匹配正则：\[(IDCARD|BANKCARD|PHONE|EMAIL|PERSON|BRANCH|ORG|CUSTOM)_[0-9A-F]{8}\]
- unmask(masked, mapping) → (text, report)；report = {restored, unknown_placeholders(列表), modified_candidates(形似占位符但查表失败的计数), ok}
- ok = unknown 为空。ok=False 时结果文本不得被上层当作"已还原内容"使用（此纪律写入 docstring，Task 4 落为强制拦截）。

### 2.8 Markdown / Obsidian 结构适配边界（Task 2 实施只定基调）
- Task 1 按纯文本处理，不识别任何结构。
- Task 2 基调：区域策略表（代码块/行内代码默认跳过、Frontmatter 默认处理、WikiLink 只处理显示文本不碰链接目标——目标脱敏会断链）；逐区域扫描；永久禁止"解析再序列化"。

## 3. 与现有能力的关系
- mask_privacy（Rust，单向打码）继续负责对话/落盘链路，不动。
- DataMasker（Python，双向还原）服务未来知识库/LLM 链路，两者识别口径对齐：证件 18 位含 X、手机 1[3-9]11 位，与 profile_rules.rs 一致。
- Task 4 接入后，"所有云端请求必经脱敏层"才成为可验收事实；Task 1 完成不等于云端调用已受保护。
