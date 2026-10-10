//! Shared, versioned application secret store. Windows current-user DPAPI;
//! no plaintext fallback. Each integration owns a typed section.
use crate::config::{config_write_lock, replace_file_atomically};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub(crate) const FILE_MAGIC: &[u8] = b"TTSBARD-SECRETS-V1\n";
const VERSION: u32 = 1;
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SecretsError {
    #[error("Secret encryption is only supported on Windows")]
    #[cfg(not(windows))]
    UnsupportedPlatform,
    #[error("Secrets could not be encrypted or decrypted")]
    Crypto,
    #[error("Secrets file is corrupt or has an unknown format")]
    Malformed,
    #[error("Secrets IO failure: {0}")]
    Io(String),
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u32,
    sections: BTreeMap<String, serde_json::Value>,
}

pub struct SecretsStore {
    path: PathBuf,
}
impl SecretsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn read_document(&self) -> Result<Document, SecretsError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Document {
                    version: VERSION,
                    sections: BTreeMap::new(),
                })
            }
            Err(e) => return Err(SecretsError::Io(e.to_string())),
        };
        let encrypted = bytes
            .strip_prefix(FILE_MAGIC)
            .ok_or(SecretsError::Malformed)?;
        let plaintext = dpapi_unprotect(encrypted)?;
        let document: Document =
            serde_json::from_slice(&plaintext).map_err(|_| SecretsError::Malformed)?;
        if document.version != VERSION {
            return Err(SecretsError::Malformed);
        }
        Ok(document)
    }

    pub fn load<T: DeserializeOwned>(&self, section: &str) -> Result<Option<T>, SecretsError> {
        let _guard = config_write_lock().lock();
        self.read_document()?
            .sections
            .remove(section)
            .map(|value| serde_json::from_value(value).map_err(|_| SecretsError::Malformed))
            .transpose()
    }

    pub fn save<T: Serialize>(&self, section: &str, value: &T) -> Result<(), SecretsError> {
        let value = serde_json::to_value(value).map_err(|_| SecretsError::Malformed)?;
        self.update(|document| {
            document.sections.insert(section.to_owned(), value);
        })
    }

    pub fn remove(&self, section: &str) -> Result<(), SecretsError> {
        self.update(|document| {
            document.sections.remove(section);
        })
    }

    fn update(&self, change: impl FnOnce(&mut Document)) -> Result<(), SecretsError> {
        // Lock the entire read/modify/write, including across independent instances.
        let _guard = config_write_lock().lock();
        let mut document = self.read_document()?;
        change(&mut document);
        let json = serde_json::to_vec(&document).map_err(|_| SecretsError::Malformed)?;
        let encrypted = dpapi_protect(&json)?;
        let mut blob = FILE_MAGIC.to_vec();
        blob.extend_from_slice(&encrypted);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| SecretsError::Io(e.to_string()))?;
        }
        replace_file_atomically(&self.path, &blob).map_err(|e| SecretsError::Io(e.to_string()))
    }
}
#[cfg(all(test, windows))]
mod tests {
    use super::*;
    fn path() -> PathBuf {
        std::env::temp_dir()
            .join(format!(
                "ttsbard-secrets-{}-{}",
                std::process::id(),
                rand::random::<u64>()
            ))
            .join("secrets.dat")
    }
    #[test]
    fn sections_roundtrip_and_removal_preserve_other_integrations() {
        let path = path();
        let store = SecretsStore::new(&path);
        assert!(store.load::<String>("twitch").unwrap().is_none());
        store.save("twitch", &"secret-token-one").unwrap();
        SecretsStore::new(&path)
            .save("other", &"secret-token-two")
            .unwrap();
        assert_eq!(
            store.load::<String>("twitch").unwrap().as_deref(),
            Some("secret-token-one")
        );
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(FILE_MAGIC));
        assert!(!String::from_utf8_lossy(&bytes).contains("secret-token"));
        store.remove("twitch").unwrap();
        assert!(store.load::<String>("twitch").unwrap().is_none());
        assert_eq!(
            store.load::<String>("other").unwrap().as_deref(),
            Some("secret-token-two")
        );
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn concurrent_writers_keep_every_section() {
        let path = path();
        std::thread::scope(|scope| {
            for i in 0..12 {
                let path = &path;
                scope.spawn(move || {
                    SecretsStore::new(path)
                        .save(&format!("section-{i}"), &i)
                        .unwrap()
                });
            }
        });
        for i in 0..12 {
            assert_eq!(
                SecretsStore::new(&path)
                    .load::<i32>(&format!("section-{i}"))
                    .unwrap(),
                Some(i)
            );
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
    #[test]
    fn corrupt_or_future_documents_are_not_overwritten() {
        let path = path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        for blob in [b"invalid".to_vec(), {
            let mut bytes = FILE_MAGIC.to_vec();
            bytes.extend(dpapi_protect(br#"{"version":99,"sections":{}}"#).unwrap());
            bytes
        }] {
            std::fs::write(&path, &blob).unwrap();
            assert!(SecretsStore::new(&path).save("twitch", &"token").is_err());
            assert_eq!(std::fs::read(&path).unwrap(), blob);
        }
        std::fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }
}

#[cfg(windows)]
fn dpapi_protect(plaintext: &[u8]) -> Result<Vec<u8>, SecretsError> {
    use windows::core::PCWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: plaintext.len() as u32,
        pbData: plaintext.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let protected = unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| SecretsError::Crypto)?;
        if output.pbData.is_null() {
            return Err(SecretsError::Crypto);
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut core::ffi::c_void));
        bytes
    };
    Ok(protected)
}

#[cfg(windows)]
fn dpapi_unprotect(protected: &[u8]) -> Result<Vec<u8>, SecretsError> {
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let input = CRYPT_INTEGER_BLOB {
        cbData: protected.len() as u32,
        pbData: protected.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    let mut description = PWSTR::null();
    let plaintext = unsafe {
        CryptUnprotectData(
            &input,
            Some(&mut description),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|_| SecretsError::Crypto)?;
        if output.pbData.is_null() {
            return Err(SecretsError::Crypto);
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut core::ffi::c_void));
        if !description.0.is_null() {
            let _ = LocalFree(HLOCAL(description.0 as *mut core::ffi::c_void));
        }
        bytes
    };
    Ok(plaintext)
}

#[cfg(not(windows))]
fn dpapi_protect(_plaintext: &[u8]) -> Result<Vec<u8>, SecretsError> {
    Err(SecretsError::UnsupportedPlatform)
}

#[cfg(not(windows))]
fn dpapi_unprotect(_protected: &[u8]) -> Result<Vec<u8>, SecretsError> {
    Err(SecretsError::UnsupportedPlatform)
}
