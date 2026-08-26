# CodingPet

[English](README.md) | 简体中文

一只会响应 AI 编程状态的原生轻量桌面宠物。

**Windows 典型内存约 14 MB · 空闲 CPU 约 0% · 原生 Rust · 无 WebView · 无额外运行时**

Windows · macOS

[**下载 Windows x86_64 版**](https://github.com/Enkialon/coding-pet/releases/download/v0.2.2/coding-pet-v0.2.2-windows-x86_64.exe) · [**下载 macOS 通用版**](https://github.com/Enkialon/coding-pet/releases/download/v0.2.2/coding-pet-v0.2.2-macos-universal.zip) · [全部版本](https://github.com/Enkialon/coding-pet/releases)

CodingPet 把编程 Agent 的事件变成宠物动画，让你一眼看出 Agent 正在工作、等待操作、已经完成，还是执行失败。

| Agent 状态 | 宠物反应 |
|---|---|
| 工作中 | 跑步 |
| 审查或调用工具 | 审查 |
| 等待输入或授权 | 等待 |
| 已完成 | 挥手 |
| 失败 | 失败 |

## 为什么选择 CodingPet？

- **足够轻量：** Windows GDI 后端的典型内存占用约 14 MB。
- **空闲安静：** 事件驱动，不在后台持续重绘或轮询。
- **真正原生：** 纯 Rust；Windows 使用 Win32/GDI，macOS 使用 AppKit/Metal。
- **没有 WebView：** 不启动浏览器进程，也不要求安装额外运行时。
- **开放宠物格式：** 支持 Codex v2 PNG/WebP 宠物，并兼容旧版 Codex v1 宠物。
- **统一事件协议：** Codex 与其他编程工具可以驱动同一套本地事件接口。

## 快速开始

1. 下载对应平台的版本。
2. 启动 CodingPet。
3. 将 Codex 兼容宠物放入 `~/.codex/pets/`，或直接使用其中已有的宠物。
4. 使用下方内置发送器或适配器连接编程工具。

CodingPet 会自动发现包含 `spritesheet.webp` 或 `spritesheet.png` 的宠物目录。如果尚未安装宠物，程序会显示自动生成的备用图集，保证应用仍可运行；v0.2.2 暂未附带正式的 starter pet。

## 编程工具集成

CodingPet v0.2.2 已包含统一的本地事件 API、通用命令行发送器和内置 Codex Hook 适配器。其他工具可将自己的生命周期事件转换为相同协议。

```powershell
coding-pet.exe emit agent.turn.started --source codex --session turn-123
coding-pet.exe emit agent.input.required --source claude-code --message "需要批准"
coding-pet.exe emit agent.turn.completed --source cursor --session turn-123
```

目前仍需手动配置 Hook，一键安装尚在计划中。Codex 适配器、事件目录、HTTP 契约、安全机制和适配规则见[集成协议 v1](docs/integrations.md)。

## 宠物资源

CodingPet 自动扫描：

```text
~/.codex/pets/
```

```text
.codex/
└── pets/
    └── cat/
        ├── pet.json
        └── spritesheet.webp
```

主要格式为 Codex v2：1536×2288 的透明 PNG 或 WebP 图集，按 8 列×11 行排列。旧版 1536×1872 Codex v1 图集仍然可用。

```json
{
  "id": "my-pet",
  "displayName": "我的宠物",
  "description": "一个自定义桌面宠物",
  "spriteVersionNumber": 2,
  "spritesheetPath": "spritesheet.webp"
}
```

动画行包含待机、拖动、挥手、跳跃、失败、等待、工作、审查，以及 16 个顺时针鼠标注视方向。未知的 `pet.json` 字段会被忽略，以便向前兼容。

## 日常功能

- 透明、无边框、始终置顶的宠物窗口
- 拖动调整位置并播放方向动画
- 16 个方向的鼠标注视
- 多显示器与每显示器 DPI 支持
- Windows 系统托盘与 macOS 菜单栏控制
- 右键播放动画和切换宠物
- 可选环境动画和设置持久化

动画按以下优先级显示：

```text
拖动动画 > 鼠标注视 > 编程工具状态 > 环境动画
```

鼠标交互期间收到的 Agent 事件不会丢失，交互结束后会继续显示。

## 性能

CodingPet 面向常驻桌面的事件驱动场景设计。当前 Windows GDI 后端在一般使用中的典型内存占用约为 14 MB。

| 指标 | 当前目标 |
|---|---|
| 空闲 CPU | 约 0% |
| 动画 CPU | 小于 2% |
| Windows 常驻内存 | 小于 30 MB；典型约 14 MB |
| 启动 | 无启动画面和网络请求 |

具体数值会受平台、宠物资源、显示缩放和测量方式影响；以上是当前工程目标，并非完整的跨平台基准测试结果。

## 开发

### 构建

在 Windows 或 macOS 安装 Rust stable 工具链，然后运行：

```bash
cargo build --release
```

Windows 产物为 `target/release/coding-pet.exe`，macOS 产物为 `target/release/coding-pet`。GitHub Releases 另行提供 macOS 通用 `.app` 包。

### 配置

Windows 设置保存在 `%APPDATA%\CodingPet\config\config.json`。配置缺失或损坏时会自动恢复默认值。

### 架构

- 单线程、事件驱动的 Rust 核心
- 跨平台动画、宠物资源、配置和工具集成层
- Windows 使用 Win32 `UpdateLayeredWindow` 实现逐像素透明
- macOS 使用透明 AppKit 窗口和事件驱动的 Metal 表面
- 仅在动画帧变化时重绘

```text
src/
├── app/          # 生命周期、事件分发、状态和鼠标交互
├── animation/    # 动画播放和环境动画计时
├── integration/  # CLI、HTTP 服务、运行时和编程工具事件
├── pet/          # Codex 资源、共享模型和 Provider 契约
├── platform/     # Windows 与 macOS 原生窗口和渲染后端
├── ui/           # 托盘和设置界面
└── main.rs       # 进程入口
```

## 项目状态

当前版本：**v0.2.2**

目前已提供统一事件协议、通用发送器、Codex Hook 适配器、事件动画状态和设置界面。后续计划包括更简单的 Hook 安装、starter pet 初次体验、通知、休息提醒，以及可自定义的映射和适配器。

## 许可证

本项目采用 [Apache License 2.0](LICENSE)。
