//! Twitch schema adapter for the shared encrypted application secret store.
//!
//! Owns a versioned JSON document of the local application's client id/secret,
//! the bot account grant, the canonical collection of authorized channel grants
//! and an optional app access token. Stored as the `twitch` section of
//! `secrets.dat`; encryption and atomic persistence belong to `secrets`.
//! The caller supplies the file path; this adapter never talks to the frontend.
//!
//! # Ownership semantics
//!
//! Tokens are bound to the `(client, user)` pair they were issued for. Changing
//! the client id therefore invalidates both account grants and any cached app
//! token. The authorization coordinator clears them whenever the client id
//! differs from the previously stored one.

use std::fmt;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Current on-disk format version. Bump only for a breaking layout change.
pub const FORMAT_VERSION: u32 = 1;

/// Shared envelope prefix used by roundtrip tests.
#[cfg(test)]
use crate::secrets::FILE_MAGIC;

/// Current UNIX epoch time in whole seconds.
pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A string whose contents are secret and must never appear in logs, debug
/// output, errors or URLs.
///
/// `Debug` is implemented manually and always prints `<redacted>`; the value is
/// only reachable through [`SecretString::expose`], used by the HTTP client when
/// building authenticated request bodies. The backing buffer is zeroed on drop.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SecretString(String);

impl SecretString {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// Borrow the raw secret. Callers must not log, format or leak it.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::new(value.to_string())
    }
}

impl Default for SecretString {
    fn default() -> Self {
        Self::new(String::new())
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.fill(0);
    }
}

/// A single account authorization grant (bot or broadcaster).
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AccountGrant {
    /// Twitch user id of the authorized account.
    pub user_id: String,
    /// Twitch login (lowercase display name) of the authorized account.
    pub login: String,
    /// OAuth user access token.
    pub access_token: SecretString,
    /// OAuth refresh token.
    pub refresh_token: SecretString,
    /// Unix epoch seconds at which `access_token` expires.
    pub expires_at: i64,
    /// Scopes actually granted (space-separated by Twitch, stored as a list).
    pub scopes: Vec<String>,
}

impl fmt::Debug for AccountGrant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountGrant")
            .field("user_id", &self.user_id)
            .field("login", &self.login)
            .field("expires_at", &self.expires_at)
            .field("scopes", &self.scopes)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

/// A cached client-credentials app access token.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct AppToken {
    pub access_token: SecretString,
    /// Unix epoch seconds at which the token expires.
    pub expires_at: i64,
}

impl fmt::Debug for AppToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppToken")
            .field("expires_at", &self.expires_at)
            .field("access_token", &"<redacted>")
            .finish()
    }
}

/// Default format version used by [`StoredCredentialsWire`] when the field is
/// absent from an otherwise valid document.
fn default_format_version() -> u32 {
    FORMAT_VERSION
}

/// Serialized form of [`StoredCredentials`], tolerant of the legacy
/// `broadcaster` field. Deserialization migrates a legacy single-broadcaster
/// blob into the canonical `channels` list plus `selected_channel_id`.
#[derive(Deserialize)]
struct StoredCredentialsWire {
    #[serde(default = "default_format_version")]
    version: u32,
    #[serde(default)]
    client_id: String,
    #[serde(default)]
    client_secret: SecretString,
    #[serde(default)]
    bot: Option<AccountGrant>,
    #[serde(default)]
    channels: Vec<AccountGrant>,
    #[serde(default)]
    selected_channel_id: Option<String>,
    /// Legacy version-1 field, read-only: migrated into `channels`. Never
    /// written back.
    #[serde(default)]
    broadcaster: Option<AccountGrant>,
    #[serde(default)]
    app_token: Option<AppToken>,
}

impl From<StoredCredentialsWire> for StoredCredentials {
    fn from(wire: StoredCredentialsWire) -> Self {
        let mut channels = wire.channels;
        let mut selected_channel_id = wire.selected_channel_id;
        // Legacy version-1 blobs stored exactly one broadcaster grant and no
        // canonical list; migrate it and select it so existing installs keep
        // sending to the same channel after the storage upgrade.
        if channels.is_empty() {
            if let Some(broadcaster) = wire.broadcaster {
                selected_channel_id = Some(broadcaster.user_id.clone());
                channels.push(broadcaster);
            }
        }
        let mut creds = StoredCredentials {
            version: wire.version,
            client_id: wire.client_id,
            client_secret: wire.client_secret,
            bot: wire.bot,
            channels,
            selected_channel_id,
            app_token: wire.app_token,
        };
        creds.normalize();
        creds
    }
}

