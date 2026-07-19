#!/usr/bin/env bash
# ============================================================
# 一键打包脚本 — 清理 + 构建 + 输出产物清单
# 用法: ./scripts/build.sh [--clean|--release]
# ============================================================
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

OUTPUT_DIR="$PROJECT_ROOT/output"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle"

MODE="${1:---release}"
if [ "$#" -gt 1 ]; then
    error "用法: ./scripts/build.sh [--clean|--release]"
    exit 1
fi
case "$MODE" in
    --clean|--release) ;;
    *)
        error "用法: ./scripts/build.sh [--clean|--release]"
        exit 1
        ;;
esac

banner "SpriteAnimte - 生产构建"

cd "$PROJECT_ROOT"

# ---- 清理 ----
if [ "$MODE" = "--clean" ]; then
    step "清理构建产物"
    rm -rf "$OUTPUT_DIR"
    rm -rf "$PROJECT_ROOT/dist"
    mkdir -p "$PROJECT_ROOT/dist"
    touch "$PROJECT_ROOT/dist/.gitkeep"
    info "清理完成"
fi

# ---- Tauri Bundle ----
step "打包 Tauri 安装包"
cd "$PROJECT_ROOT"
mkdir -p "$PROJECT_ROOT/logs"
rm -rf "$BUILD_DIR"
node "$PROJECT_ROOT/scripts/prepare-cli-sidecar.mjs" --release
TAURI_BUILD_LOG="$PROJECT_ROOT/logs/tauri-build.log"
if npx tauri build -c src-tauri/tauri.sidecar.conf.json --bundles deb,rpm,appimage 2>&1 | tee "$TAURI_BUILD_LOG"; then
    grep -E "(Bundling|Finished|error|Bundle)" "$TAURI_BUILD_LOG" | tail -10 || true
else
    error "Tauri 打包失败，详见 logs/tauri-build.log"
    exit 1
fi

BINARY="$PROJECT_ROOT/src-tauri/target/release/sprite-anime"
CLI_BINARY="$PROJECT_ROOT/src-tauri/target/release/sprite-anime-cli"
if [ ! -f "$BINARY" ]; then
    error "Tauri 构建完成但缺少 release 二进制"
    exit 1
fi
if [ ! -f "$CLI_BINARY" ]; then
    error "CLI 构建完成但缺少 release 二进制"
    exit 1
fi
SIZE=$(du -h "$BINARY" | cut -f1)
info "二进制构建完成 → $SIZE"

for bundle_dir in deb rpm appimage; do
    if [ ! -d "$BUILD_DIR/$bundle_dir" ]; then
        error "Tauri 构建完成但缺少 $bundle_dir 产物目录"
        exit 1
    fi
done

step "修正 AppImage 启动环境"
bash "$PROJECT_ROOT/scripts/postprocess-appimage.sh" "$BUILD_DIR/appimage"

# ---- 收集产物 ----
step "收集构建产物"
mkdir -p "$OUTPUT_DIR"
rm -rf "$OUTPUT_DIR"/*

# 复制二进制
cp "$BINARY" "$OUTPUT_DIR/sprite-anime"
info "二进制: output/sprite-anime"
cp "$CLI_BINARY" "$OUTPUT_DIR/sprite-anime-cli"
tar -czf "$OUTPUT_DIR/sprite-anime-cli-linux-$(uname -m).tar.gz" \
    -C "$OUTPUT_DIR" sprite-anime-cli
info "CLI: output/sprite-anime-cli"

copy_bundle_artifacts() {
    local bundle_dir=$1 pattern=$2 label=$3
    local files=("$BUILD_DIR/$bundle_dir"/$pattern)
    if [ "${#files[@]}" -eq 0 ]; then
        error "$bundle_dir 目录中缺少 $pattern 产物"
        exit 1
    fi
    cp "${files[@]}" "$OUTPUT_DIR/"
    for file in "${files[@]}"; do
        info "$label: $(basename "$file") ($(du -h "$file" | cut -f1))"
    done
}

shopt -s nullglob
copy_bundle_artifacts deb "*.deb" "DEB 包"
copy_bundle_artifacts rpm "*.rpm" "RPM 包"
copy_bundle_artifacts appimage "*.AppImage" "AppImage"
shopt -u nullglob

# ---- 生成版本信息 ----
GIT_HASH=$(git rev-parse --short HEAD)
VERSION=$(node -e "console.log(require(process.argv[1]).version)" "$PROJECT_ROOT/src-tauri/tauri.conf.json")
BUILD_TIME=$(date '+%Y-%m-%d %H:%M:%S')
{
    echo "SpriteAnimte 构建信息"
    echo "====================="
    echo "版本: $VERSION"
    echo "构建时间: $BUILD_TIME"
    echo "Git Commit: $GIT_HASH"
    echo "Rust: $(rustc --version)"
    echo "Node: $(node --version)"
    echo "平台: $(uname -m)"
} > "$OUTPUT_DIR/BUILD_INFO.txt"

# ---- 汇总 ----
echo ""
echo -e "${CYAN}══════════════════════════════════════${NC}"
echo -e "${GREEN}  构建完成!${NC}"
echo -e "${CYAN}──────────────────────────────────────${NC}"
echo -e "  产物目录: ${YELLOW}$OUTPUT_DIR${NC}"
echo ""
ls -lh "$OUTPUT_DIR" | tail -n +2 | while read -r line; do
    echo "    $line"
done
echo -e "${CYAN}══════════════════════════════════════${NC}"
