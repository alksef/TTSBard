# Text Preprocessor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement text preprocessing system with **live replacement** in floating window - when user types `\word ` or `@username ` and presses space, the text is instantly replaced. Also works for manual input and TTS synthesis.

**Architecture:**
- **Backend (Rust):** `preprocessor` module with `TextPreprocessor` that handles replacement lists from `%APPDATA%\ttsbard\`
- **Keyboard Hook:** Integration in `hook.rs` - on spacebar press, check if preceding word is `\key` or `@user`, replace instantly
- **Frontend (Vue):** "Препроцессор" panel with two textareas for replacement lists, auto-save on blur
- **Double Safety:** Preprocessing also applied before TTS synthesis (fallback)

**Tech Stack:** Rust (regex, std::fs), Vue 3 (Composition API), Tauri commands

---

## Task 1: Add Dependencies

**Files:**
- Modify: `src-tauri/Cargo.toml`

**Step 1: Add regex and lazy_static dependencies**

Modify: `src-tauri/Cargo.toml`

Add to `[dependencies]` section:

```toml
regex = "1"
lazy_static = "1.4"
```

**Step 2: Run cargo check**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: Dependencies download and compile successfully

**Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "build: add regex and lazy_static dependencies

Required for text preprocessing patterns

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 2: Create Rust Preprocessor Module

**Files:**
- Create: `src-tauri/src/preprocessor/mod.rs`
- Create: `src-tauri/src/preprocessor/replacer.rs`
- Modify: `src-tauri/src/lib.rs` (module registration)

**Step 1: Create preprocessor directory and module structure**

Create: `src-tauri/src/preprocessor/mod.rs`

```rust
mod replacer;

pub use replacer::{TextPreprocessor, ReplacementList};

use anyhow::Result;
use std::path::PathBuf;

/// Get the appdata directory for preprocessor files
pub fn get_preprocessor_dir() -> Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Failed to get config dir"))?
        .join("ttsbard");

    // Create directory if it doesn't exist
    std::fs::create_dir_all(&config_dir)?;

    Ok(config_dir)
}

/// Path to the replacements list file
pub fn replacements_file() -> Result<PathBuf> {
    Ok(get_preprocessor_dir()?.join("replacements.txt"))
}

/// Path to the usernames list file
pub fn usernames_file() -> Result<PathBuf> {
    Ok(get_preprocessor_dir()?.join("usernames.txt"))
}
```

**Step 2: Implement the replacer logic**

Create: `src-tauri/src/preprocessor/replacer.rs`

```rust
use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

lazy_static::lazy_static! {
    /// Regex to match \word pattern at end of text
    static ref REPLACEMENT_PATTERN_END: Regex = Regex::new(r"\\(\w+)\s*$").unwrap();
    /// Regex to match @username pattern at end of text
    static ref USERNAME_PATTERN_END: Regex = Regex::new(r"@(\w+)\s*$").unwrap();
}

#[derive(Debug, Clone)]
pub struct ReplacementList {
    replacements: HashMap<String, String>,
    usernames: HashMap<String, String>,
}

impl Default for ReplacementList {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplacementList {
    pub fn new() -> Self {
        Self {
            replacements: HashMap::new(),
            usernames: HashMap::new(),
        }
    }

    /// Add a replacement mapping
    pub fn add_replacement(&mut self, key: String, value: String) {
        self.replacements.insert(key, value);
    }

    /// Add a username mapping
    pub fn add_username(&mut self, key: String, value: String) {
        self.usernames.insert(key, value);
    }

    /// Load replacements from a file (one "key value" per line, space-separated)
    /// Lines without space are skipped as invalid
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut list = Self::new();

        if !path.as_ref().exists() {
            return Ok(list);
        }

        let content = fs::read_to_string(path)
            .context("Failed to read replacements file")?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue; // Skip empty lines and comments
            }

