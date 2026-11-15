# UsbipdTool（Rust 版）

UsbipdTool 是一个 Windows 图形界面工具，用于把本机 USB 设备共享并附加到 WSL 2。当前版本：**0.1.2**（窗口标题与信息栏会显示 `v0.1.2`）。

本程序需要已单独安装的 [usbipd-win](https://github.com/dorssel/usbipd-win)，它不会随本工具一起安装。运行时还会请求管理员权限（UAC）。界面沿用 [IOT_Power](https://github.com/HadyLiu/IOT_Power) 的明暗风格。

## 文档

- [使用说明](doc/使用说明.md)：安装前提、界面操作、附加设置与直接附加
- [编译](doc/编译.md)：在 Linux / WSL 中用 Docker 交叉编译
- [发布](doc/发布.md)：GitHub Actions 与版本标签发布
- [项目结构](doc/项目结构.md)：源码布局、图标来源与本地测试

## 快速开始

1. 安装 usbipd-win 和 WSL 2。
2. 以管理员身份运行 `dist/UsbipdTool.exe`（会弹出 UAC）。
3. 绑定设备后，点击「附加」会按已保存的设置直接附加，不再弹出对话框。附加目标可在工具栏「设置」中修改。

在 WSL / Linux（需 Docker）编译：

```bash
./script/build-image.sh    # 构建编译工具链镜像（Dockerfile 变更时）
./script/build.sh          # 使用已有镜像测试并编译，输出 dist/UsbipdTool.exe
./script/build.sh test     # 只跑测试
./script/build.sh build    # 只编译 exe
```

## 许可

MIT。原版 UsbipdTool © NolanGUO（MIT）。
