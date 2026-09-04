//! Read the installed Windows font families once, before the frontend starts.

use tracing::warn;

/// Quick choices that stay selectable even when DirectWrite finds nothing.
const BUILTIN_FAMILIES: [&str; 2] = ["default", "system"];

#[derive(Debug)]
pub struct SystemFontCatalog {
    families: Vec<String>,
}

impl SystemFontCatalog {
    pub fn load() -> Self {
        match enumerate_system_font_families() {
            Ok(families) => Self::from_families(families),
            Err(error) => {
                warn!(%error, "Unable to enumerate Windows font families");
                Self::from_families(Vec::new())
            }
        }
    }

    /// Build the catalog from already enumerated names: the built-in quick
    /// choices plus every discovered family, normalized and deduplicated.
    fn from_families(discovered: Vec<String>) -> Self {
        let mut families = discovered;
        families.extend(BUILTIN_FAMILIES.map(str::to_owned));
        Self {
            families: normalize_font_families(families),
        }
    }

    pub fn families(&self) -> &[String] {
        &self.families
    }
}

#[cfg(windows)]
fn enumerate_system_font_families() -> anyhow::Result<Vec<String>> {
    use windows::Win32::Foundation::BOOL;
    use windows::Win32::Graphics::DirectWrite::{
        DWriteCreateFactory, IDWriteFactory, DWRITE_FACTORY_TYPE_SHARED,
    };

    /// Locale of the family names shown in the UI; index 0 (usually en-US)
    /// is the fallback when a family has no such name.
    const UI_LOCALE: &str = "ru-RU";

    unsafe {
        let factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
        let mut collection = None;
        factory.GetSystemFontCollection(&mut collection, BOOL(0))?;
        let collection =
            collection.ok_or_else(|| anyhow::anyhow!("DirectWrite returned no font collection"))?;

        let mut families = Vec::with_capacity(collection.GetFontFamilyCount() as usize);
        for index in 0..collection.GetFontFamilyCount() {
            let names = collection.GetFontFamily(index)?.GetFamilyNames()?;
            if names.GetCount() == 0 {
                continue;
            }
            families.push(family_display_name(&names, UI_LOCALE)?);
        }
        Ok(families)
    }
}

/// Pick the family name for the UI locale, or the first available name.
#[cfg(windows)]
unsafe fn family_display_name(
    names: &windows::Win32::Graphics::DirectWrite::IDWriteLocalizedStrings,
    locale: &str,
) -> anyhow::Result<String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::BOOL;

    let mut index = 0u32;
    let mut exists = BOOL(0);
    if names
        .FindLocaleName(&HSTRING::from(locale), &mut index, &mut exists)
        .is_err()
        || !exists.as_bool()
    {
        index = 0;
    }
    let length = names.GetStringLength(index)? as usize;
    let mut buffer = vec![0u16; length + 1];
    names.GetString(index, &mut buffer)?;
    Ok(String::from_utf16_lossy(&buffer[..length]))
}

#[cfg(not(windows))]
fn enumerate_system_font_families() -> anyhow::Result<Vec<String>> {
    Ok(Vec::new())
}

fn normalize_font_families(families: Vec<String>) -> Vec<String> {
    let mut names: Vec<_> = families
        .into_iter()
        .map(|name| name.trim().to_owned())
        .filter(|name| !name.is_empty())
        .collect();
    names.sort_by_cached_key(|name| name.to_lowercase());
    names.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
    names
}

#[cfg(test)]
mod tests {
    use super::{normalize_font_families, SystemFontCatalog};

    #[test]
    fn normalizes_and_sorts_font_names() {
        assert_eq!(
            normalize_font_families(vec![
                "  Arial ".into(),
                "Consolas".into(),
                "arial".into(),
                "".into()
            ]),
            vec!["Arial", "Consolas"],
        );
    }

    #[test]
    fn catalog_keeps_builtin_families_without_windows() {
        let catalog = SystemFontCatalog::from_families(Vec::new());
        assert_eq!(catalog.families(), ["default", "system"]);
    }

    #[test]
    fn catalog_merges_discovered_families_with_builtins() {
        let catalog = SystemFontCatalog::from_families(vec![
            "Segoe UI".into(),
            "Arial".into(),
            "system".into(),
            "  Calibri ".into(),
        ]);
        assert_eq!(
            catalog.families(),
            ["Arial", "Calibri", "default", "Segoe UI", "system"],
        );
    }
}
