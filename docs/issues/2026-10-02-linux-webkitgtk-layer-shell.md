# 2026-10-02 Linux（Raspberry Pi OS）贴纸窗口：WebKitGTK 渲染与 layer-shell

状态：已实现首版 Linux 支持；第 5 节列出未解决项。

环境：Raspberry Pi 5（V3D GPU），Raspberry Pi OS（Debian 13 trixie，arm64），labwc 0.20.2（wlroots 0.20，Wayland，XWayland 可用），pcmanfm-pi 1.11 桌面，wf-panel-pi 1.38，WebKitGTK 2.54.0，gtk-layer-shell 0.9.0，Tauri 2.10 / tao 0.34 / wry 0.54。

验证方式：在真实桌面会话中运行 debug 版，用 `grim` 截图、`wlrctl`/`wtype` 与 uinput 虚拟鼠标模拟点击、拖拽和输入，`WAYLAND_DEBUG=client` 追踪协议，并用独立的 Python/GTK 程序（PyGObject + WebKit2 4.1 + GtkLayerShell）隔离复现。

## 1. 现象与根因

### 1.1 DMA-BUF 渲染器画出乱码

默认配置下，面板首次显示是横向拉伸的乱码帧。WebKit 自带的 `MiniBrowser` 打开同一页面也一样，说明是 WebKitGTK 2.54 在该 GPU 上的 DMA-BUF 渲染器问题，与本应用无关。设置 `WEBKIT_DISABLE_DMABUF_RENDERER=1`（共享内存渲染器）后正常。

处理：`src-tauri/src/main.rs` 在 Linux 上未设置该变量时默认设为 `1`。

### 1.2 透明窗口只重绘受损区域

使用共享内存渲染器后，`transparent: true` 的窗口在隐藏后再次显示时，只有之后被 WebKit 重绘的区域（如鼠标悬停的行、时间戳）有内容，其余区域完全透明。GTK 原生 XWayland（`GDK_BACKEND=x11`）下同样出现，因此不是 labwc 或 Wayland 特有。

关掉 WebKit 的 damage 传播（`PropagateDamagingInformation` 等 feature）、改用 Skia CPU 渲染、map 后对 web view 做一次 hide/show、显示后把窗口尺寸 +1px 再还原，都没有解决，相关实验代码已移除。以下做法验证有效：

- 主面板通过 `src-tauri/tauri.linux.conf.json` 设为 `transparent: false`（Tauri 平台配置按 JSON merge patch 合并，数组整体替换，所以重复写出整个 `windows[0]`）。
- 工作台在 `desktop/panel.rs` 中 Linux 下不透明。
- 贴纸窗口（`src/lib/panel/use-window-sync.js`）在 Linux 下既不设 `transparent: true`，也不设 `backgroundColor: [0, 0, 0, 0]`。只关 `transparent` 而保留全透明 `backgroundColor` 时，贴纸被拖动或切换层级后仍会只剩局部内容。

贴纸控制模式展开后，主体以外的区域依旧能透出桌面，这个外观与 Windows 一致。

即使按上面创建窗口，layer surface 被移动、改尺寸或换层之后，WebKit 仍只重绘它认为受损的区域（文字块）：release 版拖拽并切换层级后，贴纸上部文字块与下部之间出现一条笔直的色差分界；重新创建窗口后消失，证明是残留旧画面而非样式。处理：`note_surface.rs` 在每次 reconfigure 后 120ms（拖拽时只在最后一次之后）执行一段脚本，把根元素 `opacity` 设为 `0.999` 再于两帧后还原，使整页受损、下一帧完整重绘。

### 1.3 Wayland 下贴纸无法定位，也无法分层

- 原生 Wayland 的 xdg_toplevel 不能自行设定位置：tao 的 `set_position` 无效，`outer_position` 恒为 `(0, 0)`，贴纸的 `x/y` 无法还原。
- 改走 XWayland 可以定位，但 labwc 0.9.6 起默认拒绝 X11 客户端的置顶请求（需用户在 `rc.xml` 加 `allowAlwaysOnTop` 窗口规则），且不支持 X11 的置底请求。贴纸只能是普通窗口，会出现在任务栏和 Alt+Tab 里。

处理：在支持 wlr-layer-shell 的合成器上，贴纸窗口改为 layer surface（`src-tauri/src/platform/linux/note_surface.rs`）。锚定输出左上角，margin 即坐标，`exclusive_zone = -1`（坐标相对输出边缘，不受面板保留区影响），键盘交互 `OnDemand`。

