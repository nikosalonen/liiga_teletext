# Refactoring Quick Start Guide

## 📋 Overview

This directory contains a complete refactoring plan to modularize the liiga_teletext codebase from **7 large files (>1000 lines each)** to **50+ focused modules (<500 lines each)**.

## 📚 Documentation Files

| File | Purpose | Audience |
|------|---------|----------|
| `REFACTORING_PLAN.md` | High-level strategy & architecture | Humans |
| `REFACTORING_TASKS.md` | **Atomic step-by-step instructions** | **AI Models** |
| `REFACTORING_PROGRESS.md` | Progress tracking & metrics | Both |
| `verify_refactoring.sh` | Automated verification script | Both |

## 🤖 For AI Models: Getting Started

### Quick Start (5 Steps)

1. **Read the task instructions:**
   ```bash
   cat REFACTORING_TASKS.md
   ```

2. **Find your task** (start with Task 1.1 if beginning fresh)

3. **Follow the steps EXACTLY** as written - don't skip or modify

4. **After completing the task, run verification:**
   ```bash
   ./verify_refactoring.sh
   ```

5. **If verification passes, update progress:**
   - Mark task as ✅ DONE in `REFACTORING_PROGRESS.md`
   - Note any lessons learned
   - Proceed to next task

### Task Execution Template

```bash
# Before starting
cargo test --all-features  # Note the number of passing tests

# During task - follow REFACTORING_TASKS.md exactly
# Create files, copy code, update imports, etc.

# After each step
cargo check  # Verify it compiles

# After completing task
./verify_refactoring.sh  # Full verification

# If verification passes
git add -A
git commit -m "refactor: [Task X.Y] Description from task card"

# Update REFACTORING_PROGRESS.md
# Mark task as ✅ DONE
```

## 🎯 Current Status

**Phase:** Not Started  
**Next Task:** Task 1.1 - Extract Color Constants  
**Estimated Time:** 10 minutes  
**Risk Level:** LOW

## 📊 Progress Tracking

Check `REFACTORING_PROGRESS.md` for:
- Current completion percentage
- Task statuses (TODO/IN PROGRESS/DONE/BLOCKED)
- Time estimates
- Risk levels
- Lessons learned

## ⚠️ Critical Rules for AI Models

### DO:
✅ Follow task steps EXACTLY in order  
✅ Copy code verbatim (preserve comments, whitespace)  
✅ Run `cargo check` after EACH step  
✅ Run `./verify_refactoring.sh` after EACH task  
✅ Stop immediately if anything fails  
✅ Report errors with full output  

### DON'T:
❌ Add new features during refactoring  
❌ Optimize or "improve" code beyond task scope  
❌ Skip verification steps  
❌ Continue if tests fail  
❌ Make assumptions about file locations  
❌ Modify behavior or logic  

## 🔧 Verification Script

The `verify_refactoring.sh` script runs after each task:

1. ✅ Checks git status
2. ✅ Clean build
3. ✅ Compilation check
4. ✅ Full test suite
5. ✅ Clippy linting
6. ✅ Format checking
7. ✅ Common issues scan

**If it fails:** Stop and fix before proceeding.

## 📈 Task Priority Order

### Week 1 (High Priority)
1. Task 1.1-1.3: UI Colors & Components
2. Task 2.1-2.2: API URLs & Client  
3. Task 3.1: Cache Types

### Week 2 (Medium Priority)
4. Remaining UI splits
5. Remaining API splits
6. Cache module splits

### Week 3 (Low Priority)
7. Config module split
8. Main.rs restructure
9. Test reorganization

## 🆘 Troubleshooting

### "Cannot find module X"
→ Check `pub mod X;` in parent `mod.rs`

### "Function X is private"
→ Ensure `pub fn` and re-export in `mod.rs`

### "Tests failing after refactor"
→ Run `cargo clean && cargo test`
→ Check all `pub use` re-exports

### "Circular dependency"
→ Use `crate::` or `super::` paths

### Need to rollback?
```bash
git reset --hard HEAD~1
# or
git checkout main
git branch -D refactor/task-X.Y
```

## 📝 Task Checklist

Use this for EVERY task:

- [ ] Read entire task card
- [ ] Prerequisites completed
- [ ] Tests passing before changes
- [ ] Created new file(s)
- [ ] Copied code exactly
- [ ] Updated module declarations
- [ ] Added re-exports
- [ ] Updated imports
- [ ] `cargo check` passes after each step
- [ ] `cargo test` passes
- [ ] `./verify_refactoring.sh` passes
- [ ] Committed changes
- [ ] Updated REFACTORING_PROGRESS.md

## 🎓 Example: Task 1.1 (Extract Colors)

### Before Starting
```bash
# Check current state
cargo test --all-features
# Note: X tests passed

# Read the task
grep -A 100 "TASK 1.1" REFACTORING_TASKS.md
```

### During Task
```bash
# Step 1: Create directory
mkdir -p src/ui/teletext
touch src/ui/teletext/colors.rs

# Step 2: Copy code to new file
# (Use editor to copy lines 17-47 from src/teletext_ui.rs)

# Step 3: Delete from original
# (Use editor to remove those lines)

# Step 4: Add import
# (Add: use crate::ui::teletext::colors::*;)

# Step 5-7: Create module files
touch src/ui/teletext/mod.rs
# Add: pub mod colors;

# Step 8: Verify
cargo check  # Should pass

# Step 9: Test
cargo test --all-features  # Should pass with X tests
```

### After Task
```bash
# Run verification
./verify_refactoring.sh

# If passed, commit
git add -A
git commit -m "refactor: [Task 1.1] Extract color constants to separate module"

# Update progress
# Edit REFACTORING_PROGRESS.md:
# Change Task 1.1 status from ⬜️ TODO to ✅ DONE
# Add completion date and any notes
```

## 📞 Getting Help

If stuck on a task:

1. **Re-read the task card** - follow steps exactly
2. **Check the Quick Reference** in REFACTORING_TASKS.md
3. **Run the verification script** - it shows detailed errors
4. **Review git diff** - see what changed
5. **Report the issue** with:
   - Task number
   - Step number where stuck
   - Full error output
   - Output of `cargo check` or `cargo test`

## 🎯 Success Metrics

After completing all tasks:

- ✅ All files under 800 lines
- ✅ No files over 1,000 lines  
- ✅ 100% test pass rate maintained
- ✅ No clippy warnings
- ✅ Clean git history
- ✅ Documentation updated

## 📦 Final Structure Preview

```
src/
├── ui/
│   ├── components/
│   │   └── abbreviations.rs
│   ├── teletext/
│   │   ├── colors.rs
│   │   ├── compact_display.rs
│   │   ├── page.rs
│   │   └── ...
│   └── interactive/
│       ├── state_manager.rs
│       ├── event_handler.rs
│       └── ...
├── data_fetcher/
│   ├── api/
│   │   ├── urls.rs
│   │   ├── client.rs
│   │   └── ...
│   ├── cache/
│   │   ├── types.rs
│   │   ├── tournament_cache.rs
│   │   └── ...
│   └── players/
│       ├── disambiguation.rs
│       └── ...
└── cli/
    ├── args.rs
    ├── commands.rs
    └── ...
```

## 🏁 Ready to Start?

```bash
# Start with Task 1.1
grep -A 150 "TASK 1.1" REFACTORING_TASKS.md

# Good luck! 🚀
```

---

**Document Version:** 1.0  
**Created:** 2025-09-30  
**For:** AI Models executing refactoring tasks