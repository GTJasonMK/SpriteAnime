#!/usr/bin/env bash
# ============================================================
# 一键运行脚本 — 开发模式自动热重载
# 用法: ./scripts/run.sh [--dev|--debug|--release]
#   默认      Tauri dev 模式（Vite HMR + Rust 自动重编译重启）
#   --dev     Tauri dev 模式（Vite HMR + Rust 自动重编译重启）
#   --debug   构建前端 + 运行 debug 二进制
#   --release 构建前端 + 运行 release 二进制
# ============================================================
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

PID_FILE="$PROJECT_ROOT/.run-pids"
LOCK_FILE="$PROJECT_ROOT/.run-lock"
CLEANUP_DONE=false

# ============================================================
# 资源回收
# ============================================================
cleanup() {
    local exit_code=$?
    if [ "$CLEANUP_DONE" = true ]; then return; fi
    CLEANUP_DONE=true

    echo ""
    warn "正在回收所有资源..."

    # 终止 PID 文件中记录的所有进程（倒序，先杀子进程）
    if [ -f "$PID_FILE" ]; then
        local pids
        pids=$(tac "$PID_FILE" 2>/dev/null || cat "$PID_FILE")
        for pid in $pids; do
            if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
                info "优雅终止 PID=$pid"
                kill -TERM "$pid" 2>/dev/null || true
            fi
        done
        sleep 0.5
        for pid in $pids; do
            if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
                warn "强制终止 PID=$pid"
                kill -KILL "$pid" 2>/dev/null || true
            fi
        done
        rm -f "$PID_FILE"
    fi

    # 杀死本脚本的所有孤儿子进程（进程组）
    local script_pgid
    script_pgid=$(ps -o pgid= -p $$ 2>/dev/null | tr -d ' ' || true)
    if [ -n "$script_pgid" ]; then
        local children
        children=$(ps -o pid= -g "$script_pgid" 2>/dev/null | tr -d ' ' | grep -v "$$" || true)
        for cpid in $children; do
            kill -KILL "$cpid" 2>/dev/null || true
        done
    fi

    rm -f "$LOCK_FILE"
    echo -e "${GREEN}  资源回收完成${NC}"
    exit "$exit_code"
}

trap cleanup EXIT INT TERM HUP QUIT

# ============================================================
# 防重复启动
# ============================================================
check_duplicate() {
    if [ -f "$LOCK_FILE" ]; then
        local lock_pid
        lock_pid=$(cat "$LOCK_FILE" 2>/dev/null || true)
        if [ -n "$lock_pid" ] && kill -0 "$lock_pid" 2>/dev/null; then
            error "已有 SpriteAnimte 实例在运行 (PID=$lock_pid)"
            error "如需强制启动，请先执行: rm $LOCK_FILE"
            exit 1
        fi
        warn "清理过期锁文件"
        rm -f "$LOCK_FILE"
    fi
    echo "$$" > "$LOCK_FILE"
}

# ============================================================
# 检查依赖
# ============================================================
check_deps() {
    if [ ! -d "$PROJECT_ROOT/node_modules" ]; then
        error "node_modules 不存在，请先运行: ./scripts/deploy.sh"
        exit 1
    fi
    if ! command -v cargo &>/dev/null; then
        error "cargo 不可用，请先安装 Rust"
        exit 1
    fi
}

# ============================================================
# 主流程
# ============================================================
MODE="${1:---dev}"

banner "SpriteAnimte - 启动中..."

check_deps
check_duplicate
cd "$PROJECT_ROOT"

case "$MODE" in
    # ---- Tauri dev 模式：前端 HMR + Rust 自动重载 ----
    --dev)
        info "开发模式（自动热重载）"
        info "  → 前端修改：Vite HMR 即时生效"
        info "  → Rust 修改：cargo 自动重编译并重启应用"
        echo ""

        npx tauri dev &
        TAURI_PID=$!
        echo "$TAURI_PID" > "$PID_FILE"
        info "Tauri dev 已启动 PID=$TAURI_PID"
        ;;

    # ---- 发布模式：构建前端 + release 二进制 ----
    --release)
        info "构建前端..."
        run_vite_build 3

        info "编译 Rust (release)..."
        run_cargo_build release 3

        info "启动应用 (release)..."
        "$PROJECT_ROOT/src-tauri/target/release/sprite-anime" &
        TAURI_PID=$!
        echo "$TAURI_PID" > "$PID_FILE"
        info "应用已启动 PID=$TAURI_PID"
        ;;

    # ---- debug 模式：构建前端 + debug 二进制 ----
    --debug)
        info "构建前端..."
        run_vite_build 3

        info "编译 Rust (debug)..."
        run_cargo_build debug 3

        info "启动应用..."
        "$PROJECT_ROOT/src-tauri/target/debug/sprite-anime" &
        TAURI_PID=$!
        echo "$TAURI_PID" > "$PID_FILE"
        info "应用已启动 PID=$TAURI_PID"
        ;;

    *)
        error "未知参数: $MODE"
        error "用法: ./scripts/run.sh [--dev|--debug|--release]"
        exit 1
        ;;
esac

echo -e "${GREEN}  SpriteAnimte 运行中，按 Ctrl+C 退出${NC}"
echo ""

# 等待主进程
wait "$TAURI_PID"