| 应用层级 | layer | 实测 |
| --- | --- | --- |
| 桌面层（默认） | Bottom | 位于 pcmanfm 图标之上、普通窗口之下；全局操作关闭时点击穿透到桌面（右键弹出 pcmanfm 桌面菜单） |
| 置顶 / 全局操作 | Top | 位于普通窗口之上；可拖拽、可进入控制模式并用键盘输入 |
| 壁纸层 | Background | pcmanfm 用同一个 surface 画壁纸和图标，贴纸无法位于图标之下，效果同桌面层 |

layer surface 不出现在任务栏和 Alt+Tab 中。`DESK_TIDY_LAYER_SHELL=0` 或合成器不支持 layer-shell（X11、GNOME）时，退回原有的 `set_always_on_top` 通用路径。

### 1.4 layer surface 的尺寸来自 size request

gtk-layer-shell 用 GTK 窗口的 size request 作为 surface 尺寸，`gtk_window_resize`（tao 的 `set_size`）对它无效。WebKit web view 本身没有 size request，所以 surface 被配置为 0 大小，从不提交 buffer，贴纸完全不可见（`WAYLAND_DEBUG` 中该 `wl_surface` 只有初始的 `attach(nil)`）。Python 复现：同样的 layer 窗口放 `Gtk.Label` 正常显示，放 WebKit 不显示，给 web view 设 `set_size_request` 后正常。

处理：初始化时按创建尺寸调用 `set_size_request`；新增 `set_note_window_size` 命令，Linux 下改 size request，其他平台仍走 `set_size`。

### 1.5 位置查询与指针坐标

- `outer_position` 对 layer surface 返回 `(0, 0)`，`outer_size` 是旧值。Rust 侧在 `LinuxNoteSurfaceState` 中记录每张贴纸的逻辑坐标与尺寸，`auto_hide` 的窗口矩形、`persist_note_window_size` 和新命令 `get_note_window_position` 都从这里读取。
- WebKitGTK 在 Wayland 下给出的 `screenX/screenY` 是相对 surface 的坐标（日志中 `screenX == clientX`）。原拖拽算法按相邻事件的 `screenX` 差值移动窗口，窗口跟着指针移动后差值归零，拖不动。`get_note_window_position` 返回 `surfaceRelativePointer: true` 时，`note-window-drag.js` 改为以按下点为固定参照，每个事件相对按下点的偏移即窗口还需移动的距离。Windows/macOS 不变。
- 每个指针事件相对的是合成器发出它时窗口所在的位置；`WAYLAND_DEBUG` 显示 `set_margin` 之后约 2–7ms 才收到 `configure`，期间的 motion 仍相对旧位置，照算会把同一段移动计两次而越拖越超前。因此移动请求未完成或完成后 32ms 内的事件一律丢弃，只用之后的事件。Wayland 的按键事件不带坐标，WebKit 在松开时沿用最后一次 motion 的坐标：若该坐标已应用过则不再应用（否则松手时会再超前一段），否则用它补上 settle 期间丢弃的移动。
- 实测时注意：用 uinput 相对移动模拟指针会经过 libinput 加速，指针实际位移大于发送的数值；应以“按下点始终在指针下”为准，或用 `wlrctl pointer move`（virtual-pointer，无加速）产生移动。

### 1.6 tao 在未 realize 的窗口上 unwrap

tao 处理 `set_ignore_cursor_events(true)` 时对 `window.window()` 直接 `unwrap()`。贴纸窗口隐藏创建，在 layer-shell 初始化之后、显示之前尚未 realize；如果此时收到该请求（贴纸页面挂载、或全局操作切换遍历所有贴纸），主线程会 panic。处理：初始化 layer-shell 后立即 `realize()`。

### 1.7 全局快捷键与托盘

- `global-hotkey` 在 Linux 上通过 X11 抓键。Wayland 会话里 DISPLAY 指向 XWayland，注册能成功，但焦点在 Wayland 窗口时收不到按键。
- 托盘项通过 libayatana-appindicator 注册（`RegisteredStatusNotifierItems` 可见），但 `IconName` 是 PNG 绝对路径。wf-panel-pi 的托盘只用图标主题查 `IconName`，不支持绝对路径，也不读取该项提供的 `IconThemePath`；libayatana 不提供 `IconPixmap`，因此不显示。同一面板上能显示的托盘项，一个用主题图标名（Fcitx：`input-keyboard-symbolic`），一个提供 `IconPixmap`（Electron 应用）。tray-icon 0.21 在 Linux 上只能用文件设置图标，无法改为主题图标名。

处理（托盘，2026-10-03）：参照 cats-platform（Electron 在 Linux 上把托盘图标作为像素传出），Linux 下改用 ksni 自行发布 StatusNotifierItem，`IconPixmap` 为 ARGB32（`desktop/tray_linux.rs`），菜单项与 Tauri 托盘共用 `run_tray_action`，文字随前端语言更新；找不到 StatusNotifierWatcher 时退回 Tauri 托盘。实测 wf-panel-pi 显示图标，左键打开面板，右键菜单（含分隔线）各项可用，“退出”经 `EventGroup` 正常结束程序。

