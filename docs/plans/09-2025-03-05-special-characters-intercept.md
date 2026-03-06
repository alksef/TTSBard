# Special Characters Processing in Intercept Mode Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable processing of all special characters (Shift+0-9, Shift+punctuation, `\`, `.`, etc.) in text interception mode.

**Architecture:** The current implementation filters out special characters through `is_ascii_graphic()` check in `vk_code_to_char()`. We need to expand the filter to allow printable ASCII punctuation marks while still blocking control characters.

**Tech Stack:** Rust, Windows API (ToUnicodeEx), WH_KEYBOARD_LL hook

---

## Problem Analysis

**Current Issue:** In `hook.rs:191`, the filter:
```rust
if ch.is_ascii_graphic() || (!ch.is_ascii() && !ch.is_control()) {
    return Some(ch);
}
```

`is_ascii_graphic()` returns `true` ONLY for:
- Letters (A-Z, a-z)
- Digits (0-9)
- Underscore (_)

It returns `false` for:
- Punctuation: `!`, `"`, `#`, `$`, `%`, `&`, `'`, `(`, `)`, `*`, `+`, `,`, `-`, `.`, `/`, `:`, `;`, `<`, `=`, `>`, `?`, `@`, `[`, `\`, `]`, `^`, `` ` ``, `{`, `|`, `}`, `~`
- Space (handled separately)
- Control characters

**ToUnicodeEx with GetKeyboardState** correctly provides shift state and returns proper shifted characters. The issue is purely in the filter logic.

---

## Task 1: Add Character Filter Test

**Files:**
- Create: `src-tauri/src/hook_test.rs` (or tests in `src-tauri/tests/hook_tests.rs`)

**Step 1: Write the failing test**

Create test file `src-tauri/tests/hook_filter_test.rs`:

