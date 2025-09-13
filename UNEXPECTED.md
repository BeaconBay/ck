# Unexpected Behaviors in ck

This file tracks instances where ck behaves unexpectedly during testing or usage.

## Format

**Command:** `command run`  
**Expected:** What should have happened  
**Actual:** What actually happened  
**Date:** YYYY-MM-DD  
**Status:** [Fixed/Open/Investigating]

---

## Issues Found

**Command:** `ck ""`
**Expected:** Error message or no results
**Actual:** Massive output with every empty line in the codebase being matched
**Date:** 2025-09-13
**Status:** Open
**Notes:** Empty pattern matches all empty lines, resulting in overwhelming output. Perhaps should warn or limit results?

---

**Command:** `ck --add /tmp/test.txt`
**Expected:** Add file to index or meaningful error
**Actual:** Error: "No file specified. Usage: ck --add <file>"
**Date:** 2025-09-13
**Status:** Open
**Notes:** The error message is incorrect - a file was specified. Seems like argument parsing issue.

---

**Command:** `ck --sem "🎉🦀✨"`
**Expected:** No results or error about emoji patterns
**Actual:** Returns seemingly random code results
**Date:** 2025-09-13
**Status:** Open
**Notes:** Emoji search returns unrelated results. The semantic embedding seems to handle emojis unpredictably.

---

## Instructions

When you encounter unexpected behavior while using ck:

1. Note the exact command you ran
2. Describe what you expected to happen
3. Describe what actually happened
4. Add the date
5. Set status to "Open"

This helps track and fix edge cases and user experience issues.