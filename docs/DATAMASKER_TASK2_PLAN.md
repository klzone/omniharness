# DataMasker Task 2 实施任务卡（OD-ARCH-140 · 待批准后交实施）

> 前置：Task 1（OD-ARCH-139）已收口，masker.py 纯文本引擎可用。
> 测试文件：tests/verification/engine-interface/adapter/test_security_masker_markdown_unit.py（unittest，全虚构数据）。
> 运行：cd tests/verification/engine-interface/adapter && python3 -m unittest -v test_security_masker_markdown_unit -k <Key>
> 红线：只改 engine/omnidesk_engine/security/masker.py（或新增 markdown.py 挂同包）与测试文件；零 Rust 改动；零第三方依赖；禁止解析再序列化。

## 区域策略表（实现依据，不得自行调整）

| 区域 | 策略 |
| --- | --- |
| YAML Frontmatter（文件首 --- 到 ---） | 脱敏 |
| 围栏代码块（三反引号）与行内代码（单反引号） | 跳过 |
| WikiLink 目标（双括号）与嵌入（叹号双括号） | 跳过整段 |
| WikiLink 显示文本（目标|显示） | 仅显示文本脱敏，目标与竖线保留 |
| 标签（#tag） | 跳过 |
| URL（http/https） | 跳过 |
| 其余正文 | 脱敏 |

## S1 区域扫描器（-k Region）
- 动作：scan_markdown_regions(text) 返回有序区间列表 [(start, end, kind)]，kind 属于 {frontmatter, code, inline_code, wikilink_target, wikilink_display, text}。
- 断言要点：三种 WikiLink 形态区分正确；行内代码与围栏代码块区分；frontmatter 只认文件起始且闭合（未闭合则整文件按纯文本处理并 warn）；区间互不重叠且并集覆盖全文；空串/纯代码文档退化正确。最小断言数 12。
- 前置：无。

## S2 mask_markdown 主路径（-k MaskMd）
- 动作：只对 frontmatter / wikilink_display / text 区间调用 Task 1 引擎，其余区间原样保留。
- 断言要点：输出与输入相比除占位符外逐字节一致（含空行、缩进、表格管道、注释）；代码块内证件号不替换；frontmatter 内人名替换；显示文本替换但链接目标保留；同名实体跨区域（正文与 frontmatter）使用同一占位符；session 参数透传。最小断言数 10。
- 前置：S1。

## S3 unmask_markdown（-k UnmaskMd）
- 动作：对全篇占位符还原（占位符只可能出现在脱敏区，但还原按全文扫描，容忍模型挪动占位符位置）。
- 断言要点：mask 再 unmask 往返逐字节一致；跨映射时 ok=False；被修改占位符保留原样；report 字段与 Task 1 一致。最小断言数 8。
- 前置：S2。

## S4 畸形结构与性能（-k Edge）
- 断言要点：未闭合围栏代码块之后整段按 code 跳过并 warn；未闭合行内码按单行处理；1000 个 WikiLink 的文档正常完成；1MB 文档 5 秒内完成（time.monotonic 断言，防偶发超时）；嵌套反引号不崩溃。最小断言数 7。
- 前置：S2。

## S5 vault_write 对齐确认（文档步骤）
- 动作：实读 vault_write.rs 写入白名单，确认与 DataMasker 输出无冲突；结论写入 STATUS.md（无需代码）。
- 前置：S2。

## S6 收口
- 命令：python3 -m unittest -v test_security_masker_markdown_unit 全量，且 python3 -m unittest -v test_security_masker_unit（Task 1 回归 32 项）。
- 断言要点：两条命令退出码 0；STATUS.md 更新；停下待 ARCHITECT 复核。
- 前置：S1 到 S5。

## 验收红线
- 全部测试数据虚构；测试中零 print（用断言验证）；不得修改 Task 1 已通过的纯文本路径行为（mask/unmask 现有 32 项测试必须原样通过）。
