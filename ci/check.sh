#!/bin/bash
# ci/check.sh — 全项目质量检查
# 用法: bash ci/check.sh [--quick|--full]
#   --quick: 仅编译 + 单元测试 (约 30s)
#   --full:  全部检查包括 E2E (约 5min)

set -euo pipefail

MODE="${1:-quick}"
FAILED=0
PASS_COUNT=0
FAIL_COUNT=0

check() {
    local name="$1"
    local cmd="$2"
    local required="${3:-yes}"
    
    echo ""
    echo "=== $name ==="
    if eval "$cmd" 2>&1; then
        echo "✅ PASS: $name"
        PASS_COUNT=$((PASS_COUNT + 1))
    else
        if [ "$required" = "yes" ]; then
            echo "❌ FAIL: $name"
            FAILED=1
            FAIL_COUNT=$((FAIL_COUNT + 1))
        else
            echo "⚠️ WARN: $name (non-blocking)"
        fi
    fi
}

echo "========================================="
echo " ass2sup CI Check (mode: $MODE)"
echo "========================================="
echo "Time: $(date '+%Y-%m-%d %H:%M:%S')"
echo "Dir:  $(pwd)"
echo ""

# --- 阶段 1: 编译 ---
check "cargo check" \
    "cargo check --workspace 2>&1 | tail -5"

# --- 阶段 2: Clippy ---
check "cargo clippy" \
    "cargo clippy --workspace -- -D warnings 2>&1 | tail -10"

# --- 阶段 3: 单元测试 ---
check "unit tests (lib)" \
    "cargo test --workspace --lib 2>&1 | tail -10"

# --- 阶段 4: 集成测试 ---
check "integration tests" \
    "cargo test --workspace --tests 2>&1 | tail -10"

# --- 模式特定检查 ---
if [ "$MODE" = "full" ]; then
    # --- 阶段 5: E2E 测试 ---
    check "E2E tests" \
        "cargo test --workspace -- --ignored 2>&1 | tail -10"
    
    # --- 阶段 6: 文档检查 ---
    check "rustdoc missing docs" \
        "test -z \"\$(RUSTDOCFLAGS='-D warnings' cargo doc --workspace 2>&1 | grep 'missing documentation')\"" \
        "no"
    
    # --- 阶段 7: unwrap 检查 ---
    check "no unwrap() in production" \
        "test 0 -eq \$(grep -rn '\.unwrap()' crates/*/src/ | grep -v 'test' | grep -v '// ' | wc -l)"
    
    # --- 阶段 8: dead_code 滥用检查 ---
    check "no allow(dead_code) abuse" \
        "test 0 -eq \$(grep -rn 'allow(dead_code)' crates/*/src/ | wc -l)" \
        "no"
fi

# --- 总结 ---
echo ""
echo "========================================="
echo " Results: $PASS_COUNT passed, $FAIL_COUNT failed"
echo "========================================="

if [ "$FAILED" -eq 1 ]; then
    echo "❌ CI CHECK FAILED"
    exit 1
else
    echo "✅ ALL CHECKS PASSED"
    exit 0
fi
