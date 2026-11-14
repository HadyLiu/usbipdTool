# UsbipdTool（Rust 版）

UsbipdTool 是一个 Windows USB 设备转发工具，可通过图形界面将 USB 设备共享并附加到 WSL 2，支持扫描设备、绑定、附加、分离和解绑，也可用于 CH340/CH343、CP210x 等串口适配器。

程序使用 Rust 和 eframe/egui 构建，提供单文件 exe，无需 Python 运行时；运行时需要安装 usbipd-win。

当前版本：**0.1.1**（窗口标题与信息栏会显示 `v0.1.1`；打 tag 发布时会与 tag 对齐）。

## 功能

- 启动自检：未检测到 `usbipd` 时提示安装（`winget install usbipd-win`）。
- 一键扫描：优先 `usbipd state`（JSON），失败回退解析 `usbipd list`；表格列 BUSID / VID:PID / 设备 / 状态。
- 插拔 USB 设备时自动刷新列表（监听 `WM_DEVICECHANGE`，约 0.6 秒去抖；正在执行绑定等操作时会排队，结束后再扫一次）。「重新扫描」按钮仍然保留。
- 按状态显示操作：未共享 → 绑定（可选「强制绑定」`--force`）；已共享 → 附加、解绑；已附加 → 分离、解绑。
  绑定因 USBPcap / hrdevmon 冲突失败时，会询问是否用 `--force` 重试。
- 附加对话框：使用默认发行版或从 WSL 发行版列表中选择（可手动输入）、可选主机 IP（`--host-ip`）、
  设备重插后自动重新附加（`--auto-attach`），并实时预览将执行的命令。
- 日志面板：回显实际执行的命令及其输出（UTF-8 / UTF-16 / GBK 自动解码）。
- 中英文界面运行时切换；浅色 / 深色 / 跟随系统主题。
- 设置保存在 `%APPDATA%\UsbipdTool\config.json`（键名与原版相同：`language`、`theme`、
  `force_bind_default`、`last_wsl_distro`、`last_host_ip`、`auto_attach_default`）。
- 所有命令在后台线程执行，不弹黑色控制台窗口；exe 内嵌 `requireAdministrator` 清单和图标，启动时弹一次 UAC。

## 使用

1. 安装 usbipd-win 和 WSL 2。
2. 双击 `dist\UsbipdTool.exe`（需管理员权限，会弹 UAC）。
3. 插入目标设备后列表会自动更新（也可点「重新扫描」），再「绑定」→「附加」到 WSL。
4. 在 WSL 中即可使用该 USB 设备（若是串口适配器，通常出现为 `/dev/ttyUSB*` 或 `/dev/ttyACM*`）。

命令对应关系（usbipd 5.x）：

| 操作 | 命令 |
|------|------|
| 扫描 | `usbipd state`（回退 `usbipd list`） |
| 绑定 | `usbipd bind --busid <id> [--force]` |
| 解绑 | `usbipd unbind --busid <id>` |
| 附加 | `usbipd attach --wsl [发行版] --busid <id> [--host-ip <ip>] [--auto-attach]` |
| 分离 | `usbipd detach --busid <id>` |

关于 `--auto-attach`：该参数会让 usbipd 进程常驻、在设备重插后自动重新附加。本工具把它放到后台
（无窗口）运行，输出实时写入日志；对该设备点「分离」/「解绑」或关闭程序时会先结束这个后台进程
（否则它会立刻重新附加）。

## 编译（Docker 交叉编译，推荐）

在 WSL / Linux 中（需要 Docker）：

```bash
cd /home/hady/workSpaceWsl/usbipdTool
./build.sh          # 跑单元测试 + 编译，输出 dist/UsbipdTool.exe
./build.sh test     # 只跑单元测试
./build.sh build    # 只编译 exe
```

`build.sh` 会用 `Dockerfile`（`rust:1-bookworm` + `mingw-w64` + `x86_64-pc-windows-gnu` 目标）构建镜像，
把项目目录挂载进容器执行 `cargo build --release --target x86_64-pc-windows-gnu`；图标与 UAC 清单由
`x86_64-w64-mingw32-windres` 通过 embed-resource 嵌入。cargo 依赖缓存在 Docker 卷
`usbipdtool-cargo-registry` 中，第二次编译会快很多。

在 Windows PowerShell 中也可以直接调用：

