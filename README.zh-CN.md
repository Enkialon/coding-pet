# CodingPet

[English](README.md) | 简体中文

CodingPet 是一个使用 Rust 编写的轻量级 Windows 与 macOS 程序员桌面宠物。

它兼容 Codex 自定义宠物资源格式，可以直接在桌面显示动画宠物，并通过统一的本地事件协议接入 Codex、Cursor、Claude Code、IDE、Git 和 CI 等编程工具。

## 功能特性（V0.1）

- 桌面透明无边框动画宠物
- 窗口始终置顶
- 拖动宠物调整位置，并播放对应方向的移动动画
- 支持 Codex v2 的 16 个顺时针注视方向
- 支持多显示器和负坐标
- 支持每显示器 DPI 感知
- 加载 Codex v2 宠物图集（1536×2288、8×11、PNG/WebP）
- 迁移期间兼容旧版 Codex v1 8×9 宠物
- 自动发现 `~/.codex/pets/` 下的宠物
- Windows 系统托盘或 macOS 菜单栏图标，可显示、隐藏和退出
- 右键菜单播放动画和切换宠物
- 可选的随机待机动画
- 自动保存宠物选择、窗口位置和设置
- 事件驱动，空闲时几乎不占用 CPU
- 面向 Codex 和其他编程工具的本地集成 API

## 构建

需要 Windows 或 macOS，以及 Rust stable 工具链。

```bash
cargo build --release
```

构建产物：

```text
target/release/coding-pet.exe
target/release/coding-pet
```

Windows 产物是单文件可执行程序；GitHub Release 会提供同时支持 Intel 和 Apple Silicon 的 macOS `.app` 压缩包。两者都不需要额外运行时。

## 运行

双击下载的应用，或者在终端运行本地构建产物：

```powershell
.\target\release\coding-pet.exe
```

```bash
./target/release/coding-pet
```

启用调试日志：

```powershell
$env:RUST_LOG = "debug"
.\target\release\coding-pet.exe
```

## 编程工具集成

CodingPet 会在 `127.0.0.1` 的随机端口启动带令牌保护的 HTTP API，并在可执行文件中提供通用事件发送器：

```powershell
coding-pet.exe emit agent.turn.started --source codex --session turn-123
coding-pet.exe emit agent.turn.completed --source cursor --session turn-123
coding-pet.exe emit agent.input.required --source claude-code --message "需要批准"
```

Codex、Claude Code、Cursor、IDE、Git 和 CI 适配器只需要把自身生命周期事件转换成相同的版本化事件信封。宠物核心不解析各工具的私有负载。

完整事件目录、HTTP 契约、安全机制和适配规则请参阅[集成协议 v1](docs/integrations.md)。

## Codex 宠物导入

CodingPet 会自动扫描 Codex 的本地宠物目录：

```text
~/.codex/pets/
```

### 目录结构

```text
.codex/
└── pets/
    ├── cat/
    │   ├── pet.json
    │   └── spritesheet.webp
    └── dog/
        ├── pet.json
        └── spritesheet.png
```

### Codex v2 图集格式

- 资源契约：Codex v2（`spriteVersionNumber: 2`）
- 图集尺寸：1536×2288
- 文件格式：带透明通道的 PNG 或 WebP
- 网格：8 列×11 行
- 单元格：192×208
- 未声明 `spriteVersionNumber` 的 1536×1872 图集按旧版 v1 加载

### 动画布局

| 行 | 动画 | 帧数 |
|---:|---|---:|
| 0 | 待机（Idle） | 6 |
| 1 | 向右拖动（Running Right） | 8 |
| 2 | 向左拖动（Running Left） | 8 |
| 3 | 挥手（Waving） | 4 |
| 4 | 跳跃（Jumping） | 5 |
| 5 | 失败（Failed） | 8 |
| 6 | 等待输入（Waiting） | 6 |
| 7 | 工作中（Running） | 6 |
| 8 | 审查中（Review） | 6 |
| 9 | 000°～157.5° 注视方向 | 8 |
| 10 | 180°～337.5° 注视方向 | 8 |

第 9～10 行组成连续顺时针的 16 个注视方向：

- `000°`：向上
- `090°`：屏幕右侧
- `180°`：向下
- `270°`：屏幕左侧

鼠标位于宠物中心死区时，宠物恢复正常待机或当前编程任务状态。

### pet.json

```json
{
  "id": "my-pet",
  "displayName": "我的宠物",
  "description": "一个自定义桌面宠物",
  "spriteVersionNumber": 2,
  "spritesheetPath": "spritesheet.webp"
}
```

未知字段会被忽略，以便向前兼容。省略 `spriteVersionNumber` 时使用旧版 v1 契约。

## 配置

配置文件位置：

```text
%APPDATA%\CodingPet\config\config.json
```

示例：

```json
{
  "pet": "my-pet",
  "position": { "x": 1200, "y": 700 },
  "scale": 0.75,
  "always_on_top": true,
  "idle_animation": false,
  "language": "auto"
}
```

配置字段：

| 字段 | 说明 |
|---|---|
| `pet` | 当前宠物 ID |
| `position` | 窗口在桌面上的物理像素坐标 |
| `scale` | 宠物缩放比例 |
| `always_on_top` | 是否保持置顶 |
| `idle_animation` | 是否启用随机待机动画 |
| `language` | `auto`、`zh` 或 `en` |

配置缺失或损坏时会自动使用默认值。

## 交互优先级

宠物动画按以下优先级进行仲裁：

```text
拖动动画 > 鼠标注视 > 编程工具状态 > 随机待机动画
```

鼠标交互期间收到的 Hook 事件不会丢失；交互结束后会继续显示最新的任务状态或一次性通知动画。

## 性能目标

| 指标 | 目标 |
|---|---|
| 空闲 CPU | 约 0%（事件驱动，无忙轮询） |
| 动画 CPU | 小于 2% |
| 常驻内存 | Windows GDI 后端小于 30 MB（典型约 14 MB） |
| 启动速度 | 即时启动，无启动画面和网络请求 |

## 架构

- 纯 Rust、单线程事件循环
- 动画、宠物资源、Hook 和配置组成跨平台核心
- Windows 后端使用 Win32 `UpdateLayeredWindow` 实现逐像素透明
- macOS 后端使用透明 AppKit 窗口和事件驱动的 Metal 表面
- 使用 `winit` 处理窗口和输入事件
- 仅在动画帧变化时重绘
- 不使用 WebView 或外部运行时；Windows 后端仍为纯 CPU/GDI
- 宠物资源层采用 Codex v2 契约
- 工具集成层采用独立的提供方无关事件协议

## 路线图

- **V0.2**：提供方适配器和可配置的事件动画映射
- **V0.3**：AI 任务状态显示和 Toast 通知
- **V0.4**：久坐、喝水、眼睛和手腕休息提醒
- **V0.5**：自定义事件映射、设置界面和插件系统

## 发布

推送语义化版本标签后，GitHub Actions 会自动测试 Windows 和 macOS，构建 Windows x86_64 可执行文件与 macOS 通用应用，生成 SHA-256 校验文件，并发布到 GitHub Release。

```powershell
# 先更新 Cargo.toml 中的版本号，然后执行：
git commit -am "chore: release v0.0.2"
git tag v0.0.2
git push origin main v0.0.2
```

标签去掉开头的 `v` 后，必须与 `Cargo.toml` 中的包版本完全一致。

## 许可证

待定。
