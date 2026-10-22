# OD-ARCH-059 S2 · 对话中枢归一验收证据

**执行日期：** 2026-10-22
**证据等级：** 真实桌面窗口运行（一次性 Xvfb + 合成端点）+ 文件事实（线程记录 JSON）+ 静态代码检查
**数据边界：** 一次性数据根 /tmp/od059-s2-verify；对话内容为合成例句；无凭证、无真实材料。未触碰 runtime/、~/.dsh、~/.agents、~/OmniDesk。

## 复现

```bash
python3 tests/verification/desktop/verify_conversation_center_059.py --display :97 --root /tmp/od059-s2-verify
bash /tmp/od054-reg.sh   # 夜间批次同一套全回归
```

## 结果

| 场景 | 操作 | 可观察证据 | 判定 |
| --- | --- | --- | --- |
| 首页对话 chip | 全新数据根启动，截图输入区 | `01-home-chips.png`：场景 chip「工作记录」+ 对话 chip + 「新对话」；Dock 第二项为「工作记录」 | 通过（截图） |
| 载入既有线程 | 预置 `app-data/threads/_unassigned/t059s2verify.json`（tag=weekly，2 条合成消息）后重启 | `02-home-loaded-thread.png`：对话 chip 显示「[工作记录] 合成验收对话」，两条消息完整显示 | 通过（截图 + 文件） |
| 场景页纯档案化 | Dock 第二项进入工作记录页 | `03-scene-archive.png`：左栏「工作记录 · 历史对话」列出该对话，右栏成果档案；页面无输入框/对话框 | 通过（截图） |
| 去对话续聊 | 点左栏对话标题 | `04-open-thread-home.png`：回到首页并载入该对话，历史完整 | 通过（截图） |
| 场景改挂 | 首页场景 chip 展开 → Home → 两次 Down → Return | 落盘 JSON：`tag` 由 `weekly` 变 `writing`，`messages` 仍为 2 条 | 通过（文件事实） |
| 声明联动 | 声明副本删除 writing 条目后重启，同样 Home + 两次 Down | `06-chip-without-writing.png`：下拉只剩「自由对话（不归类）/ 工作记录 / 项目跟进」；落盘 tag 变成 `project`（证明 writing 选项确已移除） | 通过（截图 + 文件事实） |
| 声明复原 | 运行结束写回默认声明 | `facts.json` 的 `decl_restored=true` | 通过（文件事实） |

## 全回归（regression.txt）

- cargo test：主库 190 passed，其余测试目标 2/13/3/1 passed，0 failed
- cargo build：通过（1 个既有 dead_code warning）
- tsc --noEmit：0 错误
- npm run build：成功（约 500ms）
- pytest：26 passed
- `git status --porcelain runtime/`：空

## 已知限制

- chip 的键盘步进在 XTEST 下不可从「当前项」直接推算（首次 Down 的落点受展开前选中项影响），脚本统一用 Home 归位后再计数，保证可复现；这属测试驱动方式，不是产品行为缺陷。
- 「成果落 weekly_report 目录」由静态链路核对：`changeTag → retagThread(tag=weekly)` → `ScenePage` 按 `def.scenario="weekly_report"` 过滤，本轮未再跑一次保存成果全流程（032/033 已有覆盖）。
- 场景页截图无对话框属人工目视判定，未做 DOM 断言。