```powershell
wsl -e bash -lc "cd /home/hady/workSpaceWsl/usbipdTool && ./build.sh"
```

本机已装 Rust（windows-gnu 或 msvc 工具链）时也可直接 `cargo build --release`。


## GitHub Actions 与发布

把本仓库推到 GitHub 后，工作流会自动运行（无需改文件名，镜像名为 `ghcr.io/<owner>/<repo>`，自动转小写）。本地仍用 `./build.sh`。

| 工作流 | 触发 | 做什么 |
|--------|------|--------|
| [`.github/workflows/ci.yml`](.github/workflows/ci.yml) | 推送到 `main` / `master`，或提交 Pull Request | 用现有 `Dockerfile` 构建交叉编译镜像，运行 `./build.sh`（单元测试 + 产出 `dist/UsbipdTool.exe`），并把 exe 上传为 Artifact。推送到主分支时，再把构建镜像推到 GitHub Container Registry，标签为分支名（如 `main`）和 `sha-<7位>`。 |
| [`.github/workflows/release.yml`](.github/workflows/release.yml) | 推送形如 `v0.1.1` 的 tag | 用 tag 写入版本号（`VERSION=0.1.1`），再 `./build.sh`；创建 GitHub Release，附带 `UsbipdTool.exe` 与 `UsbipdTool.exe.sha256`；并把构建镜像推到 GHCR，标签为版本号和 `latest`。 |

发布新版本（请先把 `Cargo.toml` 里的 `version` 改成要发布的号，再打 tag）：

```bash
# 假设当前要发布 0.1.1
git tag v0.1.1
git push origin v0.1.1
```

不要漏掉前缀 `v`。Release 工作流会去掉 `v` 后写入 Cargo 版本，从而同步到 exe 的文件属性、清单和界面上的版本号。

### 拉取构建镜像（GHCR）

GHCR 上发布的是**交叉编译用的构建镜像**（Rust + mingw），不是 Windows 程序本身；Windows 可执行文件请到 GitHub Release 下载。

新建仓库时，包默认可能是私有的，可在 GitHub → Packages 里改成 Public。拉取示例：

```bash
# 公开包可直接拉取；私有包需先登录（用有 read:packages 权限的 PAT 或 GITHUB_TOKEN）
echo $GITHUB_TOKEN | docker login ghcr.io -u USERNAME --password-stdin

docker pull ghcr.io/<owner>/<repo>:0.1.1
# 或最新版、主分支构建：
docker pull ghcr.io/<owner>/<repo>:latest
docker pull ghcr.io/<owner>/<repo>:main

# 用该镜像在本地交叉编译（与 ./build.sh 相同）
IMAGE=ghcr.io/<owner>/<repo>:0.1.1 ./build.sh
```

将 `<owner>/<repo>` 换成你的 GitHub 用户名（或组织）和仓库名，并改为小写。

## 项目结构

```
src/
├── main.rs              # 入口：窗口 / 图标 / 提权兜底
├── lib.rs
├── core/                # 纯逻辑，不依赖 GUI，可单元测试
│   ├── models.rs        # UsbDevice / WslDistro
│   ├── parser.rs        # state JSON / list 文本 / wsl 列表解析
│   ├── decode.rs        # 输出解码（UTF-16 / UTF-8 / GBK / 系统代码页）
│   ├── usbipd.rs        # 命令执行（无窗口、超时）、is_admin
│   ├── actions.rs       # bind / unbind / attach / detach / scan
│   ├── config.rs        # 配置读写
│   ├── i18n.rs          # JSON 词典 i18n（复用原版词典）
│   └── icon.rs
└── gui/                 # eframe/egui 界面
    ├── app.rs           # 主窗口
    ├── attach_dialog.rs # 附加对话框
    ├── worker.rs        # 后台任务、--auto-attach 进程管理
    ├── hotplug.rs       # USB 插拔监听（WM_DEVICECHANGE）
    ├── theme.rs         # 浅色/深色主题
    ├── fonts.rs         # 加载系统中文字体（msyh.ttc / simhei.ttf）
    └── platform.rs      # 系统深色检测、runas 提权
resources/               # icon.ico、i18n 词典、清单、.rc
tests/core_tests.rs      # 解析器等单元测试（含本机 usbipd 5.3 真实输出）
.github/workflows/       # CI（push/PR）与 Release（v* tag）
```

## 许可

MIT。原版 UsbipdTool © NolanGUO（MIT）。
