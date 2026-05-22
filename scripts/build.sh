#!/usr/bin/env bash
# ============================================================
# 一键打包脚本 — 清理 + 构建 + 输出产物清单
# 用法: ./scripts/build.sh [--clean|--release|--all]
# ============================================================
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

OUTPUT_DIR="$PROJECT_ROOT/output"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle"

MODE="${1:---release}"

banner "SpriteAnimte - 生产构建"

cd "$PROJECT_ROOT"

# ---- 清理 ----
if [ "$MODE" = "--clean" ] || [ "$MODE" = "--all" ]; then
    step "清理构建产物"
    rm -rf "$OUTPUT_DIR"
    rm -rf "$PROJECT_ROOT/dist"
    mkdir -p "$PROJECT_ROOT/dist"
    touch "$PROJECT_ROOT/dist/.gitkeep"
    info "清理完成"
fi

# ---- 前端构建 ----
step "构建前端 (Vite)"
run_vite_build 0
info "前端构建完成 → dist/"

# ---- Rust Release 构建 ----
step "构建 Rust Release 二进制"
run_cargo_build release 5
BINARY="$PROJECT_ROOT/src-tauri/target/release/sprite-anime"
if [ -f "$BINARY" ]; then
    SIZE=$(du -h "$BINARY" | cut -f1)
    info "二进制构建完成 → $SIZE"
else
    error "二进制构建失败"
    exit 1
fi

# ---- Tauri Bundle ----
step "打包 Tauri 安装包"
cd "$PROJECT_ROOT"
mkdir -p "$PROJECT_ROOT/logs"
rm -rf "$BUILD_DIR"
TAURI_BUILD_LOG="$PROJECT_ROOT/logs/tauri-build.log"
if npx tauri build --bundles deb,rpm 2>&1 | tee "$TAURI_BUILD_LOG"; then
    grep -E "(Bundling|Finished|error|Bundle)" "$TAURI_BUILD_LOG" | tail -10 || true
else
    error "Tauri 打包失败，详见 logs/tauri-build.log"
    exit 1
fi

# ---- 收集产物 ----
step "收集构建产物"
mkdir -p "$OUTPUT_DIR"
rm -rf "$OUTPUT_DIR"/*

# 复制二进制
cp "$BINARY" "$OUTPUT_DIR/sprite-anime"
info "二进制: output/sprite-anime"

# 复制 deb
if [ -d "$BUILD_DIR/deb" ]; then
    cp "$BUILD_DIR/deb"/*.deb "$OUTPUT_DIR/" 2>/dev/null || true
    DEB_FILE=$(ls "$OUTPUT_DIR"/*.deb 2>/dev/null | head -1)
    if [ -n "$DEB_FILE" ]; then
        info "DEB 包: $(basename "$DEB_FILE") ($(du -h "$DEB_FILE" | cut -f1))"
    fi
fi

# 复制 rpm
if [ -d "$BUILD_DIR/rpm" ]; then
    cp "$BUILD_DIR/rpm"/*.rpm "$OUTPUT_DIR/" 2>/dev/null || true
    RPM_FILE=$(ls "$OUTPUT_DIR"/*.rpm 2>/dev/null | head -1)
    if [ -n "$RPM_FILE" ]; then
        info "RPM 包: $(basename "$RPM_FILE") ($(du -h "$RPM_FILE" | cut -f1))"
    fi
fi

# 复制 AppImage（如果存在）
if [ -d "$BUILD_DIR/appimage" ]; then
    cp "$BUILD_DIR/appimage"/*.AppImage "$OUTPUT_DIR/" 2>/dev/null || true
    AI_FILE=$(ls "$OUTPUT_DIR"/*.AppImage 2>/dev/null | head -1)
    if [ -n "$AI_FILE" ]; then
        info "AppImage: $(basename "$AI_FILE") ($(du -h "$AI_FILE" | cut -f1))"
    fi
fi

# ---- 生成版本信息 ----
GIT_HASH=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME=$(date '+%Y-%m-%d %H:%M:%S')
{
    echo "SpriteAnimte 构建信息"
    echo "====================="
    echo "版本: 0.1.0"
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