            // Split on first space only
            if let Some((key, value)) = line.split_once(' ') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if !key.is_empty() && !value.is_empty() {
                    list.add_replacement(key, value);
                }
            }
            // If no space found, skip this line as invalid
        }

        Ok(list)
    }

    /// Load usernames from a file (one "key value" per line, space-separated)
    pub fn load_usernames_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut list = Self::new();

        if !path.as_ref().exists() {
            return Ok(list);
        }

        let content = fs::read_to_string(path)
            .context("Failed to read usernames file")?;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Split on first space only
            if let Some((key, value)) = line.split_once(' ') {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if !key.is_empty() && !value.is_empty() {
                    list.add_username(key, value);
                }
            }
            // If no space found, skip this line as invalid
        }

        Ok(list)
    }

    /// Save replacements to a file (space-separated format)
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut content = String::new();

        for (key, value) in &self.replacements {
            content.push_str(&format!("{} {}\n", key, value));
        }

        fs::write(path, content)
            .context("Failed to write replacements file")?;

        Ok(())
    }

    /// Save usernames to a file (space-separated format)
    pub fn save_usernames_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut content = String::new();

        for (key, value) in &self.usernames {
            content.push_str(&format!("{} {}\n", key, value));
        }

        fs::write(path, content)
            .context("Failed to write usernames file")?;

        Ok(())
    }

    /// Get replacement for a key
    pub fn get_replacement(&self, key: &str) -> Option<&String> {
        self.replacements.get(key)
    }

    /// Get username for a key
    pub fn get_username(&self, key: &str) -> Option<&String> {
        self.usernames.get(key)
    }
}

/// Text preprocessor that applies replacements
pub struct TextPreprocessor {
    replacements: ReplacementList,
}

impl TextPreprocessor {
    pub fn new(replacements: ReplacementList) -> Self {
        Self { replacements }
    }

    /// Create a preprocessor by loading from files
    pub fn load_from_files() -> Result<Self> {
        let replacements_path = super::replacements_file()?;
        let usernames_path = super::usernames_file()?;

        let mut replacement_list = ReplacementList::load_from_file(&replacements_path)
            .unwrap_or_default();

        // Load usernames from separate file
        if usernames_path.exists() {
            let username_list = ReplacementList::load_usernames_from_file(&usernames_path)?;
            for (key, value) in username_list.get_usernames_map() {
                replacement_list.add_username(key.clone(), value.clone());
            }
        }

        Ok(Self::new(replacement_list))
    }

    /// Check if text ends with a replaceable pattern and return the replacement
    /// Returns Some((original_pattern, replacement_value)) or None
    pub fn check_and_replace_end(&self, text: &str) -> Option<(String, String)> {
        // Check for \word pattern at end
        if let Some(caps) = REPLACEMENT_PATTERN_END.captures(text) {
            let key = &caps[1];
            if let Some(replacement) = self.replacements.get_replacement(key) {
                let pattern = format!(r"\{}", key);
                let result = text.replacen(&pattern, replacement, 1);
                return Some((pattern, result));
            }
        }

        // Check for @username pattern at end
        if let Some(caps) = USERNAME_PATTERN_END.captures(text) {
            let key = &caps[1];
            if let Some(username) = self.replacements.get_username(key) {
                let pattern = format!("@{}", key);
                let result = text.replacen(&pattern, username, 1);
                return Some((pattern, result));
            }
        }

        None
    }

    /// Preprocess all text, replacing all \word and @username patterns
    pub fn process(&self, text: &str) -> String {
        let result = text.to_string();

        // Replace all \word patterns
        let result = result.replace(|c: char| {
            // Find all \word patterns and replace
            unimplemented!()
        });

        // Simple approach: use regex for all patterns
        let re = Regex::new(r"\\(\w+)").unwrap();
        let result = re.replace_all(&result, |caps: &regex::Captures| {
            let key = &caps[1];
            if let Some(replacement) = self.replacements.get_replacement(key) {
                replacement.clone()
            } else {
                format!("\\{}", key)
            }
        }).to_string();

        // Replace all @username patterns
        let re = Regex::new(r"@(\w+)").unwrap();
        let result = re.replace_all(&result, |caps: &regex::Captures| {
            let key = &caps[1];
            if let Some(username) = self.replacements.get_username(key) {
                username.clone()
            } else {
                format!("@{}", key)
            }
        }).to_string();

        result
    }

    /// Reload replacements from files
    pub fn reload(&mut self) -> Result<()> {
        self.replacements = ReplacementList::load_from_file(super::replacements_file()?)
            .unwrap_or_default();

        let usernames_path = super::usernames_file()?;
        if usernames_path.exists() {
            let username_list = ReplacementList::load_usernames_from_file(&usernames_path)?;
            for (key, value) in username_list.get_usernames_map() {
                self.replacements.add_username(key.clone(), value.clone());
            }
        }

        Ok(())
    }
}

