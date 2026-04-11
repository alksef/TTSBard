use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

lazy_static::lazy_static! {
    /// Regex to match \word pattern at end of text
    static ref REPLACEMENT_PATTERN_END: Regex = Regex::new(r"\\(\w+)\s*$").unwrap();
    /// Regex to match %username pattern at end of text
    static ref USERNAME_PATTERN_END: Regex = Regex::new(r"%(\w+)\s*$").unwrap();
    /// Regex to match all \word patterns
    static ref REPLACEMENT_PATTERN_ALL: Regex = Regex::new(r"\\(\w+)").unwrap();
    /// Regex to match all %username patterns
    static ref USERNAME_PATTERN_ALL: Regex = Regex::new(r"%(\w+)").unwrap();
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
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

    /// Get all replacements as map (for UI display)
    #[allow(dead_code)]
    pub fn get_replacements_map(&self) -> &HashMap<String, String> {
        &self.replacements
    }

    /// Get all usernames as map (for UI display)
    pub fn get_usernames_map(&self) -> &HashMap<String, String> {
        &self.usernames
    }
}

/// Text preprocessor that applies replacements
#[derive(Clone)]
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
    #[allow(dead_code)]
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

        // Check for %username pattern at end
        if let Some(caps) = USERNAME_PATTERN_END.captures(text) {
            let key = &caps[1];
            if let Some(username) = self.replacements.get_username(key) {
                let pattern = format!("%{}", key);
                let result = text.replacen(&pattern, username, 1);
                return Some((pattern, result));
            }
        }

        None
    }

    /// Preprocess all text, replacing all \word and %username patterns
    pub fn process(&self, text: &str) -> String {
        let mut result = text.to_string();

        // Replace all \word patterns
        result = REPLACEMENT_PATTERN_ALL.replace_all(&result, |caps: &regex::Captures| {
            let key = &caps[1];
            if let Some(replacement) = self.replacements.get_replacement(key) {
                replacement.clone()
            } else {
                format!("\\{}", key)
            }
        }).to_string();

        // Replace all %username patterns
        result = USERNAME_PATTERN_ALL.replace_all(&result, |caps: &regex::Captures| {
            let key = &caps[1];
            if let Some(username) = self.replacements.get_username(key) {
                username.clone()
            } else {
                format!("%{}", key)
            }
        }).to_string();

        result
    }

    /// Get all replacements as map (for UI live replacement)
    pub fn get_replacements_map(&self) -> &HashMap<String, String> {
        self.replacements.get_replacements_map()
    }

    /// Get all usernames as map (for UI live replacement)
    pub fn get_usernames_map(&self) -> &HashMap<String, String> {
        self.replacements.get_usernames_map()
    }

    /// Get the total number of replacements (for status display)
    pub fn replacements_count(&self) -> usize {
        self.replacements.get_replacements_map().len() + self.replacements.get_usernames_map().len()
    }

    /// Reload replacements from files
    #[allow(dead_code)]
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
        let text = "Hey %user ";
        let matches: Vec<_> = USERNAME_PATTERN_END.find_iter(text).collect();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].as_str(), "%user ");
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
    fn test_process_usernames() {
        let mut replacements = ReplacementList::new();
        replacements.add_username("john".to_string(), "John Doe".to_string());
        replacements.add_username("admin".to_string(), "Administrator".to_string());

        let preprocessor = TextPreprocessor::new(replacements);

        let input = "Message from %john: hello %admin";
        let output = preprocessor.process(input);
        assert_eq!(output, "Message from John Doe: hello Administrator");
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