/// The versioned, encrypted credential document.
///
/// `channels` is the canonical collection of authorized channel grants and
/// `selected_channel_id` names exactly one of them as the delivery recipient
/// (or `None` when nothing is selected). The legacy `broadcaster` field is
/// still accepted on read and migrated into `channels`; it is never written.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(from = "StoredCredentialsWire")]
pub struct StoredCredentials {
    pub version: u32,
    pub client_id: String,
    pub client_secret: SecretString,
    pub bot: Option<AccountGrant>,
    pub channels: Vec<AccountGrant>,
    pub selected_channel_id: Option<String>,
    pub app_token: Option<AppToken>,
}

impl Default for StoredCredentials {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            client_id: String::new(),
            client_secret: SecretString::new(String::new()),
            bot: None,
            channels: Vec::new(),
            selected_channel_id: None,
            app_token: None,
        }
    }
}

impl StoredCredentials {
    /// The channel currently selected as the delivery recipient, if any.
    pub fn selected_channel(&self) -> Option<&AccountGrant> {
        let id = self.selected_channel_id.as_deref()?;
        self.channels.iter().find(|c| c.user_id == id)
    }

    /// Whether a channel grant with `user_id` is stored.
    pub fn has_channel(&self, user_id: &str) -> bool {
        self.channels.iter().any(|c| c.user_id == user_id)
    }

    /// Insert or update a channel grant. When no valid channel is currently
    /// selected, the upserted channel becomes the selection; an existing valid
    /// selection is preserved. Re-authorizing the same user id updates the
    /// grant in place without creating a duplicate.
    pub(crate) fn upsert_channel(&mut self, grant: AccountGrant) {
        let user_id = grant.user_id.clone();
        match self.channels.iter_mut().find(|c| c.user_id == user_id) {
            Some(existing) => *existing = grant,
            None => self.channels.push(grant),
        }
        if !self.selection_is_valid() {
            self.selected_channel_id = Some(user_id);
        }
    }

    /// Replace the stored grant for an already-present channel id. A late
    /// refresh for a removed channel is a no-op and never changes selection.
    pub(crate) fn replace_channel_grant(&mut self, grant: AccountGrant) {
        if let Some(existing) = self
            .channels
            .iter_mut()
            .find(|c| c.user_id == grant.user_id)
        {
            *existing = grant;
        }
    }

    /// Remove a channel grant. Removing the selected channel clears the
    /// selection without falling back to any other channel. Returns whether a
    /// grant was removed.
    pub(crate) fn remove_channel(&mut self, user_id: &str) -> bool {
        let before = self.channels.len();
        self.channels.retain(|c| c.user_id != user_id);
        let removed = self.channels.len() != before;
        if removed && self.selected_channel_id.as_deref() == Some(user_id) {
            self.selected_channel_id = None;
        }
        removed
    }

    /// Select an existing channel as the delivery recipient. Returns `false`
    /// for an unknown id.
    pub(crate) fn select_channel(&mut self, user_id: &str) -> bool {
        if !self.has_channel(user_id) {
            return false;
        }
        self.selected_channel_id = Some(user_id.to_string());
        true
    }

    /// Whether `selected_channel_id` names a channel that is actually stored.
    fn selection_is_valid(&self) -> bool {
        self.selected_channel_id
            .as_ref()
            .map(|id| self.channels.iter().any(|c| &c.user_id == id))
            .unwrap_or(false)
    }

    /// Drop invalid or duplicate channel ids and clear a dangling selection,
    /// so a deserialized document can never point at a non-existent recipient.
    fn normalize(&mut self) {
        let mut seen = std::collections::HashSet::new();
        let mut deduped = Vec::with_capacity(self.channels.len());
        for grant in self.channels.drain(..) {
            if grant.user_id.is_empty() {
                continue;
            }
            if seen.insert(grant.user_id.clone()) {
                deduped.push(grant);
            }
        }
        self.channels = deduped;
        if self.selected_channel_id.is_some() && !self.selection_is_valid() {
            self.selected_channel_id = None;
        }
    }

    /// Nonsecret summary of the current authorization state, safe for the UI.
    #[cfg(test)]
    pub fn auth_status(&self) -> AuthStatus {
        AuthStatus {
            client_configured: !self.client_id.is_empty() && !self.client_secret.is_empty(),
            bot_login: self.bot.as_ref().map(|g| g.login.clone()),
            broadcaster_login: self.selected_channel().map(|g| g.login.clone()),
            app_token_available: self.app_token.is_some(),
        }
    }
}

