# Preprocessor Testing Guide

## Test Cases

### Live Replacement (Interception Mode)
- [ ] Type `\name ` → instantly replaced to "Alice"
- [ ] Type `%user ` → instantly replaced to "John Doe"
- [ ] Multiple replacements in one sentence
- [ ] Mixed replacements and usernames

### Manual Input Mode
- [ ] Enter `\name %user` in Ввод panel
- [ ] Click "Озвучить"
- [ ] TTS says replaced text

### Edge Cases
- [ ] Unknown `\word` - keeps `\word`
- [ ] Unknown `%username` - keeps `%username`
- [ ] Empty replacement list - text passes through
- [ ] Special characters in values
- [ ] Unicode/Cyrillic in keys and values
- [ ] Very long replacement values
- [ ] Replacement with spaces

### Persistence
- [ ] Save on blur works
- [ ] Data persists across app restarts
- [ ] Both files created in %APPDATA%\ttsbard\

### Reload Behavior
- [ ] Changing replacement reloads preprocessor
- [ ] New replacements work immediately after save

### File Format Validation
- [ ] Lines without space are skipped as invalid
- [ ] Lines starting with # are treated as comments
- [ ] Empty lines are skipped
- [ ] Format: `key value` (space-separated)

### Files Location
Replacements: `%APPDATA%\ttsbard\replacements.txt`
Usernames: `%APPDATA%\ttsbard\usernames.txt`

## Example Test Session

1. Open the app
2. Navigate to "Препроцессор" panel
3. Add replacements:
   ```
   name Alice
   greeting Hello there
   cond condition
   ```
4. Add usernames:
   ```
   john John Doe
   admin Administrator
   ```
5. Click away to save (blur)
6. Enable interception mode (Ctrl+Shift+F1)
7. In floating window, type: `\g` then press space
   **Expected:** Text instantly changes to "Hello there"
8. Continue typing: ` %j` then space
   **Expected:** Text changes to "Hello there John Doe"
9. Press Enter to send to TTS
   **Expected:** TTS says "Hello there John Doe"
