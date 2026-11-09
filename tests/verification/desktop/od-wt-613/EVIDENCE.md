# OD-WT-613 证据

- 基线 HEAD：`821b1a4cf3024f4de533d2e0227ad51b3d0fc707`（工作区已有 601–612 的未提交改动，本会话原样保留）。
- 基线复核：`export_engine.rs` 已写完导出主体，但当时**没有 `#[cfg(test)]` 测试模块**；`cargo test --offline --lib` 为 1045 passed / 0 failed。上一会话在测试、前端封装与验收材料之前中断。
- `cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib`：退出码 0，**1070 passed / 0 failed**（净增 25 项 export_engine 测试，cargo-lib.log）。
- `cd desktop && npx tsc --noEmit`：退出码 0，无诊断输出（tsc.log）。
- `cd desktop && npm --prefix . run build`：退出码 0（build.log，仅有既有 >500kB chunk 提示）。
- `python3 tests/verification/desktop/verify_writing_export_613.py --json .../verify_writing_export_613.json`：**29/29**，退出码 0（verify.log）。
- `python3 tests/verification/desktop/verify_writing_review_612.py --json .../verify_writing_review_612.json`：**28/28**，退出码 0（verify_612.log；原 `not_mounted` 断言已改为组件已挂载）。

证据等级：源码事实 + Rust 模块测试 + TypeScript 编译 + Vite 构建 + 静态验收脚本。未开真实窗口、未调用模型、未读取用户数据。

## 本次核对到的实现要点

- MD 三态：正式稿剥离 frontmatter、内部编号、来源标记与 AI 调试信息；审阅稿逐段加来源标记并在文末列明细；工作稿追加核验统计与逐条摘要。
- 导出前过当前 Profile 的敏感词扫描，命中即拒绝且错误信息不回显原文；成功导出追加 manifest `export_receipts`。
- DOCX 双路径：python-docx 成功记 `office-docx`，失败如实记 `textutil` 且 `simplified=true`；PDF 走 soffice headless，超时/失败转打印流并校验 `%PDF-` 签名。
- 公众号 HTML 由宿主确定性渲染并转义；模板拒绝 `<script>`、`<iframe>`、`javascript:` 与远程资源。
- 样式包读取应用数据根 `config/writing-export-styles/` 的可编辑副本，缺省使用编译期嵌入的三份仓内样式。
- 状态迁移按 601 合法表，待审核→已定稿先写 `final` 正文快照；citations/factcheck 查询支持类型、章节、段落、结论与处理状态过滤。
- 前端补齐 `writingStatusTransition` / `writingRecordsQuery` / `writingExport` 三条封装与类型；任务详情新增大纲/正文/核验/版本四个页签，定稿按钮接状态迁移 IPC。

## 界面参考（OD-ARCH-035）

Craft 的块式文档画布与安静工具条适合承载大纲/正文编辑；Dovetail 的左侧分类与文档主面提示把核验和版本作为任务详情的次级页签，保持低 chrome、不再叠加一层导航。

## 未验证与已知限制

- 真实 Word / LibreOffice / textutil 的格式兼容性未在本轮验证；DOCX 双路径与 PDF 超时用可控假可执行文件验证回执与错误语义，真实工具产物只做结构前提。
- 真实窗口目视未做；本轮只验证 TypeScript 编译、构建与组件挂载点。
- 导出验收脚本为静态断言；真实导出路径由 25 项 Rust 测试中的临时目录集成用例覆盖。
- 测试数据全部虚构，无真实个人号码、姓名或业务材料。
