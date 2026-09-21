#!/usr/bin/env bash
# 构建 Blade².app(macOS 原生壳)。
#
# 用法:
#   scripts/build-app.sh                  # 构建可执行 + .app 壳(不打包内核)
#   scripts/build-app.sh --with-kernel    # 额外把内核树 + node 二进制打进 Resources/Kernel
#                                         # (自包含形态,对应 Win 版 MSIX 的 Kernel/ 布局)
#
# 版本契约:mac/VERSION 是唯一版本来源,写入 Info.plist 的 CFBundleShortVersionString;
# CFBundleVersion 用去点并去掉前导零的数字(0.7.7.1 → 771),满足 Apple "CFBundleVersion 须为数字"的要求。
set -euo pipefail

cd "$(dirname "$0")/.."

WITH_KERNEL=0
[ "${1:-}" = "--with-kernel" ] && WITH_KERNEL=1

# 版本单一来源:mac/VERSION。缺失即失败,不允许静默打出无版本号的包。
VERSION="$(tr -d '[:space:]' < VERSION)"
[ -n "$VERSION" ] || { echo "error: mac/VERSION 为空或缺失" >&2; exit 1; }
BUILD_NUMBER="$((10#${VERSION//./}))"

echo "==> swift build (release)"
swift build -c release

APP="build/Blade2.app"
CONTENTS="$APP/Contents"
BIN_PATH="$(swift build -c release --show-bin-path)"
BIN="$BIN_PATH/DshMacUI"

echo "==> 组装 $APP (version $VERSION)"
rm -rf "$APP"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

cp "$BIN" "$CONTENTS/MacOS/DshMacUI"

# 资源:SPM 资源包放进 Resources 让 Bundle.module 可被发现。
# glob 全拷而不是硬编码包名 —— SwiftPM 生成的 bundle 名带 target 前缀
# (i18n 资源在 DshMacUI_Blade2Core.bundle,不在 DshMacUI_DshMacUI.bundle),对改名不再敏感。
for BUNDLE in "$BIN_PATH"/*.bundle; do
  [ -d "$BUNDLE" ] && cp -R "$BUNDLE" "$CONTENTS/Resources/"
done

# 应用图标:仓库内生成好的 icns(源是 Assets 品牌图,iconutil 制作),缺失即失败。
[ -f "Resources/AppIcon.icns" ] || { echo "error: 缺少 mac/Resources/AppIcon.icns" >&2; exit 1; }
cp "Resources/AppIcon.icns" "$CONTENTS/Resources/AppIcon.icns"

# Mac 壳不发送 Apple Events,故无 NSAppleEventsUsageDescription。
cat > "$CONTENTS/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key><string>zh-Hans</string>
    <key>CFBundleDisplayName</key><string>Blade²</string>
    <key>CFBundleExecutable</key><string>DshMacUI</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundleIdentifier</key><string>com.dsh.blade2.macos</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>CFBundleName</key><string>Blade²</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key><string>${BUILD_NUMBER}</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
    <key>LSMinimumSystemVersion</key><string>14.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>NSSupportsAutomaticTermination</key><false/>
</dict>
</plist>
PLIST

# 内核打包(自包含形态)
if [ "$WITH_KERNEL" = "1" ]; then
  echo "==> 打包内核到 Resources/Kernel"
  KERNEL_SRC="$HOME/Library/Application Support/Blade2/Kernel/dsh"
  if [ ! -f "$KERNEL_SRC/lib/bin.js" ]; then
    echo "error: 未找到已准备的内核,先运行 scripts/prepare-kernel.sh" >&2
    exit 1
  fi
  mkdir -p "$CONTENTS/Resources/Kernel"
  # 排除 win32 预编译目录,减小体积(内核代码原样不动)
  rsync -a --exclude 'node_modules/@img/sharp-win32-x64' \
        --exclude 'node_modules/@koromix/koffi-win32-x64' \
        --exclude 'node_modules/node-addon-require-builtin-win32-x64-msvc' \
        --exclude 'node_modules/node-pty/prebuilds/win32-*' \
        --exclude 'node_modules/node-pty/prebuilds/linux-*' \
        "$KERNEL_SRC/" "$CONTENTS/Resources/Kernel/dsh/"
  NODE_SRC="$(/usr/bin/env node -p 2>/dev/null || echo /opt/homebrew/bin/node)"
  if [ -x "$NODE_SRC" ] && [ ! -d "$NODE_SRC" ]; then
    cp "$NODE_SRC" "$CONTENTS/Resources/Kernel/node"
  else
    cp /opt/homebrew/bin/node "$CONTENTS/Resources/Kernel/node"
  fi
fi

echo "==> ad-hoc 签名"
codesign --force --deep --sign - "$APP" 2>/dev/null || true

echo ""
echo "完成:"
echo "  open $APP          # 启动"
echo "  也可先 scripts/prepare-kernel.sh 部署内核(--with-kernel 之外形态的默认解析路径)"
