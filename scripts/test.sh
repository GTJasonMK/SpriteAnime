#!/usr/bin/env bash
# ============================================================
# 一键测试脚本 — Rust单元测试 + TS类型检查 + 前端构建
# 用法: ./scripts/test.sh [--unit|--type|--build|--lint|--all]
# ============================================================
set -euo pipefail

source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/lib/common.sh"

REPORT_FILE="$PROJECT_ROOT/logs/test-report.txt"

PASS=0; FAIL=0; SKIP=0
RESULTS=""

record() {
    local name=$1 status=$2 detail=${3:-}
    case $status in
        PASS) PASS=$((PASS + 1)); RESULTS="${RESULTS}${GREEN}✓${NC} $name\n" ;;
        FAIL) FAIL=$((FAIL + 1)); RESULTS="${RESULTS}${RED}✗${NC} $name — $detail\n" ;;
        SKIP) SKIP=$((SKIP + 1)); RESULTS="${RESULTS}${YELLOW}○${NC} $name (跳过)\n" ;;
    esac
}

MODE="${1:---all}"
if [ "$#" -gt 1 ]; then
    error "用法: ./scripts/test.sh [--unit|--type|--build|--lint|--all]"
    exit 1
fi

banner "SpriteAnimte - 测试套件"

mkdir -p "$PROJECT_ROOT/logs"

# ============================================================
# 测试1: Rust 单元测试
# ============================================================
run_rust_tests() {
    step "Rust 单元测试"
    cd "$PROJECT_ROOT/src-tauri"

    if timeout 60s cargo test --no-fail-fast 2>&1 | tee "$PROJECT_ROOT/logs/cargo-test.log"; then
        record "Rust 单元测试" PASS
        # 统计通过数
        local passed; passed=$(grep -c "ok" "$PROJECT_ROOT/logs/cargo-test.log" 2>/dev/null || echo "?")
        info "测试通过: $passed"
    else
        record "Rust 单元测试" FAIL "详见 logs/cargo-test.log"
    fi
}

# ============================================================
# 测试2: TypeScript 类型检查
# ============================================================
run_type_check() {
    step "TypeScript 类型检查"
    cd "$PROJECT_ROOT"

    if npm run typecheck 2>&1 | tee "$PROJECT_ROOT/logs/tsc-check.log"; then
        record "TS 类型检查" PASS
    else
        local err_count; err_count=$(grep -c "error TS" "$PROJECT_ROOT/logs/tsc-check.log" 2>/dev/null || echo "?")
        record "TS 类型检查" FAIL "${err_count} 个类型错误"
    fi
}

# ============================================================
# 测试3: 前端构建验证
# ============================================================
run_build_check() {
    step "前端构建验证"

    if run_frontend_build 0 | tee "$PROJECT_ROOT/logs/vite-build.log"; then
        record "前端构建" PASS
    else
        record "前端构建" FAIL "详见 logs/vite-build.log"
    fi
}

# ============================================================
# 测试4: Rust 编译检查 (release)
# ============================================================
run_release_check() {
    step "Rust Release 编译检查"

    if run_cargo_check_release 5; then
        record "Rust Release 编译" PASS
    else
        record "Rust Release 编译" FAIL ""
    fi
}

# ============================================================
# 测试5: 代码静态分析 (clippy)
# ============================================================
run_clippy() {
    step "Rust Clippy 静态分析"
    cd "$PROJECT_ROOT/src-tauri"

    if command -v cargo-clippy &>/dev/null; then
        if cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tee "$PROJECT_ROOT/logs/clippy.log"; then
            record "Clippy 静态分析" PASS
        else
            local warn_count; warn_count=$(grep -c "warning:" "$PROJECT_ROOT/logs/clippy.log" 2>/dev/null || echo "?")
            record "Clippy 静态分析" FAIL "${warn_count} 个警告"
        fi
    else
        record "Clippy 静态分析" SKIP "未安装 clippy (rustup component add clippy)"
    fi
}

# ============================================================
# 执行
# ============================================================
case $MODE in
    --unit)
        run_rust_tests
        ;;
    --type)
        run_type_check
        ;;
    --build)
        run_build_check
        run_release_check
        ;;
    --lint)
        run_clippy
        ;;
    --all)
        run_rust_tests
        run_type_check
        run_build_check
        run_release_check
        run_clippy
        ;;
    *)
        error "未知参数: $MODE"
        error "用法: ./scripts/test.sh [--unit|--type|--build|--lint|--all]"
        exit 1
        ;;
esac

# ============================================================
# 汇总报告
# ============================================================
TOTAL=$((PASS + FAIL + SKIP))
echo ""
echo -e "${CYAN}══════════════════════════════════════${NC}"
echo -e "  测试结果:"
echo -e "$RESULTS"
echo -e "${CYAN}──────────────────────────────────────${NC}"
echo -e "  通过: ${GREEN}$PASS${NC}  |  失败: ${RED}$FAIL${NC}  |  跳过: ${YELLOW}$SKIP${NC}  |  总计: $TOTAL"
echo -e "${CYAN}══════════════════════════════════════${NC}"

# 写入报告文件
{
    echo "SpriteAnimte 测试报告"
    echo "时间: $(date '+%Y-%m-%d %H:%M:%S')"
    echo "模式: $MODE"
    echo "结果: 通过=$PASS 失败=$FAIL 跳过=$SKIP"
    echo ""
    echo -e "$RESULTS"
} > "$REPORT_FILE"
info "测试报告: $REPORT_FILE"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
