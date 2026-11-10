# OD-DOCX-148a 附件广收窄取证据

日期：2026-11-10

- `RUST_TEST_THREADS=4 cargo test --offline --lib`：1096 passed / 0 failed（新增 docx 提取、二进制元数据降级、超上限拒绝 3 项）。
- `npx tsc --noEmit`：exit 0。
- `npm --prefix desktop run build`：exit 0。
- `verify_docx_attachment_148.py`：14/14（含合成 docx helper 实跑）；JSON 见同目录。

说明：helper 由 `task_shell.rs` 的 `include_str!` 内置；docx 用 stdlib，PDF 仅在 pypdf 可导入时提取，其余格式带路径与 sha256 降级。超单文件上限仍拒绝。

## 148c / 148d

- `scripts/python-requirements.txt` 增加 python-docx 1.1+ 与 lxml；`prepare-runtime-assets.sh` 内置到 `runtime/python-vendor`，仅对 charset_normalizer 删除架构相关扩展，保留 lxml C 扩展。
- prepare 实跑后：`PYTHONPATH=runtime/python-vendor python3 -c "import docx"` exit 0（lxml 6.1.3）。
- `verify_docx_vendor_148cd.py`：9/9；证据 JSON 同目录。
- 环境检查新增 python-docx / pypdf 两项，缺失只提示，不参与整体 `ok`。

已知限制：lxml C 扩展的 ABI 与构建机 Python 版本绑定。若用户引擎 Python 与构建机版本不同，python-docx 可能无法导入；环境检查会如实显示该项缺失，148a 的附件解析不依赖 python-docx（docx 正文走 stdlib）。
