// Windows 릴리스 빌드에서 추가 콘솔 창이 뜨는 것을 막는다. 제거 금지.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    rhwp_desktop_lib::run()
}
