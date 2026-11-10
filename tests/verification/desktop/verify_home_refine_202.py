#!/usr/bin/env python3
"""OD-UI-202 首页细节修订（2026-11-09）静态核对。

证据等级：静态源码检查；不读用户数据、不调用模型。
用法：python3 tests/verification/desktop/verify_home_refine_202.py
"""
from __future__ import annotations

from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
SRC = REPO / "desktop" / "src"
PUBLIC = REPO / "desktop" / "public"

CHECKS: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str = "") -> None:
    CHECKS.append((name, bool(ok), detail))


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def main() -> int:
    theme = read(SRC / "theme.css")
    canvas = read(SRC / "shell" / "HomeCanvas.tsx")
    chat = read(SRC / "shell" / "HomeChat.tsx")
    category = read(SRC / "shell" / "CategoryPicker.tsx")
    model = read(SRC / "shell" / "ModelPicker.tsx")
    top = read(SRC / "shell" / "TopNav.tsx")
    app = read(SRC / "App.tsx")
    api = read(SRC / "api.ts")
    calendar = read(SRC / "shell" / "ThreeWeekCalendar.tsx")

    check("welcome_text_removed", "欢迎回到" not in canvas and "font-slogan" not in canvas,
          "欢迎回到字样已移除，不再占用问候区")
    check("welcome_greeting_name",
          "home-greeting" in canvas and "desktopUserName" in canvas,
          "问候语按时段显示，并带上系统用户名")
    check("welcome_slogan_bold", "font-weight: 700" in theme,
          "行楷标语使用 700 加粗")
    check("welcome_slogan_self_hosted",
          'font-family: "Slidexiaxing"' in theme
          and (PUBLIC / "fonts" / "Slidexiaxing-Regular.ttf").is_file()
          and (PUBLIC / "fonts" / "OFL-SlideFont.txt").is_file(),
          "演示夏行楷与 OFL 授权随包自托管")
    check("category_description_removed",
          "分类可在设置里改名或新增" not in category,
          "分类下拉的说明句已移除")
    check("model_effort_description_removed",
          "思考等级是请求参数" not in model,
          "思考等级说明句已移除")
    check("follow_dsh_removed",
          "跟随 DSH 设置" not in model and "onFollowDsh" not in model,
          "模型下拉的跟随 DSH 设置入口与功能已移除")
    check("model_label_clear",
          "font-body-sm text-[12px] font-medium" in model,
          "模型显示改用 12px Inter 中等字重，提高清晰度")
    check("model_popover_narrow",
          "w-[248px]" in model,
          "模型下拉宽度收窄到 248px")
    check("category_menu_compact",
          "Math.min(176" in category and "max-w-[80px]" in category,
          "归类菜单与触发胶囊采用紧凑宽度")
    check("model_catalog_description_removed",
          "目录来自 DSH 本地 API" not in model
          and "切换会重启引擎" not in model,
          "模型目录与重启说明已移除")
    check("calendar_iso_week_label",
          "isoWeekLabel" in top and "年第" in top,
          "顶部周次标题改为全年第几周")
    check("calendar_description_removed",
          "任务与日程数据源尚未接入" not in calendar,
          "日历底部说明句已移除")
    check("profile_description_removed",
          "切换后立即生效" not in top
          and "当前生效：" not in top,
          "Profile 下拉的两条状态说明已移除")
    check("profile_popover_narrow",
          "w-max min-w-[190px] max-w-[250px]" in top,
          "Profile 弹层按内容收窄")
    check("inner_focus_ring_removed",
          ".neu-inset-field input:focus" in theme
          and ".neu-composer textarea:focus" in theme
          and "outline: none !important" in theme,
          "搜索与对话内部输入不再画第二道蓝线")
    focus_block = theme.split(".neu-inset-field:focus-within", 1)[1].split("}", 1)[0] if ".neu-inset-field:focus-within" in theme else ""
    check("search_outer_ring_removed",
          "0 0 0 2px" not in focus_block,
          "搜索胶囊外层蓝色焦点环也已移除")
    check("effort_buttons_raised",
          "neu-effort-btn" in model
          and "neu-effort-active" in model
          and ".neu-effort-btn" in theme
          and ".neu-effort-active" in theme,
          "思考等级改为凸起按钮组，选中态实心品牌靛")
    check("empty_slogan_present",
          "坚持学习人生的有无限可能" in chat
          and "font-slogan" in chat,
          "新对话空态增加行楷标语")
    check("empty_slogan_brand_color",
          "home-slogan-text" in chat and "home-rule" in chat and "home-slogan-text" in theme,
          "空态标语两侧带分隔线，颜色走靛蓝灰令牌")
    check("chat_jump_arrow",
          "scrollToLatest" in chat and "chat-jump-latest" in chat,
          "对话滚动区增加跳到最新箭头")
    check("chat_live_state_promotion",
          "liveEventAtRef" in chat and "markLiveTask" in chat,
          "收到真实事件时把 starting 快照提升为 running")
    check("chat_stale_status_guard",
          "const lastLive" in chat and "30_000" in chat,
          "事件流仍在时抑制状态查询失败的误报")
    check("chat_no_message_autoscroll",
          "el.scrollTop = el.scrollHeight" in chat
          and "}, [threadKey]);" in chat,
          "自动滚动只在切换线程时发生，生成中不再强制滚到底")
    check("chat_busy_placeholder",
          'busy' in chat and '"正在生成…"' in chat,
          "生成中占位改显示正在生成，不再误报权限未开启")
    check("chat_stuck_stop",
          "chat-stop-stuck-task" in chat and "停止本轮" in chat,
          "长时间无工具返回时提供停止本轮入口")
    check("retry_recover_task",
          "recoverTask(taskId)" in chat,
          "重新查询先尝试重建宿主任务状态")
    check("effort_advisory",
          "effort-advisory" in model and "可能被忽略" in model,
          "模型未声明档位时显示思考等级可能失效的提示")
    check("assistant_avatar_model_initial",
          "modelInitial" in chat
          and "m.local ? \"L\" : modelInitial" in chat,
          "AI 回复头像改为模型首字母")
    check("user_avatar_brandmark",
          'BrandMark className="w-[16px] h-[16px]"' in chat,
          "用户提问头像改为 OmniDesk 小 logo")

    # 2026-11-10 首页 2.0（参考稿复刻）：问候 + 背景 + 最近五个项目。
    check("home_action_cards_removed",
          "home-card-chat" not in canvas and "home-card-space" not in canvas,
          "首页不再显示四张入口卡片")
    check("home_subtitle_removed",
          "让 AI 成为您的智慧工作台" not in canvas,
          "问候下方的说明句已移除")
    check("home_greeting_no_emoji",
          "👋" not in canvas,
          "问候语不再带手掌表情")
    check("home_greeting_auto_hide",
          "conversationStarted" in canvas and "conversationStarted ? null" in canvas,
          "开始对话后收起问候区，把空间让给历史记录")
    check("home_recent_projects",
          "projectList()" in canvas and "home-recent-projects" in canvas,
          "底部小卡片改为最近五个项目（真实数据）")
    check("home_scene_backdrop",
          "app-scape-art" in app
          and "app-scape-base" in app
          and (PUBLIC / "home-scape.png").is_file(),
          "首页背景使用自托管风景插画，其余页面只留底色")
    check("home_quick_nav",
          "home-recent-projects" in canvas and "home-quick-pill" in theme,
          "项目小卡片沿用胶囊样式")
    check("home_spec_bar_absent",
          "设计规范" not in canvas and "Neumorphism Workbench" not in canvas,
          "首页不出现底部设计规范条")
    check("home_no_dark_mode",
          "prefers-color-scheme: dark" not in theme,
          "本版本不实现深色模式")
    check("desktop_user_name_readonly",
          "desktop_user_name" in api and "desktop_user_name" in (REPO / "desktop/src-tauri/src/lib.rs").read_text(encoding="utf-8"),
          "系统用户名走只读命令，读不到就不带称呼")

    passed = sum(1 for _, ok, _ in CHECKS if ok)
    total = len(CHECKS)
    for name, ok, detail in CHECKS:
        print(("PASS" if ok else "FAIL") + f" {name}" + (f" — {detail}" if detail else ""))
    print()
    print(f"{passed}/{total} checks passed")
    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())
