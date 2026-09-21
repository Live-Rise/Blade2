#!/usr/bin/env bash
# 部署 macOS 可用的 dsh 内核副本(不改内核代码,只补平台原生依赖)。
#
# 用法: scripts/prepare-kernel.sh [内核树目标目录]
#   默认目标: ~/Library/Application Support/Blade2/Kernel/dsh
#
# 步骤:
#   1. 从仓库 Kernel/dsh 复制内核树(JS 跨平台;node_modules 里的 win32 预编译无害)
#   2. 按 sharp / koffi 各自的 registry 元数据,补 darwin-arm64 平台包
#      (等价于内核报错信息给出的 `npm install --os=darwin --cpu=arm64`,
#       但绕开全树 npm 解析 —— 树里含有 registry 拉不到的私有 devDep)
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$REPO_ROOT/../Kernel/dsh"
DEST="${1:-$HOME/Library/Application Support/Blade2/Kernel/dsh}"
ARCH="arm64"

if [ ! -f "$SRC/lib/bin.js" ]; then
  echo "error: 找不到 $SRC/lib/bin.js" >&2
  exit 1
fi

fetch_version() { # $1=package name → latest-满足内核需求的版本(与树内 win32 包同版本)
  local pkg="$1" probe="$2"
  local v
  v=$(node -e "try{console.log(require('$DEST/node_modules/$probe/package.json').version)}catch(e){console.log('')}")
  if [ -z "$v" ]; then v=$(curl -s "https://registry.npmjs.org/$pkg" | node -e "let d='';process.stdin.on('data',c=>d+=c).on('end',()=>{const j=JSON.parse(d);console.log(j['dist-tags'].latest)})"); fi
  echo "$v"
}

extract() { # $1=pkg $2=version $3=dest-under-node_modules
  local pkg="$1" ver="$2" sub="$3"
  mkdir -p "$DEST/node_modules/$sub"
  local url
  url=$(curl -s "https://registry.npmjs.org/$(echo "$pkg" | sed 's|/|%2f|g')" | node -e "let d='';process.stdin.on('data',c=>d+=c).on('end',()=>{const j=JSON.parse(d);console.log(j.versions['$2'].dist.tarball)})")
  curl -sL "$url" | tar -xz -C "$DEST/node_modules/$sub" --strip-components=1
  echo "  + $sub@$ver"
}

echo "==> 复制内核树"
mkdir -p "$(dirname "$DEST")"
if [ ! -f "$DEST/lib/bin.js" ]; then
  rm -rf "$DEST"
  cp -R "$SRC" "$DEST"
else
  echo "  (已存在,跳过复制;如需重装请先删除 $DEST)"
fi

echo "==> 补 darwin-$ARCH 平台依赖(不动内核代码)"
# sharp = @img/sharp-darwin-arm64 + @img/sharp-libvips-darwin-arm64(darwin 版 libvips 独立分发)
if [ ! -f "$DEST/node_modules/@img/sharp-darwin-$ARCH/lib/sharp-darwin-$ARCH.node" ]; then
  V_SHARP=$(fetch_version "@img/sharp-darwin-$ARCH" "@img/sharp-win32-x64")
  extract "@img/sharp-darwin-$ARCH" "$V_SHARP" "@img/sharp-darwin-$ARCH"
  extract "@img/sharp-libvips-darwin-$ARCH" "1.3.3" "@img/sharp-libvips-darwin-$ARCH"
else
  echo "  (sharp-darwin 已就绪)"
fi
# koffi = @koromix/koffi-darwin-arm64
if [ ! -f "$DEST/node_modules/@koromix/koffi-darwin-$ARCH/darwin_$ARCH/koffi.node" ]; then
  V_KOFFI=$(fetch_version "@koromix/koffi-darwin-$ARCH" "@koromix/koffi-win32-x64")
  extract "@koromix/koffi-darwin-$ARCH" "$V_KOFFI" "@koromix/koffi-darwin-$ARCH"
else
  echo "  (koffi-darwin 已就绪)"
fi

echo "==> 启动自检(8 秒)"
DSH_HOME="$(mktemp -d)" /opt/homebrew/bin/node --expose-internals "$DEST/lib/bin.js" web --no-open --port 0 > /tmp/blade2-kernel-check.log 2>&1 &
PID=$!
sleep 8
kill $PID 2>/dev/null || true
if grep -q "dsh web: http" /tmp/blade2-kernel-check.log; then
  echo "==> 内核自检通过:$(grep -o 'dsh web: http[^ ]*' /tmp/blade2-kernel-check.log | head -1 | sed 's/?.*//')…"
else
  echo "==> 警告:未捕获 URL,请检查 /tmp/blade2-kernel-check.log" >&2
  exit 1
fi
