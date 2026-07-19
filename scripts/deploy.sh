#!/usr/bin/env bash
# ============================================================
# 一键部署脚本 — 检查环境、安装依赖、初始化项目
# 用法: ./scripts/deploy.sh
# ============================================================
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

if [ "$#" -ne 0 ]; then
    error "用法: ./scripts/deploy.sh"
    exit 1
fi

cleanup_on_interrupt() {
    warn "部署被中断，正在清理..."
    exit 130
}
trap cleanup_on_interrupt INT TERM

banner "SpriteAnimte — 一键部署"

# ---- 步骤1: 检查系统环境 ----
step "步骤 1/5: 检查系统环境"

check_cmd() {
    local cmd=$1 name=$2
    if command -v "$cmd" &>/dev/null; then
        local ver; ver=$("$cmd" --version 2>&1 | head -1)
        info "$name 已安装 → $ver"
    else
        error "缺少必需工具: $name (请先安装 $cmd)"
        exit 1
    fi
}

check_cmd node  "Node.js"
check_cmd npm   "npm"
check_cmd rustc "Rust"
check_cmd cargo "Cargo"

# 版本检查
NODE_MAJOR=$(node -v 2>/dev/null | sed 's/v//' | cut -d. -f1)
if [ "${NODE_MAJOR:-0}" -lt 18 ]; then
    error "Node.js 版本过低 (需要 ≥18)，当前: $(node -v)"
    exit 1
fi

RUSTC_VER=$(rustc --version 2>/dev/null | grep -oP '\d+\.\d+' | head -1)
if [ "${RUSTC_VER:-0}" = "0" ]; then
    error "无法解析 Rust 版本"
    exit 1
fi
info "Rust 版本: $RUSTC_VER"

# ---- 步骤2: 创建必要目录 ----
step "步骤 2/5: 创建目录结构"

mkdir -p "$PROJECT_ROOT/dist"
mkdir -p "$PROJECT_ROOT/logs"
touch "$PROJECT_ROOT/dist/.gitkeep"

ICON_DIR="$PROJECT_ROOT/src-tauri/icons"
for icon in 32x32.png 128x128.png 128x128@2x.png icon.ico icon.icns; do
    if [ ! -f "$ICON_DIR/$icon" ]; then
        error "缺少应用图标: src-tauri/icons/$icon"
        exit 1
    fi
done

# ---- 步骤3: 安装前端依赖 ----
step "步骤 3/5: 安装 Node.js 依赖"

cd "$PROJECT_ROOT"
npm ci --no-audit --no-fund 2>&1 | tail -5
info "Node.js 依赖安装完成"

# ---- 步骤4: 预编译 Rust 依赖 ----
step "步骤 4/5: 预编译 Rust 依赖（首次较慢）"

cd "$PROJECT_ROOT/src-tauri"
cargo fetch 2>&1 | tail -3 || { error "cargo fetch 失败"; exit 1; }
info "Rust 依赖下载完成"

# ---- 步骤5: 验证 ----
step "步骤 5/5: 验证部署"

PASS=0; FAIL=0

# 验证前端构建
cd "$PROJECT_ROOT"
if npm run build -- --logLevel error 2>&1; then
    info "前端构建验证通过"; PASS=$((PASS + 1))
else
    error "前端构建验证失败"; FAIL=$((FAIL + 1))
fi

# 验证 Rust 编译
cd "$PROJECT_ROOT/src-tauri"
if cargo check; then
    info "Rust 编译验证通过"; PASS=$((PASS + 1))
else
    error "Rust 编译验证失败"; FAIL=$((FAIL + 1))
fi

# ---- 结果 ----
echo ""
echo -e "${CYAN}══════════════════════════════════════${NC}"
if [ "$FAIL" -eq 0 ]; then
    echo -e "${GREEN}  部署成功! ($PASS/$((PASS + FAIL)) 项通过)${NC}"
    echo -e "  运行开发: ${YELLOW}./scripts/run.sh${NC}"
    echo -e "  运行测试: ${YELLOW}./scripts/test.sh${NC}"
    echo -e "  生产构建: ${YELLOW}./scripts/build.sh${NC}"
else
    echo -e "${RED}  部署失败 ($FAIL 项失败)${NC}"
    exit 1
fi
echo -e "${CYAN}══════════════════════════════════════${NC}"
