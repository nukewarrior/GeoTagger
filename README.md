# 照片地理标记工作台

一个本地优先的桌面工作台：将相机照片的拍摄时间与 GPX 轨迹匹配，在地图和时间轴中核验位置，再通过 ExifTool 把 GPS 安全写入照片副本。

当前实现依据 [`reference/照片地理标记工作台_App开发文档.docx`](reference/照片地理标记工作台_App开发文档.docx) 的 MVP-1 基线开发。

## 已实现范围

- Tauri 2 + Vue 3 + TypeScript + Pinia 桌面工程。
- 从欢迎页、四步导入到地图审核、写入确认、任务中心和设置的完整工作流。
- MapLibre 离线画布及应用自有的 GPX/照片 GeoJSON 图层；没有底图网络时仍可查看轨迹。
- WGS84、GCJ-02、BD-09 转换，保留原始坐标与 WGS84 标准化坐标。
- GPX 多文件、多 segment 解析、校验、哈希、统计与确定性排序。
- 照片扫描、ExifTool JSON 元数据读取、时区与固定相机时间偏差。
- 按 segment 二分查找、线性插值、置信度与异常状态；不会跨轨迹中断插值。
- schemaVersion 1 项目 JSON、原子保存、备份恢复和稳定 UUID。
- 写入计划、源文件指纹复核、独立输出目录、复制后写入、回读验证和单文件结果。
- CSV/JSON 报告、任务进度/取消、结构化错误和隐私保护提示。
- macOS Apple Silicon 与 Windows x64 的 GitHub Actions 构建；ExifTool 13.59 在 runner 中下载并校验后随包分发。

同步点漂移、PMTiles、RAW XMP Sidecar、SQLite、高德显示适配器和 Windows 正式签名不属于当前 MVP。

## 强制开发约束

本项目明确禁止在开发 Mac 上安装软件或项目依赖，也禁止在该机器上执行编译、构建、Lint、类型检查或测试。

因此不要在开发 Mac 上运行：

```text
npm install / npm ci / npm run ...
cargo check / cargo test / cargo build / cargo clippy
tauri build
brew install / pip install / cargo install
```

依赖解析、锁文件生成、质量门禁、测试、桌面构建、Sidecar 准备和发布全部由 GitHub Actions 执行。完整说明见 [`docs/ci.md`](docs/ci.md)。

## 第一次运行 GitHub Actions

由于锁文件也不能在本机生成，首次顺序是：

1. 将源码分支推送到 GitHub。
2. 在 Actions 中选择 `Bootstrap dependency lockfiles`，针对该分支运行。
3. 默认方式会创建只包含 `package-lock.json` 和 `src-tauri/Cargo.lock` 的 PR；也可选择直接提交当前分支。
4. 锁文件进入分支后，手动运行 `CI`。
5. CI 全绿后，运行 `Cross-platform desktop build` 获取 macOS ARM64 和 Windows x64 产物。

普通构建不读取签名秘密。正式 macOS 发布只在 `v*` tag 上运行，并要求受保护 Environment 中的 Apple 签名与公证 secrets 全部存在，否则会失败关闭。

## 架构

```text
Vue 3 / TypeScript / Pinia / MapLibre
                 │
          typed Tauri invoke
                 │
Rust commands + project/task application services
                 │
coordinate · GPX · matching · project JSON · file safety
                 │
     packaged ExifTool 13.59 resources
```

前端只负责交互与可视化，不拼接 Shell 命令。Rust 端校验类型、路径、文件格式、项目 revision 与写入范围；ExifTool 始终通过参数数组调用。

## 数据与安全原则

- 原照片默认只读，写入只发生在单独输出目录。
- 输出采用临时文件和原子重命名，失败不会破坏源文件。
- 写入前检查源文件 SHA-256、大小和修改时间；写入后重新读取 GPS 验证。
- 内部坐标和 EXIF GPS 始终使用 WGS84；地图显示不能覆盖标准化数据。
- 不上传照片、轨迹、缩略图或坐标；只有用户明确配置在线地图样式时才会产生地图网络请求。
- 诊断日志默认不记录精确坐标和完整私人路径。

## 目录

```text
src/                         Vue 工作台
src-tauri/src/               Rust 领域、应用与基础设施
src-tauri/resources/exiftool CI 注入的固定版本 Sidecar
tests/frontend/              Vue/Pinia 测试
.github/workflows/           锁文件、CI、构建和发布
.github/scripts/             Sidecar、校验、签名与产物脚本
docs/                        CI 与架构说明
reference/                   产品开发文档
```

## 许可证

[MIT](LICENSE)
