# 2026-10-03 Windows 贴纸只有上方两角是圆角

状态：已修复。取代 `2026-04-18-windows-10-sticky-region-policy.md` 的“Windows 贴纸一律直角”决策。

环境：Windows 11 Pro 24H2+（build 26200），贴纸在桌面层（WorkerW 子窗口）。

## 现象

macOS / Linux 的贴纸四角都是 12px 圆角；Windows 贴纸下方两角是直角，上方两角却有约 3px 的小圆角。

## 根因

- 前端在 Windows 上把 `--note-radius` 设为 `0px`，贴纸本身画成矩形（见 2026-04-18 的决策）。
- 上方的圆角来自系统：嵌入 WorkerW 的贴纸是带 `WS_CAPTION` 的子窗口，DWM 不为子窗口绘制边框，于是主题（uxtheme）给它套上旧式标题栏边框的窗口区域，只裁掉上方两角。`apply_desktop_child_style()` 虽然会清掉窗口区域，但之后的样式与 Z 序调整又让系统把它加回来。
- 读取窗口区域可以确认：300×342 的贴纸，区域为 `(3,0)-(298,1)`、`(1,1)-(298,2)`、`(1,2)-(300,3)`、`(0,3)-(300,342)`。

## 处理

- `platform/windows/window_style.rs` 新增 `set_rounded_window_region()`，按窗口当前尺寸用 `CreateRoundRectRgn` 设置圆角区域。
- `desktop/sticky/corners.rs` 以 12 逻辑像素乘以缩放比例作为半径；`apply_windows_layer()` 每次套用图层后调用，`desktop_app.rs` 在贴纸窗口 `Resized` / `ScaleFactorChanged` 时再调用，因为窗口区域不会跟着尺寸变化。
- 应用自己设置的区域，系统不会再覆盖（`SWP_FRAMECHANGED` 之后实测仍保留）。
- 前端 `--note-radius` 在所有平台统一为 `12px`，`windows-flat` 仍保留不透明底色与无外阴影。
- `workerw` 不再清除窗口区域。

## 取舍

- GDI 区域没有抗锯齿，圆角边缘近看有锯齿；CSS 的抗锯齿边缘落在区域内侧。
- 2026-04-18 改为直角，是因为 Windows 10 上被裁剪的透明子窗口周围会留下尖刺状残影。本次只在 Windows 11 验证，Windows 10 未验证。

## 验证

在 Windows 11（150% 缩放）上用壁纸层贴纸验证：

- 嵌入壁纸层时，以及“启用贴纸全局操作”开启（脱离 WorkerW）再关闭（重新嵌入）后，读取窗口区域均为四角圆角；全局操作开启时截图确认四角圆角，角外露出下方内容。
- 调整贴纸尺寸后，区域跟随新尺寸。
- 桌面层与单独置顶的贴纸走同一段代码，未单独验证。
