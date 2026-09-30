mod numbers;
mod prefix;
mod replacer;

pub use numbers::process_numbers;
pub use prefix::parse_prefix;
pub use replacer::TextPreprocessor;

#[cfg(test)]
pub(crate) use replacer::ReplacementList;

use anyhow::Result;
use std::path::PathBuf;

/// Get the appdata directory for preprocessor files
pub fn get_preprocessor_dir() -> Result<PathBuf> {
    let config_dir = crate::paths::config_root()?;

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