impl fmt::Debug for StoredCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StoredCredentials")
            .field("version", &self.version)
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("bot", &self.bot)
            .field("channels", &self.channels)
            .field("selected_channel_id", &self.selected_channel_id)
            .field("app_token", &self.app_token)
            .finish()
    }
}

/// Nonsecret authorization status DTO (no tokens, no secret).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg(test)]
pub struct AuthStatus {
    pub client_configured: bool,
    pub bot_login: Option<String>,
    pub broadcaster_login: Option<String>,
    pub app_token_available: bool,
}

/// Twitch schema adapter for the shared application secret store.
pub use crate::secrets::SecretsError as CredentialsError;
pub struct CredentialsStore {
    store: crate::secrets::SecretsStore,
}
impl CredentialsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            store: crate::secrets::SecretsStore::new(path),
        }
    }
    pub fn load(&self) -> Result<Option<StoredCredentials>, CredentialsError> {
        let credentials: Option<StoredCredentials> = self.store.load("twitch")?;
        if credentials
            .as_ref()
            .is_some_and(|c| c.version != FORMAT_VERSION)
        {
            return Err(CredentialsError::Malformed);
        }
        Ok(credentials)
    }
    #[cfg(test)]
    pub fn load_or_default(&self) -> Result<StoredCredentials, CredentialsError> {
        Ok(self.load()?.unwrap_or_default())
    }
    #[cfg(test)]
    pub fn remove(&self) -> Result<(), CredentialsError> {
        self.store.remove("twitch")
    }
    pub fn save(&self, creds: &StoredCredentials) -> Result<(), CredentialsError> {
        self.store.save("twitch", creds)
    }

    /// Store the client credentials, invalidating both grants and the app token
    /// whenever the client id changes (see module ownership docs).
    #[cfg(test)]
    pub fn set_client_credentials(
        &self,
        client_id: String,
        client_secret: SecretString,
    ) -> Result<StoredCredentials, CredentialsError> {
        let mut creds = self.load_or_default()?;
        if creds.client_id != client_id {
            creds.bot = None;
            creds.channels.clear();
            creds.selected_channel_id = None;
            creds.app_token = None;
        }
        creds.client_id = client_id;
        creds.client_secret = client_secret;
        self.save(&creds)?;
        Ok(creds)
    }

    /// Replace the bot account grant.
    #[cfg(test)]
    pub fn set_bot_grant(
        &self,
        grant: AccountGrant,
    ) -> Result<StoredCredentials, CredentialsError> {
        let mut creds = self.load_or_default()?;
        creds.bot = Some(grant);
        self.save(&creds)?;
        Ok(creds)
    }

    /// Cache an app access token.
    #[cfg(test)]
    pub fn set_app_token(&self, token: AppToken) -> Result<StoredCredentials, CredentialsError> {
        let mut creds = self.load_or_default()?;
        creds.app_token = Some(token);
        self.save(&creds)?;
        Ok(creds)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_file(label: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "ttsbard-twitch-creds-{}-{}-{}",
            label,
            std::process::id(),
            stamp
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("credentials.bin")
    }

    fn sample_creds() -> StoredCredentials {
        StoredCredentials {
            version: FORMAT_VERSION,
            client_id: "client-id-123".into(),
            client_secret: SecretString::from("client-secret-456"),
            bot: Some(AccountGrant {
                user_id: "111".into(),
                login: "bot".into(),
                access_token: SecretString::from("bot-access"),
                refresh_token: SecretString::from("bot-refresh"),
                expires_at: 999,
                scopes: vec!["user:write:chat".into(), "user:bot".into()],
            }),
            channels: Vec::new(),
            selected_channel_id: None,
            app_token: None,
        }
    }

    fn channel_grant(user_id: &str, login: &str, access: &str) -> AccountGrant {
        AccountGrant {
            user_id: user_id.into(),
            login: login.into(),
            access_token: SecretString::from(access),
            refresh_token: SecretString::from("refresh"),
            expires_at: 999,
            scopes: vec!["channel:bot".into()],
        }
    }

    #[test]
    fn secret_string_debug_never_reveals_value() {
        let secret = SecretString::from("super-secret-value");
        let rendered = format!("{:?}", secret);
        assert!(!rendered.contains("super-secret-value"));
        assert!(rendered.contains("redacted"));
    }

    #[test]
    fn account_grant_debug_redacts_tokens() {
        let grant = sample_creds().bot.unwrap();
        let rendered = format!("{:?}", grant);
        assert!(!rendered.contains("bot-access"));
        assert!(!rendered.contains("bot-refresh"));
        assert!(rendered.contains("bot"));
    }

    #[test]
    fn missing_file_loads_as_none() {
        let path = tmp_file("missing");
        let store = CredentialsStore::new(path.clone());
        assert!(store.load().unwrap().is_none());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn auth_status_exposes_only_nonsecret_fields() {
        let status = sample_creds().auth_status();
        assert!(status.client_configured);
        assert_eq!(status.bot_login.as_deref(), Some("bot"));
        assert_eq!(status.broadcaster_login, None);
        assert!(!status.app_token_available);
    }

    #[cfg(windows)]
    #[test]
    fn encrypted_roundtrip_and_no_plaintext_on_disk() {
        let path = tmp_file("roundtrip");
        let store = CredentialsStore::new(path.clone());
        let creds = sample_creds();

        store.save(&creds).unwrap();

        let raw = std::fs::read(&path).unwrap();
        assert!(raw.starts_with(FILE_MAGIC));
        let raw_text = String::from_utf8_lossy(&raw);
        assert!(!raw_text.contains("client-secret-456"));
        assert!(!raw_text.contains("bot-access"));
        assert!(!raw_text.contains("bot-refresh"));

        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.client_id, "client-id-123");
        assert_eq!(loaded.client_secret.expose(), "client-secret-456");
        let bot = loaded.bot.as_ref().unwrap();
        assert_eq!(bot.access_token.expose(), "bot-access");
        assert_eq!(bot.refresh_token.expose(), "bot-refresh");
        assert_eq!(bot.scopes, vec!["user:write:chat", "user:bot"]);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn unsupported_document_version_is_rejected() {
        let path = tmp_file("version");
        let store = CredentialsStore::new(path.clone());
        let mut credentials = sample_creds();
        credentials.version = FORMAT_VERSION + 1;
        store.save(&credentials).unwrap();
        assert!(matches!(store.load(), Err(CredentialsError::Malformed)));
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[cfg(windows)]
    #[test]
    fn changing_client_id_invalidates_grants() {
        let path = tmp_file("invalidate");
        let store = CredentialsStore::new(path.clone());
        let grant = AccountGrant {
            user_id: "1".into(),
            login: "bot".into(),
            access_token: SecretString::from("a"),
            refresh_token: SecretString::from("r"),
            expires_at: 0,
            scopes: vec![],
        };

        store
            .set_client_credentials("id-a".into(), SecretString::from("secret-a"))
            .unwrap();
        store.set_bot_grant(grant).unwrap();
        store
            .set_app_token(AppToken {
                access_token: SecretString::from("t"),
                expires_at: 0,
            })
            .unwrap();

        // Same id: grants and app token are preserved.
        store
            .set_client_credentials("id-a".into(), SecretString::from("secret-a2"))
            .unwrap();
        let creds = store.load().unwrap().unwrap();
        assert!(creds.bot.is_some());
        assert!(creds.app_token.is_some());

        // Changed id: both grants and the app token are invalidated.
        store
            .set_client_credentials("id-b".into(), SecretString::from("secret-b"))
            .unwrap();
        let creds = store.load().unwrap().unwrap();
        assert!(creds.bot.is_none());
        assert!(creds.channels.is_empty());
        assert!(creds.selected_channel_id.is_none());
        assert!(creds.app_token.is_none());

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn legacy_broadcaster_blob_migrates_to_channels_and_selection() {
        let legacy = serde_json::json!({
            "version": FORMAT_VERSION,
            "client_id": "client-id-123",
            "client_secret": "client-secret-456",
            "bot": {
                "user_id": "111",
                "login": "bot",
                "access_token": "bot-access",
                "refresh_token": "bot-refresh",
                "expires_at": 999,
                "scopes": ["user:write:chat"]
            },
            "broadcaster": {
                "user_id": "222",
                "login": "oldchannel",
                "access_token": "bc-access",
                "refresh_token": "bc-refresh",
                "expires_at": 999,
                "scopes": ["channel:bot"]
            },
            "app_token": null
        });
        let creds: StoredCredentials = serde_json::from_value(legacy).unwrap();
        assert_eq!(creds.channels.len(), 1);
        assert_eq!(creds.channels[0].user_id, "222");
        assert_eq!(creds.channels[0].login, "oldchannel");
        assert_eq!(creds.selected_channel_id.as_deref(), Some("222"));
        assert_eq!(creds.selected_channel().unwrap().login, "oldchannel");
        assert!(creds.bot.is_some());
    }

    #[test]
    fn canonical_roundtrip_preserves_channels_and_selection() {
        let mut creds = StoredCredentials {
            client_id: "client-id-123".into(),
            client_secret: SecretString::from("client-secret-456"),
            ..StoredCredentials::default()
        };
        creds.upsert_channel(channel_grant("222", "owner", "bc-access"));
        creds.upsert_channel(channel_grant("333", "second", "bc2-access"));
        creds.select_channel("333");
        let encoded = serde_json::to_vec(&creds).unwrap();
        let decoded: StoredCredentials = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(decoded, creds);
        assert_eq!(decoded.selected_channel_id.as_deref(), Some("333"));
        assert_eq!(decoded.channels.len(), 2);
    }

    #[test]
    fn new_serialization_omits_legacy_broadcaster_field() {
        let mut creds = StoredCredentials::default();
        creds.upsert_channel(channel_grant("222", "owner", "bc-access"));
        let value = serde_json::to_value(&creds).unwrap();
        assert!(value.get("broadcaster").is_none());
        assert!(value.get("channels").is_some());
        assert_eq!(value["selected_channel_id"], serde_json::json!("222"));
    }

    #[test]
    fn first_channel_becomes_selected_later_adds_preserve_selection() {
        let mut creds = StoredCredentials::default();
        assert!(creds.selected_channel_id.is_none());
        creds.upsert_channel(channel_grant("1", "a", "a-tok"));
        assert_eq!(creds.selected_channel_id.as_deref(), Some("1"));
        creds.upsert_channel(channel_grant("2", "b", "b-tok"));
        assert_eq!(creds.selected_channel_id.as_deref(), Some("1"));
        assert_eq!(creds.channels.len(), 2);
    }

    #[test]
    fn upsert_same_user_updates_without_duplicate() {
        let mut creds = StoredCredentials::default();
        creds.upsert_channel(channel_grant("1", "a", "a-tok"));
        creds.upsert_channel(channel_grant("2", "b", "b-tok"));
        creds.upsert_channel(channel_grant("1", "a", "a-tok-reauth"));
        assert_eq!(creds.channels.len(), 2);
        assert_eq!(creds.channels[0].access_token.expose(), "a-tok-reauth");
        assert_eq!(creds.selected_channel_id.as_deref(), Some("1"));
    }

    #[test]
    fn removing_selected_channel_leaves_no_selection_or_fallback() {
        let mut creds = StoredCredentials::default();
        creds.upsert_channel(channel_grant("1", "a", "a-tok"));
        creds.upsert_channel(channel_grant("2", "b", "b-tok"));
        creds.select_channel("1");
        assert!(creds.remove_channel("1"));
        assert!(creds.selected_channel_id.is_none());
        assert_eq!(creds.channels.len(), 1);
        assert_eq!(creds.channels[0].user_id, "2");
        assert!(creds.selected_channel().is_none());
    }

    #[test]
    fn removing_nonselected_channel_preserves_selection() {
        let mut creds = StoredCredentials::default();
        creds.upsert_channel(channel_grant("1", "a", "a-tok"));
        creds.upsert_channel(channel_grant("2", "b", "b-tok"));
        creds.select_channel("1");
        assert!(creds.remove_channel("2"));
        assert_eq!(creds.selected_channel_id.as_deref(), Some("1"));
        assert!(creds.selected_channel().is_some());
    }

    #[test]
    fn selecting_unknown_channel_is_rejected() {
        let mut creds = StoredCredentials::default();
        creds.upsert_channel(channel_grant("1", "a", "a-tok"));
        assert!(!creds.select_channel("999"));
        assert_eq!(creds.selected_channel_id.as_deref(), Some("1"));
    }

    #[test]
    fn normalization_drops_invalid_ids_and_dangling_selection() {
        let wire = serde_json::json!({
            "version": FORMAT_VERSION,
            "client_id": "c",
            "client_secret": "s",
            "channels": [
                { "user_id": "", "login": "empty", "access_token": "x", "refresh_token": "y", "expires_at": 0, "scopes": [] },
                { "user_id": "1", "login": "a", "access_token": "x", "refresh_token": "y", "expires_at": 0, "scopes": [] },
                { "user_id": "1", "login": "a-dup", "access_token": "x", "refresh_token": "y", "expires_at": 0, "scopes": [] }
            ],
            "selected_channel_id": "missing"
        });
        let creds: StoredCredentials = serde_json::from_value(wire).unwrap();
        assert_eq!(creds.channels.len(), 1);
        assert_eq!(creds.channels[0].login, "a");
        assert!(creds.selected_channel_id.is_none());
    }

    #[cfg(not(windows))]
    #[test]
    fn save_is_unsupported_elsewhere() {
        let path = tmp_file("unsupported");
        let store = CredentialsStore::new(path.clone());
        assert_eq!(
            store.save(&StoredCredentials::default()),
            Err(CredentialsError::UnsupportedPlatform)
        );
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
