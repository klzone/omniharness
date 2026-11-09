# OD-SYS-702 执行证据

日期：2026-11-09。基线 HEAD：`f4422b1da7`（任务开始时工作区干净；执行中 OD-SYS-701 并行改动出现在同一工作区，本卡未触碰其文件）。

## 依赖清单实际位置

- `/Applications/DeepSeek Harness.app/Contents/Resources/runtime/primary-runtime/runtime.json` 的 `pythonPackages` 是 DSH 官方桌面安装产物内的清单，含 pandas/python-docx/openpyxl 等；它由 DSH 上游构建器 `prepare:primary-runtime` 生成，**不在本仓库**。
- 本仓库的子模块 `runtime/deepseek-harness` 为空；`scripts/`、CI、tauri 配置中没有任何 `pip install` 或 Python requirements 清单，OmniDesk 也不打包 primary-runtime。
- 因此新增仓库侧清单 `scripts/python-requirements.txt`（requests + certifi），并在 `scripts/prepare-runtime-assets.sh` 增加“随包 primary-runtime 存在时用其 pip 补装”的步骤；没有 payload 时提示当前使用系统 Python，由环境检查给出 pip 命令。

## 改动文件

- `desktop/src-tauri/src/platform.rs`：`python_modules` 探测（`-c "import requests"`）、`DependencyStatus{name:"requests", requirement:"可导入"}`、`EnvironmentReport.ok` 纳入该字段、5 项单测。
- `desktop/src/shell/EnvironmentCheck.tsx`：新增 Python 模块行与缺失状态/提示。
- `desktop/src/types.ts`：`EnvironmentDependency` / `EnvironmentReport` 形状同步。
- `skills/anysearch/SKILL.md`：依赖说明。
- `scripts/python-requirements.txt`：仓库侧依赖清单。
- `scripts/prepare-runtime-assets.sh`：打包补装步骤。
- `tests/verification/desktop/verify_sys_deps_702.py`：本卡静态验收。

## 实跑命令与结果（2026-11-09）

1. `export PATH="$HOME/.cargo/bin:$PATH" && cd desktop/src-tauri && RUST_TEST_THREADS=4 cargo test --offline --lib`
   - 结果：`1093 passed; 0 failed; 0 ignored`，exit 0。日志 `cargo-lib.log`。
   - platform.rs 的 `#[test]`：HEAD 9 项 → 现在 14 项，净增 5 项；新增用例均出现在日志中。
2. `cd desktop && npx tsc --noEmit`：exit 0。日志 `tsc.log`。
3. `cd desktop && npm --prefix . run build`：exit 0（仅既有 >500 kB chunk 提示）。日志 `build.log`。
4. `python3 tests/verification/desktop/verify_sys_deps_702.py`：13/13 通过。日志 `verify.log`，JSON `verify_sys_deps_702.json`。
5. `bash -n scripts/prepare-runtime-assets.sh`：通过。日志 `packaging-syntax.log`。

## 已知限制

- 本仓库不生成也不打包 primary-runtime，OmniDesk 引擎使用系统 Python。补装步骤仅在随包 payload 存在时实际执行；没有 payload 时依赖环境检查提示用户对引擎所用 Python 执行 `pip install requests certifi`。
- `EnvironmentReport.ok` 纳入 `python_modules`：requests 缺失时环境检查页会接管主区域并显式提示；引擎进程本身不读取该报告，仍可独立启动。