// Helper for accessing private data in tests
impl ReplacementList {
    pub fn get_usernames_map(&self) -> &HashMap<String, String> {
        &self.usernames
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replacement_end_pattern() {
        let text = r#"Hello \name "#;
        let matches: Vec<_> = REPLACEMENT_PATTERN_END.find_iter(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), r#"\name "#);
    }

    #[test]
    fn test_username_end_pattern() {
        let text = "Hey @user ";
        let matches: Vec<_> = USERNAME_PATTERN_END.find_iter(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "@user ");
    }

    #[test]
    fn test_check_and_replace_end() {
        let mut replacements = ReplacementList::new();
        replacements.add_replacement("name".to_string(), "Alice".to_string());

        let preprocessor = TextPreprocessor::new(replacements);

        let input = "Hello \\name ";
        let result = preprocessor.check_and_replace_end(input);
        assert_eq!(result, Some((r"\name".to_string(), "Hello Alice ".to_string())));
    }

    #[test]
    fn test_check_and_replace_end_no_match() {
        let replacements = ReplacementList::new();
        let preprocessor = TextPreprocessor::new(replacements);

        let input = "Hello world";
        let result = preprocessor.check_and_replace_end(input);
        assert_eq!(result, None);
    }

    #[test]
    fn test_process_all_replacements() {
        let mut replacements = ReplacementList::new();
        replacements.add_replacement("name".to_string(), "Alice".to_string());
        replacements.add_replacement("greeting".to_string(), "Hello there".to_string());

        let preprocessor = TextPreprocessor::new(replacements);

        let input = r#"\greeting \name!"#;
        let output = preprocessor.process(input);
        assert_eq!(output, "Hello there Alice!");
    }

    #[test]
    fn test_replacement_list_parse() {
        let content = r#"
# This is a comment
name Alice
greeting Hello there

# Another comment
admin Administrator
# Invalid line without space - should be skipped
invalidline
"#;

        let temp_file = "test_replacements.txt";
        fs::write(temp_file, content).unwrap();

        let list = ReplacementList::load_from_file(temp_file).unwrap();
        assert_eq!(list.get_replacement("name"), Some(&"Alice".to_string()));
        assert_eq!(list.get_replacement("greeting"), Some(&"Hello there".to_string()));
        assert_eq!(list.get_replacement("admin"), Some(&"Administrator".to_string()));
        assert_eq!(list.get_replacement("invalidline"), None); // Skipped

        fs::remove_file(temp_file).unwrap();
    }

    #[test]
    fn test_replacement_list_save_format() {
        let mut replacements = ReplacementList::new();
        replacements.add_replacement("key".to_string(), "value with spaces".to_string());

        let temp_file = "test_save_format.txt";
        replacements.save_to_file(temp_file).unwrap();

        let content = fs::read_to_string(temp_file).unwrap();
        assert_eq!(content, "key value with spaces\n");

        // Verify it can be loaded back
        let loaded = ReplacementList::load_from_file(temp_file).unwrap();
        assert_eq!(loaded.get_replacement("key"), Some(&"value with spaces".to_string()));

        fs::remove_file(temp_file).unwrap();
    }
}
```

**Step 3: Register the module in lib.rs**

Modify: `src-tauri/src/lib.rs`

Add after line ~20 (with other module declarations):

```rust
mod preprocessor;
```

**Step 4: Run tests to verify implementation**

Run: `cargo test --manifest-path src-tauri/Cargo.toml preprocessor -- --nocapture`

Expected: All tests pass

**Step 5: Commit**

```bash
git add src-tauri/src/preprocessor/
git add src-tauri/src/lib.rs
git commit -m "feat(preprocessor): add text preprocessor module

- Add TextPreprocessor for \\word and @username replacement
- Support loading replacement lists from text files
- Add check_and_replace_end() for live replacement on space
- Add comprehensive unit tests
- Files stored in %APPDATA%\\ttsbard\\

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 3: Integrate Live Replacement in Keyboard Hook

**Files:**
- Modify: `src-tauri/src/hook.rs`
- Modify: `src-tauri/src/state.rs` (add preprocessor cache)

**Step 1: Add preprocessor to AppState**

Modify: `src-tauri/src/state.rs`

Add field to `AppState`:

```rust
use crate::preprocessor::TextPreprocessor;
use std::sync::Arc;

pub struct AppState {
    // ... existing fields ...

    /// Cached preprocessor for live replacement
    preprocessor: Arc<Mutex<Option<TextPreprocessor>>>,
}
```

Add methods:

```rust
impl AppState {
    // ... existing methods ...

    /// Get or create preprocessor instance
    pub fn get_preprocessor(&self) -> Option<TextPreprocessor> {
        let mut prep = self.preprocessor.lock().ok()?;
        if prep.is_none() {
            *prep = TextPreprocessor::load_from_files().ok();
        }
        prep.clone()
    }

    /// Reload preprocessor (call when settings change)
    pub fn reload_preprocessor(&self) {
        if let Ok(mut prep) = self.preprocessor.lock() {
            *prep = TextPreprocessor::load_from_files().ok();
        }
    }
}
```

**Step 2: Add spacebar handling in hook**

Modify: `src-tauri/src/hook.rs`

Find the keyboard callback and add spacebar handling. After the character processing, add:

```rust
// Check for spacebar - trigger live replacement
if vk_code == 0x20 {  // VK_SPACE
    // Get current text
    let current = {
        let text = state.current_text.lock()
            .map_err(|e| format!("Failed to lock current_text: {}", e))?;
        text.clone()
    };

    // Check if we need to replace
    if let Some(preprocessor) = state.get_preprocessor() {
        if let Some((_pattern, replaced)) = preprocessor.check_and_replace_end(&current) {
            // Update the text with replacement
            if let Ok(mut text) = state.current_text.lock() {
                *text = replaced.clone();

                // Emit update to floating window
                emit_update_to_floating_window(&replaced);
            }
        }
    }
}
```

**Step 3: Emit update helper**

Add helper function in `hook.rs`:

```rust
fn emit_update_to_floating_window(text: &str) {
    // Use the existing event mechanism to update UI
    // This will trigger the 'update-text' event in the floating window
    if let Ok(tx) = EVENT_CHANNEL.try_send() {
        let _ = tx.send(AppEvent::UpdateFloatingText(text.to_string()));
    }
}
```

**Step 4: Test compilation**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: Compiles successfully

**Step 5: Commit**

```bash
git add src-tauri/src/hook.rs
git add src-tauri/src/state.rs
git commit -m "feat(hook): add live replacement on spacebar

- Integrate preprocessor into keyboard hook
- When space is pressed after \\word or @user, replace instantly
- Update floating window UI with replaced text
- Cache preprocessor in AppState for performance

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 4: Add Tauri Commands for Preprocessor

**Files:**
- Create: `src-tauri/src/commands/preprocessor.rs`
- Modify: `src-tauri/src/commands.rs` (include new commands)

**Step 1: Create preprocessor commands module**

Create: `src-tauri/src/commands/preprocessor.rs`

```rust
use crate::preprocessor::{TextPreprocessor, ReplacementList, replacements_file, usernames_file};
use tauri::State;
use std::fs;

/// Get the current replacements list content
#[tauri::command]
pub fn get_replacements() -> Result<String, String> {
    let path = replacements_file()
        .map_err(|e| format!("Failed to get replacements file path: {}", e))?;

    if path.exists() {
        fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read replacements file: {}", e))
    } else {
        Ok(String::new())
    }
}

/// Save the replacements list content
#[tauri::command]
pub fn save_replacements(content: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = replacements_file()
        .map_err(|e| format!("Failed to get replacements file path: {}", e))?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(&path, content)
        .map_err(|e| format!("Failed to write replacements file: {}", e))?;

    // Reload preprocessor in state
    state.reload_preprocessor();

    Ok(())
}

/// Get the current usernames list content
#[tauri::command]
pub fn get_usernames() -> Result<String, String> {
    let path = usernames_file()
        .map_err(|e| format!("Failed to get usernames file path: {}", e))?;

    if path.exists() {
        fs::read_to_string(&path)
            .map_err(|e| format!("Failed to read usernames file: {}", e))
    } else {
        Ok(String::new())
    }
}

/// Save the usernames list content
#[tauri::command]
pub fn save_usernames(content: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = usernames_file()
        .map_err(|e| format!("Failed to get usernames file path: {}", e))?;

    // Create parent directory if it doesn't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory: {}", e))?;
    }

