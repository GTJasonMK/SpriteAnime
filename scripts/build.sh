#!/usr/bin/env bash
# ============================================================
# 一键打包脚本 — 清理 + 构建 + 输出产物清单
# 用法: ./scripts/build.sh [--clean|--release|--all]
# ============================================================
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUTPUT_DIR="$PROJECT_ROOT/output"
BUILD_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle"

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo -e "\n${CYAN}═══ $* ═══${NC}"; }

MODE="${1:---release}"

echo -e "${CYAN}"
echo "╔══════════════════════════════════════╗"
echo "║   SpriteAnimte — 生产构建           ║"
echo "╚══════════════════════════════════════╝"
echo -e "${NC}"

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
npx vite build 2>&1
info "前端构建完成 → dist/"

# ---- Rust Release 构建 ----
step "构建 Rust Release 二进制"
cd "$PROJECT_ROOT/src-tauri"
cargo build --release 2>&1 | tail -5
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
npx tauri build --bundles deb,rpm 2>&1 | grep -E "(Bundling|Finished|error|Bundle)" | tail -10 || true

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
