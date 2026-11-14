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

统一工作流： [`.github/workflows/ci.yml`](.github/workflows/ci.yml)。推送 `main` / `master` 或提交 PR 时，先用 Docker 编译并运行测试，上传 exe 与 SHA256 校验文件。

主分支编译结束后会拉取标签，并检查**本次构建的确切提交**：只有 `v<Cargo.toml 版本>`（例如 `v0.1.1`）精确指向该提交，才创建/更新对应 GitHub Release，并发布 GHCR 构建镜像。没有对应标签则仅保留构建产物，不会发布历史版本。一次主分支推送只产生一个工作流运行，所有步骤在同一任务中依次执行：编译/测试 → 检查该提交的版本标签 → 按条件发布。保留 PR 编译验证，但**单独推送标签不会触发工作流**。

发布示例（请先更新并提交 Cargo.toml 版本）：

```bash
git tag -a v0.1.1 -m "发布 v0.1.1"
git push --atomic origin main v0.1.1
```

标签必须在主分支构建结束、检查标签之前到达远端，推荐上面的原子推送，让 main 与标签一起生效。若标签晚于本次检查，单独推标签不会补发；可在标签到达后手动重新运行该主分支构建。工作流使用完整历史和标签，绝不选择与已构建提交无关的“最新标签”。版本保持与 Cargo.toml 一致，标签必须匹配该版本。

### 拉取构建镜像（GHCR）

镜像名为 `ghcr.io/<owner>/<repo>`（自动转小写），发布标签为版本号（如 `0.1.1`）和 `latest`。这里是 Rust + mingw 的**交叉编译环境**，Windows 程序请到 GitHub Release 下载。公开包可直接拉取；私有包需要具有 `read:packages` 权限的正常 GitHub 登录。新建包默认可能是私有的，可在 GitHub Packages 中改为 Public。

```bash
echo "$GITHUB_TOKEN" | docker login ghcr.io -u USERNAME --password-stdin
docker pull ghcr.io/<owner>/<repo>:0.1.1
docker pull ghcr.io/<owner>/<repo>:latest
```

本地构建仍执行 `./build.sh`（使用现有 Dockerfile），不需要 GitHub Actions。

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
.github/workflows/       # 统一串行 CI/Release（主分支构建后检查同提交版本标签）
```

## 许可

MIT。原版 UsbipdTool © NolanGUO（MIT）。
