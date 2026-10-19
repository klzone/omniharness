# OD-ARCH-011 证据索引：日期校验、成果落点与离线流程窗口验证

- 采集时间：2026-10-18 10:24–10:40（Asia/Shanghai）
- 采集者：BUILDER
- 对应计划：[PLANS.md](../../../../../PLANS.md) 的 **OD-ARCH-011**
- 工作区：`/mnt/data/Code/MyWorkStation`，分支 `main`，HEAD `5ffc710`
- 结论范围：**只证明准入关闭时的本地确定性流程、日期校验与保存行为**。模型撰写步骤受准入阻塞，未验证。

---

## 1. 场景与结果

| 场景 | 输入 | 实际输出 | 证据等级 |
| --- | --- | --- | --- |
| 非法日历日期（周记录） | `2026-02-30 \| x \| done` | 拒绝，提示「第 1 行：日期必须是存在的 YYYY-MM-DD（例如 2026-02-30 不存在）」 | 真实窗口运行 + Rust 运行 |
| 闰日 | `2024-02-29` | 接受（周记录、期限、判定日共用校验） | Rust 运行 |
| 非闰年 2 月 29 日 | `1900-02-29`、`2100-02-29` | 拒绝 | Rust 运行 |
| 月份天数 | `2026-04-31`、`2026-01-32`、`2026-00-10`、`2026-13-10` | 拒绝 | Rust 运行 |
| 逾期边界 | 期限 2026-10-14，判定日 2026-10-14 / 10-15 | 逾期 0 / 1 | Rust 运行 |
| 周记录 → 周报 | `samples/workflows/synthetic-weekly.txt` | 事项 4 / done 2 / pending 1 / blocked 1 | 真实窗口运行 + Rust 运行 |
| 一般文案（缺事实） | 目的与受众为合成值，事实留空 | 未使用模板；首节为「【待确认】事实依据缺失，暂不能形成初稿」 | Rust 运行 |
| 项目跟进 | `samples/workflows/synthetic-project.txt`，判定日 2026-10-15 | 事项 4 / 逾期 2 | Rust 运行 |
| 人工确认后保存 | 周报场景勾选确认并保存 | 写入仓库外 `/tmp/od011-tYouyK/local-data/offline-weekly-report-20261018-023000.md`，986 字节 | 真实窗口运行 |
| 再次保存 | 同一内容再点一次「保存成果」 | 生成新时间戳文件（`023020`、`023026`、`023036`、`023037`），**未覆盖**既有文件 | 真实窗口运行 |
| 旧 YvanAI 保护 | 临时 HOME 下预置 `~/YvanAI/projects/Omnidesk/analysis/existing-artifact.md` | 新成果落到 `~/OmniDesk/workspace/artifacts`；旧文件正文未变 | Rust 运行（单元测试） |
| 共享写入器 | 目标文件已存在 | 拒绝并保留原内容；新路径正常写入 | Rust 运行（单元测试） |
| 准入拒绝 | 直接调用宿主启动入口 | `ENDPOINT_ISOLATION_CONFIRMED = false`，拒绝且不建目录/进程/任务 | Rust 运行 |

三个离线场景在窗口内都显示 `模型步骤：受准入阻塞，未验证：本次未调用任何模型`，没有把模型步骤显示成已生成。

---

## 2. 窗口运行环境

| 项 | 值 |
| --- | --- |
| 显示器 | 一次性 `Xvfb :96`（1320×900×24），不占用用户桌面 |
| 窗口 | `OmniDesk Discipline`，正常渲染 |
| 前端 | `vite preview` on `127.0.0.1:5273`，加载 `desktop/dist` |
| 应用 | `desktop/src-tauri/target/debug/omnidesk-desktop` |
| 数据根 | 一次性 `/tmp/od011-tYouyK/local-data`，`HOME` 也指向临时目录 |
| 渲染规避 | `WEBKIT_DISABLE_DMABUF_RENDERER=1`（本机 WebKitGTK 环境差异，未进仓库） |
| 引擎状态 | 顶栏显示「引擎未就绪」；`ENDPOINT_ISOLATION_CONFIRMED = false` |

**注意：** 窗口内看到的新成果路径是 `/tmp/od011-tYouyK/local-data`，这是测试用 `OMNIDESK_LOCAL_DATA_ROOT` 覆盖的结果，不代表默认路径被改写。默认（未设置该变量）时新成果根为 `~/OmniDesk/workspace/artifacts`，由单元测试与证据生成器分别证明。

截图：

| 文件 | 内容 |
| --- | --- |
| `window-01-scope-notice.png` | 离线流程页的适用范围提示与保存落点说明 |
| `window-02-weekly-result.png` | 载入合成样例后的本地周报结果 |
| `window-03-saved.png` | 勾选人工确认后保存成功，显示实际路径与字节数 |
| `window-04-second-save-new-file.png` | 再次保存生成新文件，原因见下方说明 |
| `window-05-invalid-date-rejected.png` | 窗口内输入 2026-02-30 被拒绝 |

`offline-scenarios.txt` 是 `examples/evaluate_offline_scenarios` 的直接输出（脚本运行证据，不是窗口证据）。
`saved-offline-weekly-report.md` 是窗口内保存的成果副本，用于核对内容；原始文件留在一次性 `/tmp` 目录，未进入仓库。

### 关于「第二次保存没有覆盖」

离线成果的文件名带秒级 UTC 时间戳（`offline-<scenario>-YYYYMMDD-HHMMSS.md`）。同一秒内重复保存会命中同名文件并报「已存在，不覆盖」；跨越秒后会写成新文件。窗口内实测产生了 5 个独立文件，每个 986 字节，没有一个被覆盖。

「同名即拒绝」这一条由 `create_new(true)` 与单元测试 `offline_save_requires_confirmation_and_never_overwrites`、`shared_writer_never_clobbers_an_existing_file` 直接证明，不依赖窗口计时。

---

## 3. 本轮实现改动

- `parse_date` 按月天数与公历闰年规则校验；周记录、项目期限、逾期判定日共用。
- 新成果默认根从旧的 `~/YvanAI/projects/Omnidesk/analysis` 改为 `~/OmniDesk/workspace/artifacts`；旧文件不读取、不迁移、不改名。
- 分析成果与离线成果共用 `write_new_file_durable()`：`create_new` 防覆盖，写入或刷盘失败时删除本次半成品。此前离线路径缺少这层清理，本轮统一。
- 离线流程页新增适用范围提示、实际保存目录、日期真实性提示。

---

## 4. 隔离与边界核对

- `git diff -- runtime/` 输出 0 行；未修改 DSH Runtime。
- `~/.dsh` 与 `~/.agents` 的 mtime 在执行前后一致（`2026-10-15 20:17:43`、`2026-09-23 15:12:11`），未读取其内容。
- 窗口运行使用一次性 `HOME`、独立 `DSH_HOME`/`DSH_AGENTS_HOME`/应用数据根；引擎未启动。
- 未读取、打印或复制任何凭证；未调用任何模型。
- 未提交、未推送、未打标签。

---

## 5. 未验证与限制

- **模型撰写完全未验证：** 准入仍关闭，本阶段没有任何真实模型输出，不得据此声称生成质量或业务收益。
- **用途未获准：** 周报、一般写作、项目跟进在 `task_types()` 中仍为 `reserved`，不构成正式能力或用途授权。
- 仅验证 Linux + Xvfb；跨平台未验证。
- 窗口操作经 XTEST 注入完成，属真实窗口运行，但不是人工手工点选。
- 旧 YvanAI 保护由临时 HOME 单元测试证明；没有读取或改动真实旧目录中的任何内容。
