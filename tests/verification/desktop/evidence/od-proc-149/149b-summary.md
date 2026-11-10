# OD-PROC-149b macOS 进程标题（node --require shim + python setproctitle）

- 日期：2026-11-11
- HEAD：f3f21e68c620dee70619b3a935c53ead11b7a154
- 依据：PLANS.md 顶部 OD-PROC-149 小节（2026-11-10 用户批准）
- 状态：已实施，待 ARCHITECT 独立复核；未提交、未推送、未打标签

## 改动文件

| 文件 | 改动 |
| --- | --- |
| `scripts/proc_title_shim.cjs`（新增） | `process.title = "omnidesk-dsh"`；注释说明仅在 macOS/Linux 的 ps/活动监视器生效，Windows 任务管理器忽略 |
| `engine/omnidesk_engine/acp_adapter.py` | 新增 `proc_title_shim_args()`：仅 `sys.platform == "darwin"` 返回 `["--require", <shim 绝对路径>]`；查找顺序为引擎包上一级 `scripts/`（开发态仓库根）与引擎包自身 `scripts/`（打包态同级布局），都找不到返回空列表；`AcpEngineAdapter.for_dsh` 的 cmd 改为 `[node, *shim_args, dsh_bin, --profile, ...]` |
| `engine/omnidesk_engine/web_api_adapter.py` | `from .acp_adapter import proc_title_shim_args`（该模块原本已从 acp_adapter 导入工具函数），`WebApiAdapter.for_dsh` 同样插入 shim 参数，单一实现避免两处漂移 |
| `engine/omnidesk_engine/__main__.py` | 在 `from .serve import main` 之前执行 `setproctitle.setproctitle("omnidesk-engine")`，`except Exception: pass` |
| `scripts/python-requirements.txt` | 加 `setproctitle>=1.3`，注释标 OD-PROC-149b |
| `tests/verification/desktop/verify_proc_alias_149.py`（新增） | 分节脚本：149a 段留空由 149a 追加；149b 段断言 shim 内容、helper 的平台分支、两处 for_dsh 实跑命令形态、process_owner 识别、`__main__` 的 AST 位置与异常包裹、依赖清单、node 实跑探针 |

未改：`runtime/` 内 DSH（bin.js 与 node_modules 未触碰）、`desktop/` Rust 代码、模型路由与凭证、`process_owner.py`。

## 联动核对（开工前只读结论）

`process_owner.is_engine_cmdline` 的判定是 `exe.startswith("node") and any(arg.endswith("bin.js") for arg in args[1:])`。插入 `--require shim` 后 cmd 为 `[node, --require, shim, bin.js, ...]`，`any(...)` 遍历全部参数仍命中 `bin.js`，且 exe 仍是 `node`。验证脚本用真实构造的 cmd 调用了该函数，两个适配器均 PASS。**因此不需要改动 process_owner.py。**

## 实际运行

| 命令 | 结果 |
| --- | --- |
| `python3 tests/verification/desktop/verify_proc_alias_149.py` | **18/18 PASS**，0 FAIL，0 SKIP（darwin，PATH 内 node v26.13.0） |
| `cd engine && PYTHONPYCACHEPREFIX=/tmp/od-pyc-149b python3 -m compileall -q omnidesk_engine` | exit 0，0 error |
| `python3 tests/verification/desktop/verify_diagnose_omnidesk.py` | **15/15**，不回归 |
| `node --require scripts/proc_title_shim.cjs -e "console.log(process.title)"` | 输出 `omnidesk-dsh`（脚本内探针，rc=0） |
| `pip install --target /tmp/od149b-deps setproctitle` 后 `runpy` 加载 `omnidesk_engine/__main__.py` | `getproctitle() == "omnidesk-engine"`；未装 setproctitle 时同一加载路径无异常 |

说明：`compileall` 首次直接运行报 4 个 `PermissionError`，原因是本机 Python 把字节码缓存写到 `~/Library/Caches/com.apple.python/...`（沙箱外），并非语法错误；设 `PYTHONPYCACHEPREFIX=/tmp/...` 后 exit 0，另用 `ast.parse` 全量解析亦通过。

## 需 ARCHITECT 裁定的计划外发现

1. **既有单测的位置断言失效**：`tests/verification/engine-interface/adapter/test_web_api_unit.py:464` 的 `test_for_dsh_argument_order` 断言 `cmd[:4] == [node, bin.js, "--profile", "web"]`。darwin 上插入 `--require` 后该断言失败（实测 `Ran 89 tests, FAILED (failures=1)`，其余 88 项通过）。该文件不在获批改动范围，未修改。建议改为从 `cmd` 中剔除 `--require <shim>` 前缀后断言，或断言 `bin.js` 与 `--profile` 的相对顺序。
2. **macOS 发布包不含 scripts/**：`desktop/src-tauri/tauri.macos.conf.json` 的 bundle resources 只有 engine/、runtime/；实查 `desktop/src-tauri/target/release/bundle/macos/OmniDesk.app/Contents/Resources` 只有 `engine`、`icon.icns`、`runtime`。打包后 `proc_title_shim_args()` 的两处候选（`Resources/scripts`、`Resources/engine/scripts`）都不存在，会返回空列表，DSH 标题保持默认——**149b 在 .app 发布形态下不生效**。需要决定：把 `scripts/proc_title_shim.cjs`（或整个 scripts/）加入 macOS bundle resources，或把 shim 放到 `engine/` 内随包分发。Windows 免安装包会整目录复制 scripts/，但因为只对 darwin 注入，不受影响。

## 未验证项

- Activity Monitor 中真实显示 `omnidesk-engine` / `omnidesk-dsh`（用户实测项；本机沙箱禁止 `ps`，只能用 `getproctitle()` 与 node 自报标题作为间接证据）。
- 打包后 .app 内的路径解析（受上一条发现 2 阻塞，当前预期不注入）。
- setproctitle 在 Windows/多架构 macOS 构建机上的 wheel 可用性：本机 Python 3.9 无预编译 wheel，pip 走源码编译成功；发布包按构建机 Python 安装，属现有 lxml 同类 ABI 约束，import 失败时已有 try/except 兜底。

## 复跑方式

```bash
python3 tests/verification/desktop/verify_proc_alias_149.py
cd engine && PYTHONPYCACHEPREFIX=/tmp/od-pyc-149b python3 -m compileall -q omnidesk_engine
python3 tests/verification/desktop/verify_diagnose_omnidesk.py
```
