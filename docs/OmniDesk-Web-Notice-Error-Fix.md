# Web 界面“暂时无法保存确认状态，请重试。”问题定位与修复

日期：2026-10-15
现象：打开 DSH Web UI 工作台时，界面提示“暂时无法保存确认状态，请重试。”，重试无效。

## 一、结论

这条提示来自 DSH 的“预览版说明”确认机制，不是会话丢失，也不是数据损坏。

机制是：DSH 在一个设置命名空间的 `welcomeNoticeVersion` 字段里记录用户确认过的版本号，并按严格相等与当前版本期望值比较。不相等时反复弹出预览版说明；如果确认写入本身被拒绝，就弹出这条错误。

本机出现这条错误的关键原因是版本错位：

- 你原版日常环境 `~/.dsh/profiles/desktop/cordis.patch.yml` 第 7 行仍钉住旧值 `2026-08-13.1`。
- 当前官方 0.2.1-alpha.2 客户端期望的值是 `2026-09-28.1`。
- 该文件里同时禁用了设置面板（`ui-settings: enabled: false`），所以在常规设置界面里也清不掉这个旧值，确认会反复失败。

## 二、源码依据（已证实）

上游 0.2.1-alpha.2 源码：

- 文案：`packages/client/ui-settings-models/src/client/locales.ts` 第 111 行（英文）与第 231 行（中文“暂时无法保存确认状态，请重试。”）。
- 期望值与命名空间：`packages/client/ui-settings-models/src/onboarding-copy.ts` 第 2 行命名空间 `ui-settings-general`，第 11 行 `WELCOME_NOTICE_VERSION = '2026-09-28.1'`。
- 判定：`welcome-store.ts` 第 128 行按严格相等比较。
- 该文案只在预览版说明组件里、当 `state.error !== null` 时渲染；DSH 其他位置不会产生同一字符串，所以这条提示唯一对应确认写入失败。

本机已安装包核对：

- 官方运行时 `/opt/DeepSeek Harness/.../dsh-client-ui-settings-models` 版本 `0.2.1-alpha.2`：命名空间 `ui-settings-general`，期望值 `2026-09-28.1`。
- 单独提取的官方 CLI 树 `~/dsh-asar/dsh/node_modules/@deepseek-ai/...` 同为 `0.2.1-alpha.2`。两者都含该英文与中文文案，确认这条提示来自官方客户端代码。

## 三、3080 页面属于哪套外壳（已证实）

你看到的 `http://127.0.0.1:3080/` 页面来自社区桌面外壳，不是官方 0.2.1：

- 社区启动器日志 `~/.local/share/io.github.hairyf.deepseek-harness-desktop/logs/dsh-web.log` 明确记录 `dsh web: http://127.0.0.1:3080`。
- 该外壳 `~/Code/DSH-Desktop/dsh-plugin-desktop` 版本 `2.0.11`，其自带客户端包 `dsh-client-ui-settings-models` 为 `0.1.5-rc.2`：
  - 命名空间：`ui-onboarding`（旧）
  - 期望值：`2026-08-13.1`（旧）
- 官方 0.2.1-alpha.2 用的是 `ui-settings-general` 与 `2026-09-28.1`（新）。

也就是说，你日常那套 3080 页面是“旧社区外壳 + 旧命名空间 + 旧期望值”，与你 profile 里钉住的旧值恰好同源；而当前官方运行时用的是新命名空间与新期望值。

隔离实例实测：请求 `ui-onboarding` 命名空间会得到 `No configurable plugin entry "ui-onboarding"`，说明该命名空间在当前运行时里根本不存在。因此任何仍按旧命名空间保存/读取确认状态的客户端，都无法把确认落到当前运行时，表现为“确认失败、重试无效”。

补充：3080 端口当前没有任何进程监听（连接返回 000），浏览器里标题为 “DeepSeek Harness” 的 3080 页面是之前一次已关闭实例留下的旧标签页。3080 是 Web bundle 的默认端口；当前实际在运行的是隔离实例的 3210/3300 页面。

## 四、修复

OmniDesk 仓库层（已修复，未触碰原环境）：把以下三处统一为当前期望值 `2026-09-28.1`：

- `profiles/yvan-work/cordis.patch.yml`
- `profiles/desktop/cordis.patch.yml`
- `workstations/discipline/profile.yaml`

隔离实例验证（在 `~/OmniDesk/runtime/dsh-home` 与临时根上实测）：

- `settings/describe` 返回 `ui-settings-general` 的 `welcomeNoticeVersion = 2026-09-28.1`。
- 用匹配的 `expectedRevision` 调用 `settings/mutate` 返回 `ok:true`，写入落盘到隔离根的 `profiles/yvan-work/cordis.patch.yml`。
- 把隔离根该值改回旧值复现时：`describe` 显示 `2026-08-13.1`，与期望值不等——这正是会反复弹窗、确认不通过的状态。改回 `2026-09-28.1` 后恢复正常。

原版日常环境（`~/.dsh/profiles/desktop/cordis.patch.yml`）仍是旧值 `2026-08-13.1`。若要消除日常环境同一条报错，只需把该文件第 7 行改为 `2026-09-28.1`。这属于对现有 DSH 数据的修改，需你确认后再执行。

## 五、已知残留

隔离实例实测发现 `ui-settings-general` 在启动后不总是稳定出现在 `settings/describe` 的命名空间列表里（同一次进程运行中重复请求会时有时无）。这属于官方运行时自身的时序/加载问题，不是隔离方案引入的；隔离方案只负责保证数据根独立。若后续出现确认异常，可先按“确认值是否为 2026-09-28.1、命名空间是否被服务”两步排查。

