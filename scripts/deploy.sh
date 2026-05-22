#!/usr/bin/env bash
# ============================================================
# 一键部署脚本 — 检查环境、安装依赖、初始化项目
# 用法: ./scripts/deploy.sh [--force]
# ============================================================
set -euo pipefail

# 颜色
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; CYAN='\033[0;36m'; NC='\033[0m'

FORCE="${1:-}"
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
LOG_DIR="$PROJECT_ROOT/logs"

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo -e "\n${CYAN}═══ $* ═══${NC}"; }

cleanup_on_interrupt() {
    warn "部署被中断，正在清理..."
    exit 130
}
trap cleanup_on_interrupt INT TERM

echo -e "${CYAN}"
echo "╔══════════════════════════════════════╗"
echo "║   SpriteAnimte — 一键部署           ║"
echo "╚══════════════════════════════════════╝"
echo -e "${NC}"

# ---- 步骤1: 检查系统环境 ----
step "步骤 1/5: 检查系统环境"

check_cmd() {
    local cmd=$1 name=$2 required=$3
    if command -v "$cmd" &>/dev/null; then
        local ver; ver=$("$cmd" --version 2>&1 | head -1 || echo "未知版本")
        info "$name 已安装 → $ver"
    else
        if [ "$required" = "true" ]; then
            error "缺少必需工具: $name (请先安装 $cmd)"
            exit 1
        else
            warn "可选工具未安装: $name"
        fi
    fi
}

check_cmd node    "Node.js"   true
check_cmd npm     "npm"       true
check_cmd rustc   "Rust"      true
check_cmd cargo   "Cargo"     true
check_cmd python3 "Python3"   false  # 用于图标生成等辅助操作

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
mkdir -p "$PROJECT_ROOT/src-tauri/icons"
touch "$PROJECT_ROOT/dist/.gitkeep"

# 检查图标文件，缺失则自动生成
ICON_DIR="$PROJECT_ROOT/src-tauri/icons"
if [ ! -f "$ICON_DIR/32x32.png" ] || [ "$FORCE" = "--force" ]; then
    info "生成占位图标..."
    python3 -c "
import struct, zlib
COLOR = (0xc6, 0x61, 0x3f, 0xff)
def rgba_png(w, h, path):
    def ch(t, d):
        c = t + d
        return struct.pack('>I', len(d)) + c + struct.pack('>I', zlib.crc32(c) & 0xffffffff)
    hdr = b'\\x89PNG\\r\\n\\x1a\\n' + ch(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 6, 0, 0, 0))
    raw = b''
    for _ in range(h):
        raw += b'\\x00' + bytes(COLOR) * w
    dat = ch(b'IDAT', zlib.compress(raw))
    with open(path, 'wb') as f: f.write(hdr + dat + ch(b'IEND', b''))

def ico_bitmap(w, h):
    r, g, b, a = COLOR
    row = bytes((b, g, r, a)) * w
    pixels = row * h
    header = struct.pack('<IIIHHIIIIII', 40, w, h * 2, 1, 32, 0, len(pixels), 0, 0, 0, 0)
    mask_stride = ((w + 31) // 32) * 4
    return header + pixels + (b'\\x00' * mask_stride * h)

def write_ico(path, sizes):
    images = [ico_bitmap(sz, sz) for sz in sizes]
    header = struct.pack('<HHH', 0, 1, len(images))
    offset = 6 + 16 * len(images)
    entries = []
    for sz, data in zip(sizes, images):
        width_byte = 0 if sz >= 256 else sz
        entries.append(struct.pack('<BBBBHHII', width_byte, width_byte, 0, 0, 1, 32, len(data), offset))
        offset += len(data)
    with open(path, 'wb') as f:
        f.write(header + b''.join(entries) + b''.join(images))

def write_icns(path, items):
    chunks = []
    for kind, png_path in items:
        data = open(png_path, 'rb').read()
        chunks.append(kind.encode('ascii') + struct.pack('>I', len(data) + 8) + data)
    body = b''.join(chunks)
    with open(path, 'wb') as f:
        f.write(b'icns' + struct.pack('>I', len(body) + 8) + body)

for sz, nm in [(32,'32x32.png'),(128,'128x128.png'),(256,'128x128@2x.png')]:
    rgba_png(sz, sz, f'$ICON_DIR/{nm}')
write_ico(f'$ICON_DIR/icon.ico', [32, 128, 256])
write_icns(f'$ICON_DIR/icon.icns', [
    ('icp5', f'$ICON_DIR/32x32.png'),
    ('ic07', f'$ICON_DIR/128x128.png'),
    ('ic08', f'$ICON_DIR/128x128@2x.png'),
])
" 2>/dev/null || warn "图标生成失败（非致命，使用 ImageMagick 备选）"
fi

# ---- 步骤3: 安装前端依赖 ----
step "步骤 3/5: 安装 Node.js 依赖"

cd "$PROJECT_ROOT"
if [ -d "node_modules" ] && [ "$FORCE" != "--force" ]; then
    info "node_modules 已存在，跳过安装（使用 --force 强制重装）"
else
    [ "$FORCE" = "--force" ] && rm -rf node_modules package-lock.json
    npm install --no-audit --no-fund 2>&1 | tail -5
    info "Node.js 依赖安装完成"
fi

# ---- 步骤4: 预编译 Rust 依赖 ----
step "步骤 4/5: 预编译 Rust 依赖（首次较慢）"

cd "$PROJECT_ROOT/src-tauri"
cargo fetch 2>&1 | tail -3 || { error "cargo fetch 失败"; exit 1; }
info "Rust 依赖下载完成"

# 预先 check 一次以编译依赖（后续运行更快）
if [ "$FORCE" = "--force" ] || [ ! -f "target/debug/sprite-anime" ]; then
    info "首次编译中（约需 2-5 分钟）..."
    cargo check 2>&1 | tail -5
    info "Rust 预编译完成"
else
    info "Rust 已编译，跳过（使用 --force 强制重编）"
fi

# ---- 步骤5: 验证 ----
step "步骤 5/5: 验证部署"

PASS=0; FAIL=0

# 验证前端构建
cd "$PROJECT_ROOT"
if npx vite build --logLevel error 2>&1; then
    info "前端构建验证通过"; PASS=$((PASS + 1))
else
    error "前端构建验证失败"; FAIL=$((FAIL + 1))
fi

# 验证 Rust 编译
cd "$PROJECT_ROOT/src-tauri"
if cargo check 2>&1 | tail -1 | grep -q "Finished"; then
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