```rust
#[cfg(test)]
mod tests {
    // Test helper function that mimics the filter logic
    fn is_allowed_character(ch: char) -> bool {
        ch.is_ascii_graphic() || (!ch.is_ascii() && !ch.is_control())
    }

    #[test]
    fn test_shift_numbers_are_allowed() {
        // Shift+0-9 symbols
        let shift_numbers = ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')'];
        for &ch in &shift_numbers {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_punctuation_is_allowed() {
        // Common punctuation
        let punctuation = ['.', ',', ';', ':', '?', '!', '\'', '"'];
        for &ch in &punctuation {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_special_symbols_are_allowed() {
        // Special symbols including backslash, pipe, etc.
        let special = ['\\', '|', '/', '@', '#', '$', '%', '^', '&', '*', '_', '+', '-', '=', '~', '`'];
        for &ch in &special {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_brackets_are_allowed() {
        let brackets = ['(', ')', '[', ']', '{', '}', '<', '>'];
        for &ch in &brackets {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_unicode_is_allowed() {
        // Cyrillic and other non-ASCII should be allowed
        let unicode = ['А', 'Б', 'В', 'а', 'б', 'в', '€', '©'];
        for &ch in &unicode {
            assert!(is_allowed_character(ch), "Character '{}' should be allowed", ch);
        }
    }

    #[test]
    fn test_control_characters_are_blocked() {
        // Control characters should be blocked
        let control = ['\x00', '\x01', '\x02', '\x07', '\x08', '\x1B', '\x7F'];
        for &ch in &control {
            assert!(!is_allowed_character(ch), "Control character '{:x}' should be blocked", ch as u32);
        }
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo test --package app-tts-v2 --test hook_filter_test`

Expected: Tests FAIL for special characters, punctuation, brackets

**Step 3: Commit test file**

```bash
git add src-tauri/tests/hook_filter_test.rs
git commit -m "test: add character filter tests for special characters"
```

---

## Task 2: Implement Improved Character Filter

**Files:**
- Modify: `src-tauri/src/hook.rs:191`

**Step 1: Write the implementation**

Replace the filter logic in `vk_code_to_char()` function:

```rust
// Replace lines 190-193 in hook.rs
// OLD:
// if ch.is_ascii_graphic() || (!ch.is_ascii() && !ch.is_control()) {
//     return Some(ch);
// }

// NEW:
// Allow character if it's:
// 1. Printable ASCII (including punctuation: 0x20-0x7E, excluding control chars 0x00-0x1F and 0x7F)
// 2. Non-ASCII and not a control character (Unicode letters, Cyrillic, etc.)
if (ch.is_ascii() && ch >= ' ' && ch <= '~') || (!ch.is_ascii() && !ch.is_control()) {
    return Some(ch);
}
```

**Explanation:**
- `' '` (0x20) through `'~'` (0x7E) covers all printable ASCII:
  - Space (0x20)
  - Punctuation: `!`, `"`, `#`, `$`, `%`, `&`, `'`, `(`, `)`, `*`, `+`, `,`, `-`, `.`, `/`
  - Digits: `0-9`
  - Symbols: `:`, `;`, `<`, `=`, `>`, `?`, `@`
  - Letters: `A-Z`, `[`, `\`, `]`, `^`, `_`, `` ` ``, `a-z`, `{`, `|`, `}`, `~`
- Excludes control characters (0x00-0x1F, 0x7F)
- Second part allows Unicode (Cyrillic, emojis, etc.)

**Step 2: Run tests**

Run: `cargo test --package app-tts-v2`

Expected: All tests PASS

**Step 3: Manual testing**

Test in the app:
1. Enable interception mode (⚡ button)
2. Type Shift+0-9: should see `!`, `@`, `#`, `$`, `%`, `^`, `&`, `*`, `(`, `)`
3. Type `\`: should see `\`
4. Type `.`: should see `.`
5. Type Shift+`,`: should see `<`
6. Type Shift+.`: should see `>`
7. Type Shift+/: should see `?`
8. Type Shift+\: should see `|`
9. Type Russian text: should work as before

**Step 4: Commit**

```bash
git add src-tauri/src/hook.rs
git commit -m "fix(hook): allow all printable ASCII characters in intercept mode

- Changed filter from is_ascii_graphic() to printable ASCII range check
- Now supports Shift+number symbols (!@#$%^&*())
- Supports all punctuation and special characters
- Maintains Unicode support for Cyrillic and other languages"
```

---

## Task 3: Add Integration Test for Shift Characters

**Files:**
- Modify: `src-tauri/tests/hook_filter_test.rs`

**Step 1: Add integration-style test**

Add to the test file:

```rust
#[test]
fn test_printable_ascii_range_complete() {
    // Test that all characters from space (0x20) to tilde (0x7E) are allowed
    for c in 0x20u8..=0x7E {
        let ch = c as char;
        assert!(is_allowed_character(ch), "Character '{}' ({:02x}) should be allowed", ch, c);
    }
}

#[test]
fn test_control_characters_range_blocked() {
    // Test that all control characters (0x00-0x1F, 0x7F) are blocked
    for c in 0x00u8..=0x1F {
        let ch = c as char;
        assert!(!is_allowed_character(ch), "Control character '{:02x}' should be blocked", c);
    }
    // DEL character
    assert!(!is_allowed_character(0x7F as char), "DEL character should be blocked");
}
```

**Step 2: Run tests**

Run: `cargo test --package app-tts-v2 --test hook_filter_test`

Expected: All tests PASS

**Step 3: Commit**

```bash
git add src-tauri/tests/hook_filter_test.rs
git commit -m "test: add comprehensive ASCII range tests for character filter"
```

---

## Documentation

**No documentation changes needed** - this is a bug fix that makes the interception mode work as expected.

---

## Summary of Changes

1. **Test file** (`src-tauri/tests/hook_filter_test.rs`): Comprehensive tests for character filtering
2. **Hook fix** (`src-tauri/src/hook.rs:191`): Changed filter from `is_ascii_graphic()` to printable ASCII range `' '..='~'`

**Before:** Only letters, digits, and underscore were allowed in interception mode
**After:** All printable ASCII characters + Unicode are allowed, control characters are blocked

---

## Testing Checklist

- [ ] Unit tests pass
- [ ] Shift+0-9 produce `!@#$%^&*()`
- [ ] Backslash `\` works
- [ ] Period `.` works
- [ ] Shift+punctuation works: `<`, `>`, `?`, `|`, `"`, `_`, `+`, etc.
- [ ] Russian/Cyrillic text still works
- [ ] Control characters still blocked
- [ ] Space still works (separate code path)
