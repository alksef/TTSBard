mod replacer;

pub use replacer::TextPreprocessor;

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
