// OmniDesk 桌面外壳入口。
//
// 本文件只做一件事：调用库里的 run()。所有宿主逻辑都在 lib.rs 及其子模块，
// 以便集成测试（src-tauri/tests/）能直接链接这些模块。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    omnidesk_desktop_lib::run()
}
