# OD-ARCH-059 S3 · Obsidian 兼容（frontmatter + vault 指引）验收证据

**执行日期：** 2026-10-22
**证据等级：** 真实桌面窗口运行（一次性 Xvfb + 合成端点）+ Rust 单测（脚本/运行级）
**数据边界：** 一次性数据根 `/tmp/od059-s3-verify`；无凭证、无真实材料；未触碰 `runtime/`、`~/.dsh`、`~/.agents`、`~/OmniDesk`。

## 复现

```bash
python3 tests/verification/desktop/verify_vault_binding_059.py --display :97 --root /tmp/od059-s3-verify
bash /tmp/od054-reg.sh
```

`window_harness.py` 自本轮起把 `XDG_DATA_HOME` / `XDG_CONFIG_HOME` / `XDG_CACHE_HOME`
一并指向一次性根，WebKitGTK 的 localStorage 不再落到用户真实的
`~/.local/share/dev.omnidesk.discipline`。

## 结果

| 场景 | 可观察证据 | 判定 |
| --- | --- | --- |
| 首屏 → 设置页 | `01-home-first-run.png`、`02-settings-top.png` | 通过（截图） |
| 成果目录区 | `03-settings-artifacts-section.png` | 通过（截图） |
| 当前生效目录 + 来源 + vault 指引 + 边界 | `04-vault-guidance.png`：显示 `/tmp/od059-s3-verify/local-data`、来源「开发/测试隔离变量覆盖」，含「打开文件夹作为 vault」指引与「只写自己的成果、不读不改 vault 其他笔记」声明 | 通过（截图） |
| 成果档案页接入提示 | `06-scene-artifacts-hint.png`：工作记录页右栏含 Obsidian 接入说明，本场景成果为空（未出现用户真实成果名） | 通过（截图） |
| 新成果首行 frontmatter | `task_shell.rs` 4 个单测：离线保存格式、分析产物格式、危险标题转义、改绑后旧目录字节不变 | 通过（cargo test） |

## 全回归（regression.txt）

cargo test 194 passed（+2/13/3/1）、cargo build 通过（1 个既有 warning）、tsc 0 错误、
npm build 成功、pytest 26 passed、`git status --porcelain runtime/` 为空。

## 已知限制

- vault「改绑」只做了 Rust 单测级验证（新文件落新目录、旧目录字节不变），未在真实窗口里点选路径改绑。
- Obsidian 本体不安装、不检测、不拉起，接入指引是文案，没有自动化断言用户端真实打开效果。
- 设置页为路径输入框，不是原生目录选择器（054 已登记）。
