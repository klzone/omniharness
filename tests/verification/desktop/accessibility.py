#!/usr/bin/env python3
"""AT-SPI 辅助：按界面元素名称定位与断言，不靠猜测像素。

只用于一次性 Xvfb 显示上的验收窗口。AT-SPI 读取的是 WebKit 暴露的无障碍树，
因此“元素存在/可执行”与截图是两份相互独立的证据。
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass
class Node:
    role: str
    name: str
    text: str
    accessible: object


def init() -> None:
    import gi

    gi.require_version("Atspi", "2.0")
    from gi.repository import Atspi

    Atspi.init()


def _collect(node, out: list, depth: int = 0) -> None:
    if depth > 24:
        return
    out.append(node)
    try:
        count = node.get_child_count()
    except Exception:
        return
    for index in range(count):
        try:
            _collect(node.get_child_at_index(index), out, depth + 1)
        except Exception:
            continue


def _node(n) -> Node:
    try:
        role = n.get_role_name() or ""
    except Exception:
        role = ""
    try:
        name = n.get_name() or ""
    except Exception:
        name = ""
    text = ""
    try:
        t = n.get_text()
        if t is not None:
            text = t.get_text(0, t.get_character_count()) or ""
    except Exception:
        text = ""
    return Node(role=role, name=name, text=text, accessible=n)


def nodes() -> list[Node]:
    from gi.repository import Atspi

    raw: list = []
    _collect(Atspi.get_desktop(0), raw)
    return [_node(n) for n in raw]


def find(name: str, role: str | None = None, contains: bool = False) -> Node | None:
    """按名称或可见文字查找第一个元素。"""
    for node in nodes():
        if role is not None and node.role != role:
            continue
        haystacks = (node.name, node.text)
        hit = any((name in h) if contains else (name == h) for h in haystacks)
        if hit:
            return node
    return None


def has_text(fragment: str) -> bool:
    return any(fragment in (n.name + "\n" + n.text) for n in nodes())


def labels() -> list[str]:
    return [n.name or n.text for n in nodes() if (n.name or n.text)]


def invoke(node: Node) -> bool:
    """优先走无障碍 Action；没有 Action 时按元素屏幕外接矩形点击中心。"""
    from gi.repository import Atspi

    try:
        action = node.accessible.get_action()
        if action is not None and action.get_n_actions() > 0:
            return bool(action.do_action(0))
    except Exception:
        pass
    try:
        component = node.accessible.get_component()
        ext = component.get_extents(Atspi.CoordType.SCREEN)
        if ext.width <= 0 or ext.height <= 0:
            return False
        from window_harness import XSession

        # 由调用方已经选好 DISPLAY；XSession 只在这一刻建立注入连接。
        import os

        XSession(os.environ["DISPLAY"]).click(
            ext.x + ext.width // 2, ext.y + ext.height // 2, settle=1.0
        )
        return True
    except Exception:
        return False


def invoke_named(name: str, role: str | None = None, contains: bool = False) -> bool:
    node = find(name, role=role, contains=contains)
    return invoke(node) if node is not None else False


def focus_point(name: str, role: str | None = None, contains: bool = False) -> tuple[int, int] | None:
    """返回元素中心点，供需要键盘输入的控件使用。"""
    from gi.repository import Atspi

    node = find(name, role=role, contains=contains)
    if node is None:
        return None
    try:
        ext = node.accessible.get_component().get_extents(Atspi.CoordType.SCREEN)
        if ext.width <= 0 or ext.height <= 0:
            return None
        return (ext.x + ext.width // 2, ext.y + ext.height // 2)
    except Exception:
        return None
