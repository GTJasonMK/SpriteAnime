#!/usr/bin/env bash

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

SCRIPT_LIB_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_LIB_DIR/../.." && pwd)"

info()  { echo -e "${GREEN}[INFO]${NC}  $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC}  $*"; }
error() { echo -e "${RED}[ERROR]${NC} $*"; }
step()  { echo -e "\n${CYAN}═══ $* ═══${NC}"; }

banner() {
    echo -e "${CYAN}"
    echo "╔══════════════════════════════════════╗"
    printf "║   %-34s ║\n" "$1"
    echo "╚══════════════════════════════════════╝"
    echo -e "${NC}"
}

run_frontend_build() {
    local tail_lines=${1:?缺少前端构建输出行数}
    cd "$PROJECT_ROOT"
    if [ "$tail_lines" -gt 0 ]; then
        npm run build 2>&1 | tail -"$tail_lines"
    else
        npm run build 2>&1
    fi
}

run_cargo_build() {
    local profile=${1:?缺少 Rust 构建模式}
    local tail_lines=${2:?缺少 Rust 构建输出行数}
    local cmd
    case "$profile" in
        debug) cmd=(cargo build --manifest-path "$PROJECT_ROOT/src-tauri/Cargo.toml") ;;
        release) cmd=(cargo build --release --manifest-path "$PROJECT_ROOT/src-tauri/Cargo.toml") ;;
        *) error "未知 Rust 构建模式: $profile"; return 1 ;;
    esac

    if [ "$tail_lines" -gt 0 ]; then
        "${cmd[@]}" 2>&1 | tail -"$tail_lines"
    else
        "${cmd[@]}" 2>&1
    fi
}

run_cargo_check_release() {
    local tail_lines=${1:?缺少 Rust 检查输出行数}
    cd "$PROJECT_ROOT/src-tauri"
    if [ "$tail_lines" -gt 0 ]; then
        cargo check --release 2>&1 | tail -"$tail_lines"
    else
        cargo check --release 2>&1
    fi
}
