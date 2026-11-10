# OD-ARCH-147a 证据摘要：基础插件全局装配

日期：2026-11-10　执行：BUILDER 子代理（Pauli，OpenDesign/deepseek-v4.1-flash + high；会话已关闭未交最终报告，本摘要由 ARCHITECT 依据落盘 diff 复原并独立验证）

## 改动清单（git diff 实读核对）

| 文件 | 改动 |
| --- | --- |
| samples/profiles/discipline.profile.json | dependencies 加 dsh-free-search@0.9.5、dsh-cost-meter@1.9.6 |
| samples/profiles/omnidesk-base.profile.json | 同上 |
| samples/profiles/web.profile.json | 出厂已含两插件，git diff 确认未改动，正确 |
| samples/profiles/discipline.profile.patch.yml | 顶部新增 - insert 块：web-search-free（bing/zh-CN）+ cost-meter |
| samples/profiles/omnidesk-base.profile.patch.yml | 末尾新增同一 insert 块 |
| scripts/prepare-runtime-assets.sh | seed 循环加 discipline |
| desktop/src-tauri/src/profile_assets.rs | ARCHITECT 补齐（计划内）：DISCIPLINE_PLUGINS 加 dsh-free-search / dsh-cost-meter（version: None） |

runtime/dsh-home-seed/profiles/{omnidesk-base,web}/cordis.patch.yml 与 samples 同步（gitignore 范围，不进 git status）。

## ARCHITECT 独立验证（2026-11-10 实跑）

1. cargo test --offline --lib：1090 passed / 4 failed。4 项失败均与本次改动无关：every_tauri_command_runs_off_the_ui_thread（HEAD 既有 desktop_user_name 同步命令，git show HEAD:lib.rs:852 实证）；process_owner 3 项（本机沙箱进程表访问受限，非代码回归）。
2. tsc --noEmit：exit 0。
3. verify_diagnose_omnidesk.py：15/15。
4. insert 块落点核对：omnidesk-base.patch.yml:150-158、discipline.patch.yml:14-22，内容与 web 出厂 patch 的 web-search-free 条目一致。

## 遗留

- prepare-runtime-assets.sh 的 seed discipline 安装未实跑（需 npm 网络；出包前实跑即可）。
- verify_plugin_scope_104.py 6 项失败：3 项由本卡+147c 机制变更引入（sample_single_insert / sample_no_entry_override / sample_no_disabled，该脚本钉的是 OD-ARCH-104 时代「补丁只能加 insert」的纪律，147a/147c 已按用户批准的计划突破），3 项为环境性失败（asset:scripts / dsh_home_present 指向不存在路径）。脚本适配待用户裁定后另开小卡。

