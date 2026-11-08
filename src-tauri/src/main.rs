// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // WebKitGTK's DMA-BUF renderer draws garbled frames on Raspberry Pi OS (V3D GPU,
    // WebKitGTK 2.54), even in its own MiniBrowser. Use the shared-memory renderer unless
    // the user chose otherwise.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    desk_tidy_sticky_lib::run()
}
