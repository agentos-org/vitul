# Vitul 的 macOS 应用图标

`vitul icon` 会从同一份原始素材生成各平台资源。macOS `.icns` 单独经过适配，Windows、Linux、Android、iOS、AgentOS、托盘及应用内 Logo 继续使用原有输出。

## 默认行为

运行：

```sh
vitul icon ./app-icon.png
```

Vitul 保留已有平台图标生成行为，并为 macOS 额外生成：

- `icon-macos-master.png`：1024 × 1024 的 macOS 母版；
- `icon.icns`：包含 16、32、64、128、256、512 和 1024 像素档位及 Retina 条目的 ICNS；
- `icon.icns.vitul.json`：源文件、SHA-256、适配模式、构图参数、模板参照和生成器版本。

母版和 ICNS 都写到图标输出目录，不覆盖输入文件。后续运行桌面构建时，Vitul 会把最新生成的 ICNS 投影到桌面配置。素材或参数变化时会重新生成；未变化时复用现有产物。

3.5.0 新建的项目会显式配置 `bundle.macosIcon`，从根目录的 `app-icon.png` 自动派生 macOS 图标，因此模板自带的 ICNS 不会阻止适配。已有来源记录的图标优先使用其中仍可访问的原始素材；只有原始路径不可用时才回退到 `bundle.icon` 中的 PNG/SVG。显式设置的 `source` 始终优先。

自动模式把方形图稿整体放进 macOS 圆角底板轮廓，图稿背景与 Logo 一起缩放。若需要独立控制底板和前景 Logo，显式将素材声明为 Logo：

```sh
vitul icon ./logo.png --macos-kind logo \
  --macos-background '#ffffff' --macos-logo-scale 72
```

这会用指定颜色填充底板，并将前景 Logo 缩放至底板边长的 72%。Vitul 不会根据 Alpha 通道猜测素材是否已适配，也不会尝试从一张扁平 PNG 中自动抠出 Logo。

## 跳过适配

已带透明边距、专用轮廓或 macOS 构图的 PNG/SVG 可直接保留原样：

```sh
vitul icon ./macos-icon.png --macos-mode passthrough
```

桌面构建也可在根目录 `vitul.conf.json` 中指定：

```json
{
  "bundle": {
    "icon": ["src-vitul/icons/app-icon.png"],
    "macosIcon": {
      "mode": "passthrough"
    }
  }
}
```

`bundle.macosIcon` 是 Vitul 专用配置；投影到桌面宿主时会移除它，并将源路径和派生 ICNS 路径转换为 `src-vitul/` 下的宿主相对路径。

要强制重新适配现有 PNG 素材，可设置 `mode` 为 `auto`。也可显式指定 `source`、`kind`、`background` 和 `logoScale`：

```json
{
  "bundle": {
    "icon": ["src-vitul/icons/app-icon.png"],
    "macosIcon": {
      "mode": "auto",
      "source": "src-vitul/icons/app-icon.png",
      "kind": "logo",
      "background": "#f2f2f2",
      "logoScale": 72
    }
  }
}
```

配置中的相对路径以根目录 `vitul.conf.json` 为基准。`kind` 支持 `square` 和 `logo`；`logoScale` 范围为 1–100。

## 自定义 ICNS 和旧项目

项目在 `bundle.icon` 中提供了自己的 `.icns` 时，Vitul 默认直接交给 macOS 打包器使用。只有显式配置 `bundle.macosIcon.mode: "auto"` 才会从 PNG/SVG 重新生成。

旧项目只有 PNG/SVG 时，桌面构建默认自动生成 macOS ICNS。旧项目若同时列有 PNG 和手工维护的 `.icns`，为兼容旧行为仍会优先保留该 ICNS。要迁移到自动适配，在根目录配置中明确启用 `auto`：

```json
{
  "bundle": {
    "icon": ["src-vitul/icons/app-icon.png", "src-vitul/icons/old-icon.icns"],
    "macosIcon": { "mode": "auto", "kind": "square" }
  }
}
```

Vitul 会从 PNG/SVG 生成并投影新的 ICNS，旧 ICNS 不再进入桌面宿主配置；源文件仍保留。只想沿用手工图标时，删去 `macosIcon` 配置并继续引用自定义 `.icns`，或为 PNG 设置 `mode: "passthrough"`。

不要把生成的 `icon-macos-master.png` 当作下一次生成的输入；每次都应使用原始素材。来源记录存放在 `icon.icns.vitul.json`，包含绝对源路径和源文件哈希，便于检查图标来自哪里以及是否需要重建。

## 几何与 Apple 图标流程

ICNS 的静态适配轮廓使用当前 macOS 随系统提供的 Calculator `AppIcon.icns` 作兼容参照：在 macOS 27.0.1 的 256 × 256 ICNS 条目中，alpha 轮廓约从第 25 像素延伸至第 230 像素；按 4 倍映射到 1024 母版后，轮廓边长为 824 像素，居中留 100 像素透明边距，圆角半径按参考图约 44 像素映射为 176 像素。轮廓以解析圆角计算并逐像素抗锯齿，避免低分辨率圆角贴图被放大。

这组数值来自 Apple 随系统提供的 ICNS 资源实测，用于传统静态 ICNS 的视觉兼容；Apple 的 Human Interface Guidelines 没有公布一个适用于所有图稿的固定圆角半径或内容缩放比例。指南目前建议为 Icon Composer 提供未预先裁切的方形图层，由系统套用遮罩。Vitul 因此不把上述 ICNS 兼容参数称作 Apple HIG 强制规范。若 Apple 更新系统轮廓，生成记录和几何版本也应随之更新。

仓库的 macOS 打包器已经能接收用户提供的 `.icon` 目录，并用 `actool` 编译成 `Assets.car`；也能直接复制用户提供的 `Assets.car`。这条路径需要 macOS 上的 Xcode 工具，现有代码要求 `actool` 主版本至少为 26。Icon Composer 的分层、外观和 Liquid Glass 效果由该格式表达；传统 `.icns` 是静态多分辨率回退，不能保存这些动态属性。Vitul 目前不从单张 PNG 自动构造 Icon Composer 项目。

Apple 文档：[App icons HIG](https://developer.apple.com/design/human-interface-guidelines/app-icons)、[Icon Composer](https://developer.apple.com/documentation/Xcode/creating-your-app-icon-using-icon-composer)、[Apple Design Resources](https://developer.apple.com/design/resources/)。

## 验证 macOS 安装包

检查生成物时，可以先查看 `icon-macos-master.png` 的透明边距，再解包 ICNS 并确认每个尺寸：

```sh
iconutil -c iconset path/to/icon.icns -o /tmp/vitul-iconset
file /tmp/vitul-iconset/*.png
```

真实安装验证应在新的 `.app` 路径或新的 Bundle ID 下执行，并分别查看 Finder 与 Dock。若生成文件尺寸、alpha 轮廓和 `.app/Contents/Resources/*.icns` 均正确，但界面仍显示旧图标，应先退出应用、重启 Dock/Finder 或更换 Bundle ID，再判断是否为 Launch Services 图标缓存；不要仅凭旧缓存画面判定产物错误。