    fs::write(&path, content)
        .map_err(|e| format!("Failed to write usernames file: {}", e))?;

    // Reload preprocessor in state
    state.reload_preprocessor();

    Ok(())
}

/// Preview preprocessed text (for UI testing)
#[tauri::command]
pub fn preview_preprocessing(text: String) -> Result<String, String> {
    let preprocessor = TextPreprocessor::load_from_files()
        .map_err(|e| format!("Failed to load preprocessor: {}", e))?;

    Ok(preprocessor.process(&text))
}
```

**Step 2: Register commands in main commands module**

Modify: `src-tauri/src/commands.rs`

Add at the end:

```rust
pub mod preprocessor;

// Re-export preprocessor commands
pub use preprocessor::{
    get_replacements,
    save_replacements,
    get_usernames,
    save_usernames,
    preview_preprocessing,
};
```

**Step 3: Register commands in tauri::Builder**

Modify: `src-tauri/src/lib.rs`

Add to `invoke_handler`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    // Preprocessor commands
    commands::get_replacements,
    commands::save_replacements,
    commands::get_usernames,
    commands::save_usernames,
    commands::preview_preprocessing,
])
```

**Step 4: Test command registration**

Run: `cargo check --manifest-path src-tauri/Cargo.toml`

Expected: No errors

