# Toney Music：Vitulfile 示例

此目录演示如何用 Vitulfile 打包 Toney Music 进程应用：保留现有
React/Vite 音乐界面，配套一个 Rust 音频服务。它不是 Tauri 或其他 Rust 桌面应用。

React/Vite 前端只负责控制界面。Rust `musicd` 进程管理媒体库、解码、播放状态和后台播放，
并通过 `AGENTOS_AUDIO_SOCKET` 把原始 PCM 直接提交给 AgentOS 宿主音频 broker；CEF 不负责
解码或播放音频流。

## 解码和输出路径

- Claxon（Rust）按 FLAC 源采样精度解码 PCM 样本。
- Symphonia（Rust）解码 WAV、AIFF、ALAC/M4A、MP3、AAC 和 Vorbis。
- `ape-decoder`（Rust）将 APE 解码为原生整数 PCM。
- `libwavpack` 将 WavPack 解码为整数 PCM。
- 原生解码器不支持或解码失败时，使用 FFmpeg 兜底。兜底输出为 32 位 PCM，状态会标记为格式
  转换路径，不会宣称 bit-perfect。
- MP3、AAC 等有损格式无法还原编码前的样本；DSD 转 PCM 也需要滤波处理。

对于受支持的无损音源，`musicd` 解码时保留源采样率、声道数和样本精度，不加增益、不重采样、
不抖动。AgentOS 独占 ALSA 模式遇到硬件不支持的格式时会拒绝打开。ALSA 接受后，AgentOS
仍无法检查设备内部 DSP，因此独占输出会显示为“未验证”。共享 PipeWire 可能混音或重采样，
会标记为非 bit-perfect。

## 构建

构建需要：

- Vitul CLI、Node.js 和 pnpm
- Rust stable 与 Cargo
- 用于 libwavpack 绑定的 CMake 和 C++ 编译器
- 目标架构与构建机不同时需要 Docker
- 打包 FFmpeg 兜底解码器需要 Linux ARM64 版 FFmpeg 可执行文件

将 `FFMPEG_BIN` 设为 Linux ARM64 FFmpeg 的路径后运行：

```sh
export FFMPEG_BIN=/path/to/linux-arm64/ffmpeg
vitul agentos validate
vitul build --target agentos/arm64
```

Rust 构建脚本在跨平台构建时使用 Linux 容器，并读取 `VITUL_TARGET_ARCH`。当前
`Vitulfile` 目标为 `agentos/arm64`。Vitul 会构建前端和 `musicd`，将 FFmpeg 作为兜底一起打包，
并生成 `dist/toney-music-0.3.10-agentos-arm64.agt`。

生成的进程 manifest 声明 `native.audio`、`native.audio.exclusive` 和 `native.tray`。AgentOS 启动应用时会提供
`AGENTOS_AUDIO_SOCKET` 和 `AGENTOS_TRAY_SOCKET`。`musicd` 后端使用打包的 Toney Music PNG 作为托盘图标，
并动态更新曲目/播放状态提示和菜单；菜单提供播放/暂停、停止操作。关闭 CEF 窗口后，`musicd`、音频会话和
托盘控制仍会运行。菜单项提供 AgentOS 约定的 `localizedLabels`，支持 `en-US` 和 `zh-CN`；Shell 按当前语言选择文案，
没有匹配时回退到 `label`。悬停提示随 Toney 界面语言更新，并保存在应用数据目录中。停止或卸载应用时，AgentOS 会结束播放并移除托盘图标。

## CUE 分轨

导入时同时选择一个 `.cue` 文件和它引用的单个受支持的整碟音频文件。Toney 会根据
`TRACK ... AUDIO` 条目建立独立歌曲，并读取专辑、艺术家、歌名和 `INDEX 01` 起点。支持
UTF-8、GB18030 和 UTF-16 编码；多音频文件 CUE 和数据轨会提示导入错误。

整碟音频仍只保留为一个源文件。各首歌共用解码后的 PCM 缓存，并按采样帧区间播放，不会拆分或重编码源文件。
删除一首歌时，只要还有其他 CUE 歌曲引用该源文件，就会保留它。浏览器选择器和拖放需要同时选择 CUE 与音频文件；
Fenn Files 不会把未选择的同目录文件提供给应用。关闭 UI 后，`musicd` 会按曲目编号继续播放当前 CUE 专辑。
音乐库和解码 PCM 缓存保存在 AgentOS 注入的 `DATA_DIR` 下；解码缓存可能比压缩后的源文件大。
