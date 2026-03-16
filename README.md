# maimai-android-touch-panel-rs

[English](README_en-US.md) | 简体中文

使用 `adb shell getevent` 记录 Android 设备触屏事件并模拟 maimai 触摸屏幕的程序 (Rust 版本).

## 提示
玩具项目，在 Xiaomi Pad 5 Pro (Android 13) 及 Xiaomi Pad 6 Pro (Android 15) 上做了测试，且仅适配了 Linux 多点触控协议类型 B。

目前已知的问题有：

- 仅支持 Linux 多点触控协议类型 B 而不支持 A，这可能会导致较旧的设备不受支持，
  两种类型不同之处详见[文档](https://www.kernel.org/doc/Documentation/input/multi-touch-protocol.txt)
- 输出 Touch Keys 但无按键按下(分辨率问题)
- 游戏内按两下只识别一个 tap(程序未进入运行模式)
- 游戏内始终显示按下(未知原因)

更加优秀的项目有：

- [KanadeDX](https://github.com/KanadeDX/Public) (某八个按键程序在 Android/iOS 上的实现)
- [AstroDX](https://github.com/2394425147/astrodx) (Android)
- [MajdataPlay](https://github.com/LingFeng-bbben/MajdataPlay) (Windows，Android)

这些项目包含对 Mai2 Chart Player 的完整实现，而不仅仅是一个触摸输入程序。

## 使用方法

1. 请先将游戏配置文件中 `DummyTouchPanel` 的值改为 `0`
2. 打开任意 P 图工具，准备一个和设备屏幕大小相同的一张图片(例如:1600x2560)，将 `./image/color_exp_panel.png`
   放置到该图片圆形触摸区域的位置，编辑好的图片放到 `image` 目录下取名 `image_monitor.png`.
3. 编辑 `config.toml` 配置文件，修改 `[zone_colors]` 配置，将各区块对应的 RGB 通道颜色值改为刚 P 的图的对应区块颜色值(一般不用改，默认就行)
4. 电脑安装 ADB 调试工具，安装路径添加到系统环境变量里面
5. 从 [Releases](../../releases) 页面下载最新的 `maimai-touch-windows.zip` 并解压到任意目录。或者从源码构建(安装 [Rust 工具链](https://rustup.rs)，然后运行 `cargo build --release`)
6. 先将实际屏幕大小填入 `config.toml` 内 `monitor_size` 配置，打开终端，运行 `adb shell getevent -l`，点一下屏幕的最右下角的位置，在终端获取该次点击得到的 `ABS_MT_POSITION_X` 和 `ABS_MT_POSITION_Y` 的数值，把十六进制转换到十进制，将得到的数据填入到 `input_size` 配置
7. Android 设备充电口朝下一般为屏幕的正向，如需反向屏幕游玩可将配置 `reverse_monitor` 改为 `true`
8. 编辑 `config.toml` 配置文件，按文件内说明修改多个配置
9. 下载一个 `VSPD` 虚拟串口工具，将 `COM3` 和 `COM33` 建立转发
10. 手机打开 USB 调试，强烈建议同时使用 USB 网络共享连接电脑，串流走 WLAN 可能不是很稳定
11. 电脑画面可使用 `Apollo`，`IddSampleDriver`，`Sunshine` 和 `Moonlight` 或者延迟较大但比较方便的 `spacedesk` 等软件串流到 Android 设备，详细过程请自行寻找，不在本篇讨论范围之内
12. 手机连接电脑，先双击运行 `start.bat`，再运行游戏，控制台输出 `Connected to game` 即可
13. 进游戏调整延迟，一般判定 A/B 都要调才能正常用，我这边是 `A:-1.0/B:+0.5` 到 `A:-2.0/B:+2.0`
14. 打一把看看蹭不蹭星星/触控是否灵敏，根据体验修改 `config.toml` 中的 `area_scope` 值
15. 如果单点延迟低但滑动时延迟极大，请将 `config.toml` 中 `sleep_mode` 修改为 `false`，或者可以调小 `sleep_delay_us` 的值(如果还是卡请提交 issue 反馈)

## 命令列表

游戏时如果不小心断开连接，请在控制台输入 `start` 并回车来重新连接游戏

输入 `reverse` 可调整触控设备屏幕方向

输入 `restart` 可重新读取配置文件/重启程序

输入 `exit` 可完全退出程序(也可使用 Ctrl+C)

## 从源码构建

安装 [Rust 工具链](https://rustup.rs)，然后在项目目录下运行：

```bash
cargo build --release
```

编译后的可执行文件位于 `target/release/maimai-touch-rs.exe` (Windows)。

## 部分问题

关于延迟/其他建议可参考 [#3](https://github.com/ERR0RPR0MPT/maimai-android-touch-panel/issues/3)

Q：在安卓高版本(13-15)上测试触摸区域完全对不上，只有点屏幕左上角有用，图片用的是平板实际分辨率，在一台安卓 10 设备测试是正常的

A：按步骤修改 `config.toml` 内 `monitor_size` 和 `input_size` 配置

Q：关闭再打开报错

A：如果直接关闭控制台窗口有可能导致后台进程残留，请使用 Ctrl+C 终止程序或者在退出前输入 `exit`。

## 注意

想要加 2P 的重新复制一下程序并添加串口 COM4 到 COM44 的转发，并且在 `config.toml` 配置文件的 `specified_device` 中指定使用 `adb devices` 获取到的设备序列号

该程序仅用于测试，目前来说打 12-13 也可以鸟加，13+ 以上开始容易断，需要在之后进行更好的优化。

## 许可证
[MIT License](/LICENSE)