**Step 5: Commit**

```bash
git add src-tauri/src/commands.rs
git add src-tauri/src/commands/preprocessor.rs
git add src-tauri/src/lib.rs
git commit -m "feat(commands): add preprocessor Tauri commands

- Add get_replacements/save_replacements for replacement list
- Add get_usernames/save_usernames for username list
- Add preview_preprocessing for testing preprocessing
- Auto-reload preprocessor when settings change

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 5: Integrate Preprocessing into TTS Flow (Fallback)

**Files:**
- Modify: `src-tauri/src/commands.rs` (speak_text function)
- Modify: `src-tauri/src/lib.rs` (handle_event TextReady)

**Step 1: Apply preprocessing in speak_text command**

Modify: `src-tauri/src/commands.rs`

Find the `speak_text` function and add preprocessing:

```rust
#[tauri::command]
pub async fn speak_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    // ... existing empty text check ...

    // Preprocess text before TTS (fallback for any missed replacements)
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        preprocessor.process(&text)
    } else {
        text
    };

    // ... rest of the function continues with preprocessed text ...
```

**Step 2: Apply preprocessing in TextReady event handler**

Modify: `src-tauri/src/lib.rs`

Find `AppEvent::TextReady(text)` and add preprocessing:

```rust
AppEvent::TextReady(text) => {
    eprintln!("[EVENT] Text received: '{}'", text);

    // Preprocess text before TTS (fallback)
    let text = if let Some(preprocessor) = state.get_preprocessor() {
        preprocessor.process(&text)
    } else {
        text
    };

    // ... rest of the TTS synthesis continues ...
```

**Step 3: Test preprocessing integration**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`

Expected: Builds successfully

**Step 4: Commit**

```bash
git add src-tauri/src/commands.rs
git add src-tauri/src/lib.rs
git commit -m "feat(integration): apply preprocessing to TTS flow

- Preprocess text in speak_text command
- Preprocess text in TextReady event handler
- Use as fallback for any missed live replacements

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 6: Create PreprocessorPanel Vue Component

**Files:**
- Create: `src/components/PreprocessorPanel.vue`
- Modify: `src/components/Sidebar.vue` (add menu item)
- Modify: `src/App.vue` (register panel)

**Step 1: Create the PreprocessorPanel component**

Create: `src/components/PreprocessorPanel.vue`

```vue
<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/tauri'

// Reactive state
const replacements = ref('')
const usernames = ref('')
const isLoading = ref(false)
const testInput = ref('Hello \\name, message from @user')
const testOutput = ref('')

// Load replacements from backend
async function loadReplacements() {
  try {
    const content = await invoke<string>('get_replacements')
    replacements.value = content
  } catch (error) {
    console.error('Failed to load replacements:', error)
  }
}

// Load usernames from backend
async function loadUsernames() {
  try {
    const content = await invoke<string>('get_usernames')
    usernames.value = content
  } catch (error) {
    console.error('Failed to load usernames:', error)
  }
}

// Save replacements to backend
async function saveReplacements() {
  try {
    await invoke('save_replacements', { content: replacements.value })
    console.log('Replacements saved')
  } catch (error) {
    console.error('Failed to save replacements:', error)
  }
}

// Save usernames to backend
async function saveUsernames() {
  try {
    await invoke('save_usernames', { content: usernames.value })
    console.log('Usernames saved')
  } catch (error) {
    console.error('Failed to save usernames:', error)
  }
}

// Test preprocessing
async function testPreprocessing() {
  try {
    const result = await invoke<string>('preview_preprocessing', { text: testInput.value })
    testOutput.value = result
  } catch (error) {
    console.error('Failed to test preprocessing:', error)
    testOutput.value = 'Error: ' + error
  }
}

// Handle blur (save on focus loss)
function onReplacementsBlur() {
  saveReplacements()
}

function onUsernamesBlur() {
  saveUsernames()
}

// Load data on mount
onMounted(async () => {
  isLoading.value = true
  await Promise.all([
    loadReplacements(),
    loadUsernames()
  ])
  isLoading.value = false
})
</script>

<template>
  <div class="preprocessor-panel">
    <h2>Препроцессор текста</h2>

    <div v-if="isLoading" class="loading">
      Загрузка...
    </div>

    <div v-else class="panel-content">
      <!-- Info Banner -->
      <div class="info-banner">
        <p>💡 В режиме перехвата текст заменяется <strong>мгновенно</strong> при нажатии пробела после <code>\ключ</code> или <code>@юзернейм</code></p>
      </div>

      <!-- Replacements Section -->
      <section class="section">
        <h3>Список замен</h3>
        <p class="hint">
          Используйте <code>\ключ</code> для замены. Формат: <code>ключ значение</code> (через пробел)
        </p>
        <textarea
          v-model="replacements"
          @blur="onReplacementsBlur"
          placeholder="name Алекс&#10;greeting Привет всем&#10;admin Администратор"
          class="input-area"
          rows="10"
        ></textarea>
        <p class="status">
          Сохраняется при потере фокуса
        </p>
      </section>

      <!-- Usernames Section -->
      <section class="section">
        <h3>Список юзернеймов</h3>
        <p class="hint">
          Используйте <code>@юзернейм</code> для замены. Формат: <code>ключ значение</code> (через пробел)
        </p>
        <textarea
          v-model="usernames"
          @blur="onUsernamesBlur"
          placeholder="john Джон Смит&#10;admin Администратор&#10;dev Разработчик"
          class="input-area"
          rows="10"
        ></textarea>
        <p class="status">
          Сохраняется при потере фокуса
        </p>
      </section>

      <!-- Test Section -->
      <section class="section test-section">
        <h3>Проверка</h3>
        <div class="test-inputs">
          <div class="input-group">
            <label>Входной текст:</label>
            <input
              v-model="testInput"
              type="text"
              class="test-input"
              placeholder="Hello \name, message from @user"
            />
          </div>
          <button @click="testPreprocessing" class="test-button">
            Проверить
          </button>
          <div class="output-group">
            <label>Результат:</label>
            <div class="test-output">{{ testOutput || 'Нажмите "Проверить"' }}</div>
          </div>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.preprocessor-panel {
  padding: 20px;
  max-width: 800px;
  margin: 0 auto;
}

h2 {
  margin-bottom: 20px;
  color: #ffffff;
}

.info-banner {
  background: rgba(74, 222, 128, 0.1);
  border: 1px solid rgba(74, 222, 128, 0.3);
  border-radius: 8px;
  padding: 12px 16px;
  margin-bottom: 20px;
}

.info-banner p {
  margin: 0;
  color: #4ade80;
  font-size: 13px;
}

.info-banner code {
  background: rgba(74, 222, 128, 0.2);
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', monospace;
}

.section {
  margin-bottom: 30px;
  background: #3a3a3a;
  padding: 20px;
  border-radius: 8px;
}

h3 {
  margin-bottom: 10px;
  color: #ffffff;
  font-size: 16px;
}

.hint {
  font-size: 12px;
  color: #888;
  margin-bottom: 10px;
}

.hint code {
  background: #4a4a4a;
  padding: 2px 6px;
  border-radius: 3px;
  font-family: 'Consolas', monospace;
  color: #4ec9b0;
}

.input-area {
  width: 100%;
  background: #2c2c2c;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #ffffff;
  padding: 10px;
  font-family: 'Consolas', monospace;
  font-size: 13px;
  resize: vertical;
}

.input-area:focus {
  outline: none;
  border-color: #007acc;
}

.status {
  font-size: 11px;
  color: #666;
  margin-top: 5px;
}

.test-section {
  background: #3a3a3a;
}

.test-inputs {
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.input-group, .output-group {
  display: flex;
  flex-direction: column;
  gap: 5px;
}

label {
  font-size: 12px;
  color: #aaa;
}

.test-input {
  background: #2c2c2c;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #ffffff;
  padding: 8px 10px;
  font-family: 'Consolas', monospace;
  font-size: 13px;
}

.test-input:focus {
  outline: none;
  border-color: #007acc;
}

.test-button {
  align-self: flex-start;
  background: #007acc;
  border: none;
  border-radius: 4px;
  color: #ffffff;
  padding: 8px 16px;
  cursor: pointer;
  font-size: 13px;
}

.test-button:hover {
  background: #0069b4;
}

.test-button:active {
  background: #005a9e;
}

.test-output {
  background: #2c2c2c;
  border: 1px solid #4a4a4a;
  border-radius: 4px;
  color: #4ec9b0;
  padding: 10px;
  font-family: 'Consolas', monospace;
  font-size: 13px;
  min-height: 40px;
}

.loading {
  text-align: center;
  padding: 40px;
  color: #888;
}
</style>
```

**Step 2: Add menu item to Sidebar**

Modify: `src/components/Sidebar.vue`

Add after AudioPanel item:

```vue
<div
  class="menu-item"
  :class="{ active: currentPanel === 'preprocessor' }"
  @click="$emit('setPanel', 'preprocessor')"
>
  <span class="icon">🔧</span>
  <span>Препроцессор</span>
</div>
```

**Step 3: Register the panel in App.vue**

Modify: `src/App.vue`

Add import and panel:

```vue
<script setup lang="ts">
import PreprocessorPanel from './components/PreprocessorPanel.vue'
// ... other imports
</script>

<template>
  <!-- ... existing panels ... -->
  <PreprocessorPanel v-if="currentPanel === 'preprocessor'" />
</template>
```

**Step 4: Test the UI**

Run: `npm run dev`

Expected:
- Preprocessor panel appears in sidebar
- Panel loads and displays two textareas
- Can enter text and save on blur
- Test preview works

**Step 5: Commit**

```bash
git add src/components/PreprocessorPanel.vue
git add src/components/Sidebar.vue
git add src/App.vue
git commit -m "feat(ui): add PreprocessorPanel component

- Add panel for managing replacements and usernames lists
- Auto-save on blur functionality
- Test preview for preprocessing
- Info banner about live replacement
- Add to sidebar menu

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 7: Integration Testing

**Files:**
- No new files, manual testing

**Step 1: Test live replacement in interception mode**

1. Build the app: `npm run tauri build` or `npm run dev`
2. Open the app
3. Navigate to "Препроцессор" panel
4. Add replacements:
   ```
   name Alice
   greeting Hello there
   cond condition
   ```
5. Add usernames:
   ```
   john John Doe
   admin Administrator
   ```
6. Click away to save (blur)
7. Enable interception mode (Ctrl+Shift+F1)
8. In floating window, type: `\g` then press space

**Expected:** Text instantly changes to "Hello there"

9. Continue typing: ` @j` then space

**Expected:** Text changes to "Hello there John Doe"

**Step 2: Test manual input preprocessing**

1. Navigate to "Ввод" panel
2. Enter: `\greeting \name, message from @john`
3. Click "Озвучить"

Expected: TTS says "Hello there Alice, message from John Doe"

**Step 3: Test edge cases**

- Empty replacements file - text passes through unchanged
- Unknown `\word` - keeps `\word` in output
- Unknown `@username` - keeps `@username` in output
- Mixed known and unknown - replace only known ones
- Multiple replacements in one sentence
- Special characters in replacement values
- Cyrillic characters in keys and values

**Step 4: Test file persistence**

1. Set up replacements and usernames
2. Close the app
3. Reopen the app
4. Navigate to "Препроцессор" panel

Expected: All replacements and usernames are preserved

**Step 5: Test replacement reload**

1. Type `\old ` (which has a replacement)
2. See it replaced
3. Go to Preprocessor panel, change the replacement
4. Go back to floating window
5. Type `\old ` again

Expected: New replacement is used

**Step 6: Create test documentation**

Create: `docs/preprocessor-testing.md`

```markdown
# Preprocessor Testing Guide

## Test Cases

### Live Replacement (Interception Mode)
- [ ] Type `\name ` → instantly replaced to "Alice"
- [ ] Type `@user ` → instantly replaced to "John Doe"
- [ ] Multiple replacements in one sentence
- [ ] Mixed replacements and usernames

### Manual Input Mode
- [ ] Enter `\name @user` in Ввод panel
- [ ] Click "Озвучить"
- [ ] TTS says replaced text

### Edge Cases
- [ ] Unknown `\word` - keeps `\word`
- [ ] Unknown `@username` - keeps `@username`
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

### Files Location
Replacements: `%APPDATA%\ttsbard\replacements.txt`
Usernames: `%APPDATA%\ttsbard\usernames.txt`
```

**Step 7: Commit documentation**

```bash
git add docs/preprocessor-testing.md
git commit -m "docs: add preprocessor testing guide

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 8: Update Documentation

**Files:**
- Modify: `docs/INDEX.md`
- Modify: `docs/architecture.md`
- Modify: `docs/vue-components.md`

**Step 1: Update INDEX.md**

Add to the features list:

```markdown
- **Текстовый препроцессор** - Мгновенная замена слов при пробеле после `\ключ` или `@юзернейм`
```

**Step 2: Update architecture.md**

Add new section:

````markdown
## Text Preprocessing

The text preprocessing system provides instant word replacement in the floating window.

### Location
- Backend: `src-tauri/src/preprocessor/`
- Frontend: `src/components/PreprocessorPanel.vue`
- Hook Integration: `src-tauri/src/hook.rs`
- Commands: `src-tauri/src/commands/preprocessor.rs`

### Live Replacement Flow (Interception Mode)
```
User types: \name ↓
User presses SPACE ↓
Keyboard hook detects pattern ↓
Check_and_replace_end() finds replacement ↓
Replace text in AppState ↓
Emit UpdateFloatingText event ↓
Floating window shows: "Alice"
```

### Fallback Flow (Manual Input / TTS)
```
Input Text
    ↓
TextPreprocessor::process()
    ↓
Replace all \word and @username patterns
    ↓
Preprocessed Text → TTS Synthesis
```

### Pattern Matching
- `\word` patterns match against replacements list
- `@username` patterns match against usernames list
- Replacement triggered by spacebar press in interception mode

### Storage
- Replacements: `%APPDATA%\ttsbard\replacements.txt`
- Usernames: `%APPDATA%\ttsbard\usernames.txt`
- Format: `key value` per line (space-separated), `#` for comments
- Lines without space are skipped as invalid
````

**Step 3: Update vue-components.md**

Add component documentation:

````markdown
## PreprocessorPanel

**Location:** `src/components/PreprocessorPanel.vue`

**Purpose:** Manage text preprocessing rules for dynamic word replacement.

**Features:**
- Two textareas for replacements and usernames lists
- Auto-save on blur (focus loss)
- Live preview of preprocessing
- Format: `key value` per line (space-separated)
- Info banner about live replacement behavior

**Props:** None

**Emits:** None

**Tauri Commands Used:**
- `get_replacements` / `save_replacements`
- `get_usernames` / `save_usernames`
- `preview_preprocessing`

**Example Usage:**
```
Replacements:
name Alice
greeting Hello

Usernames:
john John Doe
admin Administrator

Type in floating window: "\greeting @admin "
Instantly becomes: "Hello Administrator "
```
````

**Step 4: Commit documentation updates**

```bash
git add docs/
git commit -m "docs: update docs for preprocessor feature

- Add preprocessor to INDEX
- Document live replacement architecture
- Add PreprocessorPanel to vue-components.md

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Complexity Analysis

### Key Implementation Details

1. **Regex Pattern Matching**
   - `r"\\(\w+)\s*$"` - matches `\word` at end of text (before space)
   - `r"@(\w+)\s*$"` - matches `@username` at end of text (before space)
   - Using `\s*$` ensures we only trigger on trailing patterns (after space press)

2. **State Caching**
   - Preprocessor cached in AppState to avoid file I/O on every keystroke
   - Reloaded when settings change via `save_*` commands

3. **Event Flow**
   - Space press → pattern check → text replacement → event emit → UI update
   - All happens within the keyboard hook callback (same thread)

4. **Double Safety**
   - Live replacement in hook for instant feedback
   - Fallback preprocessing before TTS for any edge cases

### Potential Challenges

1. **Timing Issues**
   - Space character needs to be consumed after replacement
   - Solution: Don't append space when replacement occurs

2. **Unicode Word Boundaries**
   - `\w+` might not match all languages
   - Solution: Use `(?u)\w+` for Unicode-aware matching if needed

3. **Race Conditions**
   - Preprocessor reload during active hook
   - Solution: Arc<Mutex<>> ensures thread-safe access

4. **Multiple Spaces**
   - What if user types multiple spaces?
   - Solution: Pattern uses `\s*$` so it only matches once per space

### Extension Ideas

1. **Case-insensitive matching** - Optional flag
2. **Tab completion** - Show replacement options
3. **Sound feedback** - Play sound on replacement
4. **Replacement history** - Undo last replacement
5. **Multi-word triggers** - `\multi word` → replacement
6. **Variables** - `{date}`, `{time}` expansion

---

## Summary

This implementation adds a complete text preprocessing system with **live replacement**:

- 8 core tasks, each with 3-5 steps
- Backend Rust module with comprehensive tests
- Keyboard hook integration for instant replacement
- Tauri commands for file I/O with auto-reload
- Vue UI panel with auto-save
- Double safety: live replacement + TTS fallback
- Full documentation

**Key Feature:** Type `\name ` in floating window → BOOM → "Alice" appears instantly!

Estimated complexity: Medium
Estimated time: 4-5 hours for full implementation
