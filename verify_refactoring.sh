#!/bin/bash
# Refactoring Verification Script
# Run this after EACH task to ensure everything still works

set -e  # Exit on first error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Track failures
FAILURES=0

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}  Liiga Teletext Refactoring Verification${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

# Function to print status
print_status() {
    if [ $1 -eq 0 ]; then
        echo -e "${GREEN}✅ $2${NC}"
    else
        echo -e "${RED}❌ $2${NC}"
        FAILURES=$((FAILURES + 1))
    fi
}

# Function to run check
run_check() {
    local name=$1
    local command=$2
    
    echo -e "\n${YELLOW}Running: $name${NC}"
    
    if eval "$command" > /tmp/verify_output.log 2>&1; then
        print_status 0 "$name passed"
        return 0
    else
        print_status 1 "$name failed"
        echo -e "${RED}Output:${NC}"
        tail -n 20 /tmp/verify_output.log
        return 1
    fi
}

# Step 1: Check git status
echo -e "\n${BLUE}[1/7] Checking git status...${NC}"
if git diff --quiet; then
    echo -e "${GREEN}✅ No uncommitted changes (expected after commit)${NC}"
else
    echo -e "${YELLOW}⚠️  Uncommitted changes detected${NC}"
    echo "Modified files:"
    git status --short
fi

# Step 2: Clean build
echo -e "\n${BLUE}[2/7] Running clean build...${NC}"
run_check "cargo clean" "cargo clean"
run_check "cargo build" "cargo build --all-features"

# Step 3: Check compilation
echo -e "\n${BLUE}[3/7] Checking compilation...${NC}"
run_check "cargo check" "cargo check --all-features"

# Step 4: Run tests
echo -e "\n${BLUE}[4/7] Running tests...${NC}"
run_check "cargo test" "cargo test --all-features -- --test-threads=1"

# Step 5: Run clippy
echo -e "\n${BLUE}[5/7] Running clippy...${NC}"
run_check "cargo clippy" "cargo clippy --all-features --all-targets -- -D warnings"

# Step 6: Run formatter check
echo -e "\n${BLUE}[6/7] Checking formatting...${NC}"
run_check "cargo fmt check" "cargo fmt -- --check"

# Step 7: Check for common issues
echo -e "\n${BLUE}[7/7] Checking for common issues...${NC}"

# Check for TODO comments added during refactoring
TODO_COUNT=$(grep -r "// TODO" src/ 2>/dev/null | wc -l || echo 0)
if [ "$TODO_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⚠️  Found $TODO_COUNT TODO comments${NC}"
else
    echo -e "${GREEN}✅ No TODO comments${NC}"
fi

# Check for debug prints
DEBUG_COUNT=$(grep -r "println!\|dbg!\|eprintln!" src/ 2>/dev/null | grep -v "//\|test" | wc -l || echo 0)
if [ "$DEBUG_COUNT" -gt 0 ]; then
    echo -e "${YELLOW}⚠️  Found $DEBUG_COUNT debug print statements${NC}"
else
    echo -e "${GREEN}✅ No debug prints${NC}"
fi

# Check file sizes
echo -e "\n${BLUE}Checking file sizes...${NC}"
LARGE_FILES=$(find src -name "*.rs" -type f -exec wc -l {} \; | awk '$1 > 1000 {print $0}' | wc -l)
if [ "$LARGE_FILES" -gt 0 ]; then
    echo -e "${YELLOW}⚠️  Files still over 1000 lines:${NC}"
    find src -name "*.rs" -type f -exec wc -l {} \; | awk '$1 > 1000 {print $0}' | sort -rn
else
    echo -e "${GREEN}✅ All files under 1000 lines${NC}"
fi

# Summary
echo -e "\n${BLUE}================================================${NC}"
echo -e "${BLUE}  Summary${NC}"
echo -e "${BLUE}================================================${NC}"

if [ $FAILURES -eq 0 ]; then
    echo -e "${GREEN}✅ All checks passed!${NC}"
    echo -e "${GREEN}✅ Safe to commit and proceed to next task${NC}"
    exit 0
else
    echo -e "${RED}❌ $FAILURES check(s) failed${NC}"
    echo -e "${RED}❌ Fix issues before proceeding${NC}"
    echo ""
    echo -e "${YELLOW}To rollback:${NC}"
    echo "  git reset --hard HEAD~1"
    echo ""
    exit 1
fi