处理（快捷键）：单实例回调支持命令行动作（`--toggle-panel`、`--toggle-global-operation`、`--hide-or-reveal-stickies`、`--quit`），可绑定到 labwc 的 `rc.xml` 快捷键；没有实例运行时 `--quit` 直接退出。用法见 `docs/build/2026-10-02-linux-deb.md`。

### 1.8 切换层级后点击穿透失效（2026-10-03）

现象：全局操作开启再关闭后，桌面层贴纸不再穿透，可在贴纸上选取文字、右键弹出 WebKit 菜单；刚启动时则正常（右键弹出 pcmanfm 桌面菜单）。

根因：tao 在 Wayland 下为每个窗口装一个 header bar（`set_titlebar`），GTK 因此把它当作客户端装饰窗口，每次 size allocation 都会按 widget 自身的 input shape 重建 GdkWindow 的输入区域。tao 的 `set_ignore_cursor_events(true)` 直接把空区域写在 GdkWindow 上，layer surface 移动或换层触发的重新分配会把它覆盖成整窗可点击；启动时的顺序恰好先分配后设置，所以没暴露。

处理：Linux 下把 input shape 设在 GTK widget 上（`set_note_ignore_cursor`，空区域 = 穿透，`None` = 接收鼠标），GTK 重建时会保留它；Rust 侧与贴纸页都改走这条路径（贴纸页用 `set_note_window_ignore_cursor` 命令）。另外 GDK 只在窗口下一帧提交时才把新的输入区域发给合成器，单纯改输入区域不会产生新帧（实测“桌面层贴纸可选取文字”开关切换后要等别的重绘才生效），所以改完后 `queue_draw()`。

## 2. 改动清单

- Rust：`platform/linux/note_surface.rs`（新），`desktop/sticky/{panel_window,layer,mod,auto_hide,effects}.rs`，`notes/commands.rs`，`desktop/{panel,shortcuts,mod}.rs`，`desktop_app.rs`，`main.rs`；`Cargo.toml` 增加 Linux 专用依赖 `gtk 0.18`、`gtk-layer-shell 0.8`（feature `v0_6`）。
- 配置：`tauri.linux.conf.json`（新），`tauri.conf.json` 的 `bundle.linux.deb`（`depends` 增加 `libgtk-layer-shell0`、`libayatana-appindicator3-1`；`.desktop` 模板 `src-tauri/linux/desk_tidy_sticky.desktop`，分类 `Utility`）。
- 前端：`src/lib/runtime/platform.js`（新，`isLinuxDesktop`），`use-window-sync.js`，`note-window-drag.js`，`note/[id]/+page.svelte`；测试 `tests/frontend/sticky-note-interaction.test.js` 增加 surface 相对坐标拖拽用例。
- 构建：`scripts/linux/build-deb.sh`、`scripts/linux/test-deb-smoke.sh`，`make package-deb`/`package-deb-smoke`。

## 3. 实机验证结果

- 面板显示/隐藏多次后内容完整；工作台首次显示完整。
- 贴纸：启动时按保存坐标出现在桌面层；置顶后位于终端窗口之上；拖拽后坐标写回 `notes.json`，release 版用 virtual-pointer 做快/慢共 4 次拖拽，窗口位移与指针位移完全一致；在桌面层、置顶、壁纸层之间切换后内容完整；桌面层点击穿透到 pcmanfm；置顶贴纸点编辑按钮后可用键盘输入并保存；控制模式展开/收起时窗口扩张、移动并恢复原尺寸。
- `make check`、`make test`（前端 48、Rust 60）通过。

## 4. 调试入口

- `DESK_TIDY_LAYER_DEBUG=1`：层级切换、移动、尺寸变化日志。
- `WAYLAND_DEBUG=client`：确认 `zwlr_layer_surface_v1` 的 `set_layer`/`set_margin`/`set_size` 与 surface 是否提交了 buffer。
- `DESK_TIDY_LAYER_SHELL=0`：对照普通窗口行为。

## 5. 未解决

- 贴纸无法用窗口边缘拖拽改变尺寸：tao 的无边框缩放依赖 xdg_toplevel，layer surface 没有。需要程序内缩放手柄。
- 壁纸层无法位于 pcmanfm 图标之下（1.3）。
- 磨砂没有原生模糊效果；贴纸透明度调节在 Linux 上的视觉效果未单独验证。
- 多显示器：坐标按包含该点的输出换算 margin，未在双屏 Pi 上实测。
- 休息提醒遮罩窗口（`/break-overlay`）在 Wayland 下无法按显示器定位，未做 layer-shell 处理（休息提醒默认关闭）。
