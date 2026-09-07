use crate::config::DEFAULT_TTS_TIMEOUT_SECS;
use crate::events::EventSender;
#[cfg(debug_assertions)]
use crate::secret_log;
use crate::tts::engine::TtsEngine;
use crate::tts::proxy_utils;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tracing::{debug, error, trace};

/// Default output format compatible with the current decoder (no paid tier).
pub const DEFAULT_OUTPUT_FORMAT: &str = "mp3_44100_128";

/// Default voice-settings numeric values (all within `0.0..=1.0`).
pub const DEFAULT_STABILITY: f32 = 0.5;
pub const DEFAULT_SIMILARITY_BOOST: f32 = 0.75;
pub const DEFAULT_STYLE: f32 = 0.0;
pub const DEFAULT_USE_SPEAKER_BOOST: bool = true;

/// Explicit MVP allowlist of supported output formats.
pub const ALLOWED_OUTPUT_FORMATS: &[&str] = &[
    "mp3_44100_128",
    "mp3_44100_96",
    "mp3_44100_64",
    "mp3_22050_32",
];

/// Upper bound on the number of voice pages fetched per refresh.
pub const MAX_VOICE_PAGES: usize = 50;

/// Upper bound on how much of an error body we inspect (never the full body).
const MAX_ERROR_BODY_BYTES: usize = 4096;

/// Maximum response text emitted by the explicit debug-only HTTP diagnostic.
#[cfg(any(debug_assertions, test))]
const MAX_HTTP_DIAGNOSTIC_BODY_BYTES: usize = 64 * 1024;

/// Upper bound on a `detail.request_id` accepted for surfacing. Bounds the
/// validated hex identifier so an oversized or malicious value is dropped.
const MAX_REQUEST_ID_LEN: usize = 64;

/// Upper bound on a permission identifier accepted for surfacing. Bounds the
/// validated identifier so a remote token approaching the error-body cap is
/// never echoed into the UI.
const MAX_PERMISSION_ID_LEN: usize = 64;

/// Upper bound on a diagnostic code or other classification token accepted for
/// surfacing/logging. Bounds the validated token so a remote value approaching
/// the error-body cap is never echoed verbatim.
const MAX_DIAGNOSTIC_TOKEN_LEN: usize = 64;

/// Characters allowed in a surfaced/logged diagnostic token: ASCII lowercase
/// letters, digits, underscore and hyphen. Anything else is rejected so a
/// remote value can never smuggle markup or secret-looking text.
fn valid_api_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= MAX_DIAGNOSTIC_TOKEN_LEN
        && token
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Pure predicate shared with tests. Like the Silero diagnostic, activation is
/// deliberately strict so an accidentally inherited value does not enable it.
#[cfg(any(debug_assertions, test))]
fn should_log_elevenlabs_http(env_value: Option<&str>) -> bool {
    env_value == Some("1")
}

#[cfg(debug_assertions)]
fn elevenlabs_http_logging_enabled() -> bool {
    let value = std::env::var("TTSBARD_LOG_ELEVENLABS_HTTP").ok();
    should_log_elevenlabs_http(value.as_deref())
}

/// Preserve the request path and useful query parameters while hiding opaque
/// pagination cursors. ElevenLabs URLs never carry the API key in the query.
#[cfg(any(debug_assertions, test))]
fn safe_elevenlabs_url_for_log(url: &reqwest::Url) -> String {
    let mut safe = url.clone();
    if url.query().is_some() {
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(name, value)| {
                let value = if name == "next_page_token" {
                    "<redacted>".to_string()
                } else {
                    value.into_owned()
                };
                (name.into_owned(), value)
            })
            .collect();
        safe.set_query(None);
        let mut query = safe.query_pairs_mut();
        for (name, value) in pairs {
            query.append_pair(&name, &value);
        }
    }
    safe.to_string()
}

/// Render a bounded response body as one tracing field. Newlines and control
/// characters are escaped so a remote response cannot forge log records.
#[cfg(any(debug_assertions, test))]
fn bounded_http_body_for_log(body: &[u8], redactions: &[&str]) -> String {
    let mut raw = String::from_utf8_lossy(body).into_owned();
    for secret in redactions.iter().filter(|secret| !secret.is_empty()) {
        raw = raw.replace(secret, "<redacted>");
    }
    let mut take = raw.len().min(MAX_HTTP_DIAGNOSTIC_BODY_BYTES);
    while !raw.is_char_boundary(take) {
        take -= 1;
    }
    let mut rendered = raw[..take]
        .replace('\\', "\\\\")
        .replace('\r', "\\r")
        .replace('\n', "\\n")
        .replace('\0', "\\0");
    if raw.len() > take {
        rendered.push_str("...[truncated]");
    }
    rendered
}

#[cfg(debug_assertions)]
#[allow(clippy::too_many_arguments)]
fn log_elevenlabs_http_request(
    operation: &str,
    method: &str,
    url: &reqwest::Url,
    proxy_url: Option<&str>,
    timeout: Duration,
    voice_type: Option<&str>,
    page: Option<usize>,
    has_page_token: bool,
) {
    if !elevenlabs_http_logging_enabled() {
        return;
    }
    debug!(
        target: "elevenlabs_http",
        operation,
        method,
        url = %safe_elevenlabs_url_for_log(url),
        headers = "xi-api-key=<redacted>",
        has_proxy = proxy_url.is_some(),
        proxy = %proxy_url.map(secret_log::safe_url_for_log).unwrap_or_else(|| "direct".to_string()),
        timeout_ms = timeout.as_millis(),
        voice_type = voice_type.unwrap_or("-"),
        page = page.unwrap_or(0),
        has_page_token,
        "ElevenLabs HTTP request",
    );
}

#[cfg(not(debug_assertions))]
#[allow(clippy::too_many_arguments)]
fn log_elevenlabs_http_request(
    _operation: &str,
    _method: &str,
    _url: &reqwest::Url,
    _proxy_url: Option<&str>,
    _timeout: Duration,
    _voice_type: Option<&str>,
    _page: Option<usize>,
    _has_page_token: bool,
) {
}

#[cfg(debug_assertions)]
fn log_elevenlabs_http_response(
    operation: &str,
    status: u16,
    elapsed: Duration,
    content_type: Option<&str>,
    content_length: Option<u64>,
    body: Option<&[u8]>,
    redactions: &[&str],
) {
    if !elevenlabs_http_logging_enabled() {
        return;
    }
    let body = body.map(|body| bounded_http_body_for_log(body, redactions));
    debug!(
        target: "elevenlabs_http",
        operation,
        status,
        elapsed_ms = elapsed.as_millis(),
        content_type = content_type.unwrap_or("-"),
        content_length = content_length.unwrap_or(0),
        body = body.as_deref().unwrap_or("<binary omitted>"),
        "ElevenLabs HTTP response",
    );
}

#[cfg(not(debug_assertions))]
fn log_elevenlabs_http_response(
    _operation: &str,
    _status: u16,
    _elapsed: Duration,
    _content_type: Option<&str>,
    _content_length: Option<u64>,
    _body: Option<&[u8]>,
    _redactions: &[&str],
) {
}

#[cfg(debug_assertions)]
fn log_elevenlabs_http_transport_error(operation: &str, elapsed: Duration, error: &reqwest::Error) {
    if !elevenlabs_http_logging_enabled() {
        return;
    }
    debug!(
        target: "elevenlabs_http",
        operation,
        elapsed_ms = elapsed.as_millis(),
        is_timeout = error.is_timeout(),
        is_connect = error.is_connect(),
        status = error.status().map(|status| status.as_u16()).unwrap_or(0),
        url = %error.url().map(safe_elevenlabs_url_for_log).unwrap_or_else(|| "-".to_string()),
        "ElevenLabs HTTP transport error",
    );
}

#[cfg(not(debug_assertions))]
fn log_elevenlabs_http_transport_error(
    _operation: &str,
    _elapsed: Duration,
    _error: &reqwest::Error,
) {
}

/// Displayed classification of an ElevenLabs voice, derived from the
/// `/v2/voices` partition it was fetched from.
///
/// Serialized as the lowercase strings `default` and `library`. The unmarked
/// personal/workspace voice is represented by `None` (null/missing value).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ElevenLabsVoiceClassification {
    /// ElevenLabs built-in default voices (free-compatible).
    Default,
    /// Community / third-party library voices (may require a paid plan).
    Library,
}

/// Provider-specific voice descriptor with a stable `voice_id`.
///
/// Distinct from the Fish Audio marketplace type: ElevenLabs voices are
/// selected by `voice_id` and expose category/labels instead of languages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElevenLabsVoice {
    pub voice_id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub preview_url: Option<String>,
    /// Optional classification (`default` / `library`); `None` for an unmarked
    /// personal/workspace voice and for legacy cached entries.
    #[serde(default)]
    pub classification: Option<ElevenLabsVoiceClassification>,
}

/// Provider-specific model descriptor backed by the account model catalog.
///
/// `model_id` is the stable identifier sent to `POST /text-to-speech`.
/// `can_use_style` and `can_use_speaker_boost` gate the corresponding voice
/// controls so unsupported features are disabled/reset in a predictable way.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ElevenLabsModel {
    pub model_id: String,
    pub name: String,
    #[serde(default)]
    pub can_use_style: bool,
    #[serde(default)]
    pub can_use_speaker_boost: bool,
}

#[derive(Debug, Serialize)]
struct ElevenLabsTtsRequest {
    text: String,
    model_id: String,
    voice_settings: ElevenLabsVoiceSettings,
}

#[derive(Debug, Serialize)]
struct ElevenLabsVoiceSettings {
    stability: f32,
    similarity_boost: f32,
    style: f32,
    use_speaker_boost: bool,
}

/// Raw page of the `GET /v2/voices` response.
#[derive(Debug, Deserialize)]
struct VoicesResponse {
    #[serde(default)]
    voices: Vec<VoiceEntity>,
    #[serde(default)]
    has_more: bool,
    #[serde(default)]
    next_page_token: Option<String>,
}

/// Raw voice entity from `GET /v2/voices`.
#[derive(Debug, Deserialize)]
struct VoiceEntity {
    #[serde(default)]
    voice_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    labels: serde_json::Value,
    #[serde(default)]
    preview_url: Option<String>,
}

impl VoiceEntity {
    fn into_voice(self) -> ElevenLabsVoice {
        ElevenLabsVoice {
            voice_id: self.voice_id,
            name: self.name,
            category: self.category,
            labels: labels_to_vec(&self.labels),
            preview_url: self.preview_url,
            classification: None,
        }
    }
}

/// Raw model entity from `GET /v1/models`.
#[derive(Debug, Deserialize)]
struct ModelEntity {
    #[serde(default)]
    model_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    can_use_style: bool,
    #[serde(default)]
    can_use_speaker_boost: bool,
    #[serde(default)]
    can_do_text_to_speech: bool,
}

impl ModelEntity {
    fn into_model(self) -> ElevenLabsModel {
        ElevenLabsModel {
            model_id: self.model_id,
            name: self.name,
            can_use_style: self.can_use_style,
            can_use_speaker_boost: self.can_use_speaker_boost,
        }
    }
}

/// Reduce raw model entities to usable TTS models: keep only entries whose
/// `can_do_text_to_speech` is true.
fn filter_tts_models(models: Vec<ModelEntity>) -> Vec<ElevenLabsModel> {
    let mut seen = HashSet::new();
    models
        .into_iter()
        .filter(|m| m.can_do_text_to_speech && !m.model_id.trim().is_empty())
        .map(ModelEntity::into_model)
        .filter(|m| seen.insert(m.model_id.clone()))
        .collect()
}

/// Reduce raw model entities to usable TTS models, rejecting an empty set so a
/// malformed/empty response never reads as "no model was ever loaded".
fn usable_tts_models(models: Vec<ModelEntity>) -> Result<Vec<ElevenLabsModel>, String> {
    let models = filter_tts_models(models);
    if models.is_empty() {
        return Err("ElevenLabs returned no usable text-to-speech models.".to_string());
    }
    Ok(models)
}

/// Select the model to persist after a successful catalog refresh: keep the
/// previously selected model when it is still present, otherwise fall back to
/// the first returned model.
pub fn select_model_after_refresh(
    previous_model_id: &str,
    models: &[ElevenLabsModel],
) -> Result<String, String> {
    if models.iter().any(|m| m.model_id == previous_model_id) {
        return Ok(previous_model_id.to_string());
    }
    models
        .first()
        .map(|m| m.model_id.clone())
        .ok_or_else(|| "ElevenLabs returned no usable text-to-speech models.".to_string())
}

/// Select the voice to persist after a successful voice refresh: keep the
/// previously selected voice when it is still present, otherwise fall back to
/// the first returned voice, or an empty string when no voices were returned.
pub fn select_voice_after_refresh(previous_voice_id: &str, voices: &[ElevenLabsVoice]) -> String {
    if !previous_voice_id.is_empty() && voices.iter().any(|v| v.voice_id == previous_voice_id) {
        return previous_voice_id.to_string();
    }
    voices
        .first()
        .map(|v| v.voice_id.clone())
        .unwrap_or_default()
}

/// Convert the API `labels` field (object or array) into a deterministic,
/// deduplicated, sorted list of string values.
fn labels_to_vec(labels: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    match labels {
        serde_json::Value::Object(map) => {
            for value in map.values() {
                if let Some(s) = value.as_str() {
                    out.push(s.to_string());
                }
            }
        }
        serde_json::Value::Array(items) => {
            for value in items {
                if let Some(s) = value.as_str() {
                    out.push(s.to_string());
                }
            }
        }
        _ => {}
    }
    out.sort();
    out.dedup();
    out
}

/// Rank used to order the merged catalog: Default first, unmarked
/// (personal/workspace) second, Library last.
fn classification_rank(classification: Option<ElevenLabsVoiceClassification>) -> u8 {
    match classification {
        Some(ElevenLabsVoiceClassification::Default) => 0,
        None => 1,
        Some(ElevenLabsVoiceClassification::Library) => 2,
    }
}

/// Precedence used to deduplicate a voice present in several partitions:
/// `default` beats `library`, which beats an unmarked (personal/workspace)
/// voice.
fn classification_precedence(classification: Option<ElevenLabsVoiceClassification>) -> u8 {
    match classification {
        Some(ElevenLabsVoiceClassification::Default) => 0,
        Some(ElevenLabsVoiceClassification::Library) => 1,
        None => 2,
    }
}

/// Merge the three `/v2/voices` partitions into one deterministic catalog.
///
/// Deduplicates by `voice_id`; when an ID appears in more than one partition,
/// the classification with the highest precedence wins (`default`, then
/// `library`, then unmarked). The output keeps Default voices first,
/// personal/workspace voices second, and Library voices last, preserving source
/// order within each group.
pub fn merge_voice_partitions(
    default_voices: Vec<ElevenLabsVoice>,
    personal_voices: Vec<ElevenLabsVoice>,
    library_voices: Vec<ElevenLabsVoice>,
) -> Vec<ElevenLabsVoice> {
    let mut selected: std::collections::HashMap<String, ElevenLabsVoice> =
        std::collections::HashMap::new();
    let mut order: Vec<String> = Vec::new();

    let mut insert = |voice: ElevenLabsVoice| {
        let id = voice.voice_id.clone();
        match selected.get_mut(&id) {
            Some(existing) => {
                if classification_precedence(voice.classification)
                    < classification_precedence(existing.classification)
                {
                    *existing = voice;
                }
            }
            None => {
                order.push(id.clone());
                selected.insert(id, voice);
            }
        }
    };

    for voice in default_voices {
        insert(voice);
    }
    for voice in personal_voices {
        insert(voice);
    }
    for voice in library_voices {
        insert(voice);
    }

    let mut groups: [Vec<ElevenLabsVoice>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for id in order {
        if let Some(voice) = selected.remove(&id) {
            groups[classification_rank(voice.classification) as usize].push(voice);
        }
    }

    groups.into_iter().flatten().collect()
}

/// Pure pagination state machine for `GET /v2/voices`.
///
/// Tracks fetched pages, caps the total, and rejects a repeated token so a
/// malicious or malformed server cannot loop the refresh forever.
#[derive(Debug, Clone)]
pub struct VoicePager {
    max_pages: usize,
    pages_fetched: usize,
    visited_tokens: HashSet<String>,
}

impl VoicePager {
    pub fn new(max_pages: usize) -> Self {
        Self {
            max_pages,
            pages_fetched: 0,
            visited_tokens: HashSet::new(),
        }
    }

    /// Feed the result of a finished page and decide the next token.
    ///
    /// Returns `Ok(Some(token))` to fetch another page, `Ok(None)` to stop,
    /// or `Err` on a malformed/repeated token.
    pub fn after_page(
        &mut self,
        has_more: bool,
        next_page_token: Option<String>,
    ) -> Result<Option<String>, String> {
        self.pages_fetched += 1;
        if !has_more {
            return Ok(None);
        }
        if self.pages_fetched >= self.max_pages {
            return Ok(None);
        }
        let token = next_page_token.ok_or_else(|| {
            "ElevenLabs voices pagination: has_more set but no next_page_token".to_string()
        })?;
        if !self.visited_tokens.insert(token.clone()) {
            return Err("ElevenLabs voices pagination: repeated next_page_token".to_string());
        }
        Ok(Some(token))
    }
}

/// Build the `POST /v1/text-to-speech/{voice_id}` URL.
///
/// The voice ID is percent-encoded through reqwest's path-segment API, never
/// via string concatenation.
pub fn tts_url(voice_id: &str, output_format: &str) -> Result<reqwest::Url, String> {
    // Do not end the base path with `/`: `path_segments_mut().push()` keeps
    // that empty trailing segment and would otherwise produce `//{voice_id}`.
    let mut url = reqwest::Url::parse("https://api.elevenlabs.io/v1/text-to-speech")
        .map_err(|e| format!("Invalid ElevenLabs base URL: {}", e))?;
    {
        let mut segments = url
            .path_segments_mut()
            .map_err(|_| "ElevenLabs base URL cannot be a base".to_string())?;
        segments.push(voice_id);
    }
    url.query_pairs_mut()
        .append_pair("output_format", output_format);
    Ok(url)
}

/// Build the `GET /v2/voices` URL with a partition filter and an optional
/// pagination token.
///
/// Both query pairs are appended through reqwest's query-pairs API, never
/// string concatenation, so neither value can break the query syntax, be
/// dropped, or duplicate a pair.
pub fn voices_url(voice_type: &str, next_page_token: Option<&str>) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse("https://api.elevenlabs.io/v2/voices")
        .map_err(|e| format!("Invalid ElevenLabs base URL: {}", e))?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("voice_type", voice_type);
        if let Some(token) = next_page_token {
            pairs.append_pair("next_page_token", token);
        }
    }
    Ok(url)
}

/// Validate output format and numeric voice settings (no network, no
/// text/voice/key/model checks). Shared by the connection form (where the model
/// may not be loaded yet) and by synthesis.
pub fn validate_connection_settings(
    output_format: &str,
    stability: f32,
    similarity_boost: f32,
    style: f32,
) -> Result<(), String> {
    if !ALLOWED_OUTPUT_FORMATS.contains(&output_format) {
        return Err(format!(
            "Unsupported ElevenLabs output format: {}",
            output_format
        ));
    }
    for (name, value) in [
        ("stability", stability),
        ("similarity_boost", similarity_boost),
        ("style", style),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(format!(
                "ElevenLabs {} must be a finite value between 0.0 and 1.0",
                name
            ));
        }
    }
    Ok(())
}

/// Validate model, output format and numeric voice settings for synthesis
/// (no network, no text/voice/key checks).
pub fn validate_generation_settings(
    model_id: &str,
    output_format: &str,
    stability: f32,
    similarity_boost: f32,
    style: f32,
) -> Result<(), String> {
    if model_id.trim().is_empty() {
        return Err("ElevenLabs model is not selected.".to_string());
    }
    validate_connection_settings(output_format, stability, similarity_boost, style)
}

/// Validate all synthesis inputs before any network access.
pub fn validate_synthesis_input(
    text: &str,
    voice_id: &str,
    model_id: &str,
    output_format: &str,
    stability: f32,
    similarity_boost: f32,
    style: f32,
) -> Result<(), String> {
    if text.trim().is_empty() {
        return Err("Text is empty.".to_string());
    }
    if voice_id.trim().is_empty() {
        return Err("ElevenLabs voice is not selected.".to_string());
    }
    validate_generation_settings(model_id, output_format, stability, similarity_boost, style)
}

/// Bounded error classification read from an ElevenLabs error body.
///
/// Reads `detail.type`, `detail.code`, the legacy `detail.status`, the
/// `detail.param` plus the `detail.message` and `detail.request_id` needed for
/// the scoped-key missing-permission classification. The raw `message`,
/// `request_id`, `param` and any raw body are retained only inside this private
/// struct and never logged or surfaced; only tightly validated subsets may
/// surface in a mapped message or in trace metadata.
#[derive(Debug, Default)]
struct ElevenLabsErrorDetail {
    /// `detail.type` — coarse error family (e.g. `rate_limit_error`).
    kind: String,
    /// `detail.code` — specific documented code (e.g. `rate_limit_exceeded`).
    code: String,
    /// Legacy `detail.status` string (kept for backward compatibility).
    status: String,
    /// `detail.param` — offending request parameter (trace metadata only).
    param: String,
    /// `detail.message` — retained only for tight permission extraction.
    message: String,
    /// `detail.request_id` — retained only for tight hex validation.
    request_id: String,
}

impl ElevenLabsErrorDetail {
    /// Parse the bounded body into a classification. Malformed or truncated
    /// JSON yields an empty classification; the caller then falls back to the
    /// HTTP status code alone.
    fn parse(body: &[u8]) -> Self {
        let value: serde_json::Value = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => return Self::default(),
        };
        match value.get("detail") {
            Some(serde_json::Value::Object(map)) => Self {
                kind: lowercase_str_field(map, "type"),
                code: lowercase_str_field(map, "code"),
                status: lowercase_str_field(map, "status"),
                param: str_field(map, "param"),
                message: str_field(map, "message"),
                request_id: str_field(map, "request_id"),
            },
            // Some legacy responses carry `detail` as a plain status string.
            Some(serde_json::Value::String(status)) => Self {
                status: status.to_lowercase(),
                ..Self::default()
            },
            _ => Self::default(),
        }
    }

    /// True when this is a scoped-key permission failure: the legacy
    /// `detail.status` is `missing_permissions` and the current `detail.code`
    /// is `unauthorized`.
    fn is_missing_permissions(&self) -> bool {
        self.status == "missing_permissions" && self.code == "unauthorized"
    }

    /// True when the request is gated behind a paid plan: the current
    /// `detail.code` or the legacy `detail.status` is `paid_plan_required`.
    fn is_paid_plan_required(&self) -> bool {
        self.code == "paid_plan_required" || self.status == "paid_plan_required"
    }

    /// True when any of `type`/`code`/`status` equals any of the needles.
    fn any_eq(&self, needles: &[&str]) -> bool {
        needles
            .iter()
            .any(|needle| self.kind == *needle || self.code == *needle || self.status == *needle)
    }

    /// True when any of `type`/`code`/`status` contains any of the needles.
    fn any_contains(&self, needles: &[&str]) -> bool {
        needles.iter().any(|needle| {
            self.kind.contains(needle) || self.code.contains(needle) || self.status.contains(needle)
        })
    }

    /// True when `code` or the legacy `status` contains the needle. Used for
    /// classifications that must not be widened by the coarse `type` value.
    fn code_or_status_contains(&self, needle: &str) -> bool {
        self.code.contains(needle) || self.status.contains(needle)
    }
}

/// Read a lowercase string field out of a parsed JSON object, defaulting to
/// an empty string when absent or not a string.
fn lowercase_str_field(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    map.get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_lowercase()
}

/// Read a string field verbatim out of a parsed JSON object, defaulting to an
/// empty string when absent or not a string. The value stays bounded because
/// the parsed body never exceeds `MAX_ERROR_BODY_BYTES`.
fn str_field(map: &serde_json::Map<String, serde_json::Value>, key: &str) -> String {
    map.get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_string()
}

/// Map an ElevenLabs non-success response to a safe, readable message.
///
/// Classifies the documented `detail.type` / `detail.code` values plus the
/// legacy `detail.status`. When the body is a scoped-key missing-permission
/// failure, a tightly validated permission identifier may be named, and a
/// validated diagnostic `detail.code` and `detail.request_id` may be appended.
/// Never returns the raw body, `detail.message`, `detail.param`, the API key,
/// the request text or proxy credentials.
///
/// After parsing, structured trace metadata is emitted with only validated
/// classification tokens and a validated request id; the raw body, `message`,
/// `param`, key and proxy details are never logged.
pub fn map_elevenlabs_error(status: u16, body: &[u8]) -> String {
    let detail = ElevenLabsErrorDetail::parse(body);
    trace!(
        http_status = status,
        error_type = loggable_token(&detail.kind),
        error_code = loggable_token(&detail.code),
        legacy_status = loggable_token(&detail.status),
        error_param = loggable_token(&detail.param),
        request_id = loggable_request_id(&detail.request_id),
        "ElevenLabs API error classified"
    );
    let base = classify_elevenlabs_error(status, &detail);
    append_safe_identifiers(base, &detail.code, &detail.request_id)
}

/// Core classification of an ElevenLabs non-success response.
///
/// Never surfaces `detail.message`, `detail.param`, `detail.code` nor
/// `detail.request_id` verbatim; only a tightly validated permission identifier
/// extracted from the message may be named by the missing-permission branch,
/// and validated diagnostic identifiers are appended afterwards by the caller.
fn classify_elevenlabs_error(status: u16, detail: &ElevenLabsErrorDetail) -> String {
    // 1. scoped-key permission failure (before the generic auth branch).
    if detail.is_missing_permissions() {
        return missing_permissions_message(detail);
    }

    // 2. plan-gated request (before the credits, voice-not-found and generic
    //    HTTP-status branches): the current plan does not cover this call.
    if detail.is_paid_plan_required() {
        return "ElevenLabs: your current plan does not allow this request; \
                choose an available voice or feature, or upgrade your plan."
            .to_string();
    }

    // 3. model / feature / plan access (before the generic 403 auth branch).
    if detail.any_eq(&[
        "model_not_found",
        "model_denied",
        "model_not_available",
        "not_available_for_plan",
        "model_access_denied",
        "unsupported_model",
        "feature_not_available",
        "subscription_required",
    ]) || (detail.kind == "authorization_error"
        && detail.any_contains(&["model", "feature", "subscription", "plan"]))
    {
        return "ElevenLabs model is not available for this account or plan.".to_string();
    }

    // 4. voice not found / unavailable. Only an explicit voice classification
    //    is treated as a missing voice; a bodyless or unclassified HTTP 404
    //    falls through to the generic status message below.
    if detail.any_eq(&[
        "voice_not_found",
        "voice_disabled",
        "voice_not_available",
        "voice_access_denied",
        "invalid_voice_id",
    ]) {
        return "ElevenLabs voice not found or no longer available to this account.".to_string();
    }

    // 5. insufficient credits. `payment_required` with `insufficient_credits`
    //    stays a credits failure; `paid_plan_required` was already handled
    //    above as a plan-gating failure.
    if status == 402
        || detail.kind == "payment_required"
        || detail.any_eq(&[
            "insufficient_credits",
            "not_enough_credits",
            "quota_exceeded",
            "insufficient_quota",
        ])
    {
        return "ElevenLabs has insufficient credits for this request.".to_string();
    }

    // 6. concurrency limit (checked before the generic rate-limit branch).
    if detail.code_or_status_contains("concurrent") {
        return "ElevenLabs concurrency limit exceeded; please wait and try again.".to_string();
    }

    // 7. system busy (429 with `system_busy` is not a plain rate limit).
    if detail.any_eq(&["system_busy"]) {
        return "ElevenLabs is currently busy; please try again later.".to_string();
    }

    // 8. service unavailable / maintenance
    if detail.kind == "service_unavailable"
        || detail.any_eq(&["service_unavailable", "maintenance"])
    {
        return "ElevenLabs is temporarily unavailable; please try again later.".to_string();
    }

    // 9. request rate limit
    if status == 429
        || detail.kind == "rate_limit_error"
        || detail.any_contains(&["rate_limit"])
        || detail.any_eq(&["too_many_requests"])
    {
        return "ElevenLabs request rate limit exceeded; please slow down.".to_string();
    }

    // 10. input errors: text length first, then generic invalid input/params.
    if detail.any_contains(&["too_long", "length"]) {
        return "ElevenLabs input text exceeds the model's length limit.".to_string();
    }
    if detail.kind == "validation_error"
        || detail.kind == "invalid_request"
        || detail.any_eq(&[
            "bad_request",
            "malformed_json",
            "invalid_content_type",
            "request_too_large",
            "invalid_parameters",
            "missing_required_field",
            "invalid_text",
            "empty_text",
            "invalid_output_format",
        ])
    {
        return "ElevenLabs rejected the request: invalid input or parameters.".to_string();
    }

    // 11. authentication / permission
    if status == 401
        || status == 403
        || detail.kind == "authentication_error"
        || detail.kind == "authorization_error"
        || detail.any_eq(&[
            "invalid_api_key",
            "missing_api_key",
            "invalid_authorization_header",
            "unauthorized",
            "sign_in_required",
            "invalid_token",
            "authentication_failed",
            "forbidden",
            "insufficient_permissions",
            "workspace_access_denied",
        ])
    {
        return "ElevenLabs authentication failed: the API key is invalid or lacks permission."
            .to_string();
    }

    // 12. temporary service errors
    if (500..=599).contains(&status) {
        return "ElevenLabs is temporarily unavailable; please try again later.".to_string();
    }

    format!("ElevenLabs request failed (HTTP {}).", status)
}

/// Extract a tightly validated permission identifier from the remote
/// `detail.message`.
///
/// A candidate is a run of ASCII lowercase letters, digits and underscores
/// that ends in `_read` or `_write` and starts with a lowercase letter, and is
/// no longer than `MAX_PERMISSION_ID_LEN` bytes. The rest of the remote
/// message is never exposed.
fn extract_missing_permission(message: &str) -> Option<&str> {
    message
        .split(|c: char| !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'))
        .filter(|token| !token.is_empty())
        .filter(|token| token.len() <= MAX_PERMISSION_ID_LEN)
        .find(|token| {
            let well_formed = matches!(
                token
                    .strip_suffix("_read")
                    .or_else(|| token.strip_suffix("_write")),
                Some(stem) if !stem.is_empty()
            );
            well_formed && token.as_bytes()[0].is_ascii_lowercase()
        })
}

/// Build the message for a scoped-key missing-permission failure, naming the
/// missing permission only when a tightly validated identifier was found.
fn missing_permissions_message(detail: &ElevenLabsErrorDetail) -> String {
    match extract_missing_permission(&detail.message) {
        Some(permission) => format!(
            "ElevenLabs API key is missing the required permission {}; \
             grant the permission or use an API key with broader access.",
            permission
        ),
        None => "ElevenLabs API key is missing a required permission; \
             grant the permission or use an API key with broader access."
            .to_string(),
    }
}

/// True when `id` is a non-empty, bounded ASCII hexadecimal identifier.
fn valid_request_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= MAX_REQUEST_ID_LEN && id.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Return `token` for logging when it passes the strict diagnostic-token
/// validation, otherwise a safe placeholder so an invalid or oversized remote
/// value is never emitted verbatim.
fn loggable_token(token: &str) -> &str {
    if valid_api_token(token) {
        token
    } else {
        "-"
    }
}

/// Return `id` for logging when it is a valid request id, otherwise a safe
/// placeholder so a non-hex or oversized remote identifier is never emitted
/// verbatim.
fn loggable_request_id(id: &str) -> &str {
    if valid_request_id(id) {
        id
    } else {
        "-"
    }
}

/// Append validated diagnostic identifiers to a mapped error message.
///
/// A tightly validated `code` (lowercase ASCII letters, digits, underscore or
/// hyphen, bounded) is appended before a tightly validated ASCII hex
/// `request_id`. Arbitrary or oversized values are never surfaced, and the
/// raw `detail.message`/`detail.param` are never appended.
fn append_safe_identifiers(message: String, code: &str, request_id: &str) -> String {
    let code_part = valid_api_token(code).then(|| format!("code {}", code));
    let request_part = valid_request_id(request_id).then(|| format!("request {}", request_id));
    match (code_part, request_part) {
        (Some(code), Some(request)) => format!("{} ({}, {})", message, code, request),
        (Some(code), None) => format!("{} ({})", message, code),
        (None, Some(request)) => format!("{} ({})", message, request),
        (None, None) => message,
    }
}

/// Read an error response body until EOF or the `MAX_ERROR_BODY_BYTES` cap.
///
/// Repeatedly pulls transport chunks (an error body may be split across
/// chunks) but never allocates or retains more than the cap. Any read error
/// returns what was already collected; callers treat a capped/truncated body
/// as a safe empty classification via the JSON parse fallback.
async fn read_bounded_error_body(mut response: reqwest::Response) -> Vec<u8> {
    let mut body = Vec::with_capacity(MAX_ERROR_BODY_BYTES);
    while body.len() < MAX_ERROR_BODY_BYTES {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if chunk.is_empty() {
                    break;
                }
                let room = MAX_ERROR_BODY_BYTES - body.len();
                body.extend_from_slice(&chunk[..chunk.len().min(room)]);
            }
            Ok(None) | Err(_) => break,
        }
    }
    body
}

/// Cached `reqwest::Client` keyed by the settings that determine how it is
/// built. The client is a cheap `Arc` clone, so reusing it preserves the
/// underlying connection pool for both synthesis and catalog pagination.
#[derive(Clone, Debug)]
struct ClientCacheEntry {
    proxy_url: Option<String>,
    timeout_secs: u64,
    client: Client,
}

#[derive(Debug)]
pub struct ElevenLabsTts {
    api_key: String,
    voice_id: String,
    model_id: String,
    output_format: String,
    stability: f32,
    similarity_boost: f32,
    style: f32,
    use_speaker_boost: bool,
    proxy_url: Option<String>,
    timeout_secs: u64,
    event_tx: Option<EventSender>,
    client_cache: std::sync::Mutex<Option<ClientCacheEntry>>,
}

impl Clone for ElevenLabsTts {
    fn clone(&self) -> Self {
        let client_cache = self
            .client_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        Self {
            api_key: self.api_key.clone(),
            voice_id: self.voice_id.clone(),
            model_id: self.model_id.clone(),
            output_format: self.output_format.clone(),
            stability: self.stability,
            similarity_boost: self.similarity_boost,
            style: self.style,
            use_speaker_boost: self.use_speaker_boost,
            proxy_url: self.proxy_url.clone(),
            timeout_secs: self.timeout_secs,
            event_tx: self.event_tx.clone(),
            client_cache: std::sync::Mutex::new(client_cache),
        }
    }
}

impl ElevenLabsTts {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            voice_id: String::new(),
            model_id: String::new(),
            output_format: DEFAULT_OUTPUT_FORMAT.to_string(),
            stability: DEFAULT_STABILITY,
            similarity_boost: DEFAULT_SIMILARITY_BOOST,
            style: DEFAULT_STYLE,
            use_speaker_boost: DEFAULT_USE_SPEAKER_BOOST,
            proxy_url: None,
            timeout_secs: DEFAULT_TTS_TIMEOUT_SECS,
            event_tx: None,
            client_cache: std::sync::Mutex::new(None),
        }
    }

    pub fn with_event_tx(mut self, event_tx: EventSender) -> Self {
        self.event_tx = Some(event_tx);
        self
    }

    pub fn set_voice_id(&mut self, voice_id: String) {
        self.voice_id = voice_id;
    }

    pub fn set_model_id(&mut self, model_id: String) {
        self.model_id = model_id;
    }

    pub fn set_output_format(&mut self, output_format: String) {
        self.output_format = output_format;
    }

    pub fn set_stability(&mut self, stability: f32) {
        self.stability = stability;
    }

    pub fn set_similarity_boost(&mut self, similarity_boost: f32) {
        self.similarity_boost = similarity_boost;
    }

    pub fn set_style(&mut self, style: f32) {
        self.style = style;
    }

    pub fn set_use_speaker_boost(&mut self, use_speaker_boost: bool) {
        self.use_speaker_boost = use_speaker_boost;
    }

    pub fn set_proxy(&mut self, proxy_url: Option<String>) {
        self.proxy_url = proxy_url;
    }

    pub fn voice_id(&self) -> &str {
        &self.voice_id
    }

    /// Return the cached HTTP client, rebuilding it only when the proxy URL or
    /// timeout changed. A build error is never cached so the next call retries.
    fn cached_client(&self) -> Result<Client, String> {
        let key_proxy = self.proxy_url.clone();
        let key_timeout = self.timeout_secs;
        {
            let cache = self
                .client_cache
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some(entry) = cache.as_ref() {
                if entry.proxy_url == key_proxy && entry.timeout_secs == key_timeout {
                    return Ok(entry.client.clone());
                }
            }
        }
        let client = proxy_utils::build_client_with_proxy(
            self.proxy_url.as_deref(),
            Duration::from_secs(self.timeout_secs),
        )?;
        *self
            .client_cache
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(ClientCacheEntry {
            proxy_url: key_proxy,
            timeout_secs: key_timeout,
            client: client.clone(),
        });
        Ok(client)
    }

    async fn fetch_voices_page(
        client: &Client,
        api_key: &str,
        proxy_url: Option<&str>,
        voice_type: &str,
        page: usize,
        token: Option<&str>,
    ) -> Result<VoicesResponse, String> {
        let timeout = Duration::from_secs(30);

        let url = voices_url(voice_type, token)?;

        log_elevenlabs_http_request(
            "voices",
            "GET",
            &url,
            proxy_url,
            timeout,
            Some(voice_type),
            Some(page),
            token.is_some(),
        );

        let request = client.get(url).header("xi-api-key", api_key);
        let started = Instant::now();
        let response = request.send().await.map_err(|e| {
            log_elevenlabs_http_transport_error("voices", started.elapsed(), &e);
            format!("Failed to list ElevenLabs voices: {}", e)
        })?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        if !status.is_success() {
            let body = read_bounded_error_body(response).await;
            log_elevenlabs_http_response(
                "voices",
                status.as_u16(),
                started.elapsed(),
                content_type.as_deref(),
                content_length,
                Some(&body),
                &[api_key, proxy_url.unwrap_or("")],
            );
            return Err(map_elevenlabs_error(status.as_u16(), &body));
        }

        let body = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read ElevenLabs voices response: {}", e))?;
        log_elevenlabs_http_response(
            "voices",
            status.as_u16(),
            started.elapsed(),
            content_type.as_deref(),
            content_length,
            Some(&body),
            &[api_key, proxy_url.unwrap_or("")],
        );
        serde_json::from_slice::<VoicesResponse>(&body)
            .map_err(|e| format!("Failed to parse ElevenLabs voices response: {}", e))
    }

    /// Fetch one `voice_type` partition of the account voice catalog through the
    /// paginated `GET /v2/voices`.
    ///
    /// Follows `has_more` / `next_page_token`, caps total pages, rejects
    /// repeated tokens and tags every voice with the partition's classification.
    async fn fetch_voice_partition(
        client: &Client,
        api_key: &str,
        proxy_url: Option<&str>,
        voice_type: &str,
        classification: Option<ElevenLabsVoiceClassification>,
    ) -> Result<Vec<ElevenLabsVoice>, String> {
        let mut pager = VoicePager::new(MAX_VOICE_PAGES);
        let mut all: Vec<ElevenLabsVoice> = Vec::new();
        let mut token: Option<String> = None;
        let mut page_number = 1;

        loop {
            let page = Self::fetch_voices_page(
                client,
                api_key,
                proxy_url,
                voice_type,
                page_number,
                token.as_deref(),
            )
            .await?;
            for entity in page.voices {
                let mut voice = entity.into_voice();
                voice.classification = classification;
                all.push(voice);
            }
            match pager.after_page(page.has_more, page.next_page_token)? {
                Some(next) => {
                    token = Some(next);
                    page_number += 1;
                }
                None => break,
            }
        }

        Ok(all)
    }

    /// Fetch the account voice catalog through the paginated `GET /v2/voices`.
    ///
    /// Refreshes the three documented partitions — `default` (tagged
    /// `default`), `non-community` (unmarked) and `community` (tagged
    /// `library`) — and merges them deterministically by `voice_id`: Default
    /// voices first, personal/workspace voices second, Library voices last.
    pub async fn list_voices(
        api_key: &str,
        proxy_url: Option<&str>,
    ) -> Result<Vec<ElevenLabsVoice>, String> {
        let client = proxy_utils::build_client_with_proxy(proxy_url, Duration::from_secs(30))?;

        let (default_voices, personal_voices, library_voices) = tokio::join!(
            Self::fetch_voice_partition(
                &client,
                api_key,
                proxy_url,
                "default",
                Some(ElevenLabsVoiceClassification::Default),
            ),
            Self::fetch_voice_partition(&client, api_key, proxy_url, "non-community", None),
            Self::fetch_voice_partition(
                &client,
                api_key,
                proxy_url,
                "community",
                Some(ElevenLabsVoiceClassification::Library),
            ),
        );

        let default_voices = default_voices?;
        let personal_voices = personal_voices?;
        let library_voices = library_voices?;

        Ok(merge_voice_partitions(
            default_voices,
            personal_voices,
            library_voices,
        ))
    }

    /// Fetch the account model catalog through authenticated `GET /v1/models`.
    ///
    /// Only entries whose `can_do_text_to_speech` is true are kept; a
    /// malformed response or an empty usable set is rejected safely, and the
    /// API key or response body are never logged.
    pub async fn list_models(
        api_key: &str,
        proxy_url: Option<&str>,
    ) -> Result<Vec<ElevenLabsModel>, String> {
        let timeout = Duration::from_secs(30);
        let client = proxy_utils::build_client_with_proxy(proxy_url, timeout)?;
        let url = reqwest::Url::parse("https://api.elevenlabs.io/v1/models")
            .map_err(|e| format!("Invalid ElevenLabs models URL: {}", e))?;

        log_elevenlabs_http_request("models", "GET", &url, proxy_url, timeout, None, None, false);

        let started = Instant::now();
        let response = client
            .get(url)
            .header("xi-api-key", api_key)
            .send()
            .await
            .map_err(|e| {
                log_elevenlabs_http_transport_error("models", started.elapsed(), &e);
                format!("Failed to list ElevenLabs models: {}", e)
            })?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        if !status.is_success() {
            let body = read_bounded_error_body(response).await;
            log_elevenlabs_http_response(
                "models",
                status.as_u16(),
                started.elapsed(),
                content_type.as_deref(),
                content_length,
                Some(&body),
                &[api_key, proxy_url.unwrap_or("")],
            );
            return Err(map_elevenlabs_error(status.as_u16(), &body));
        }

        // ElevenLabs returns a top-level JSON array from `GET /v1/models`.
        let body = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read ElevenLabs models response: {}", e))?;
        log_elevenlabs_http_response(
            "models",
            status.as_u16(),
            started.elapsed(),
            content_type.as_deref(),
            content_length,
            Some(&body),
            &[api_key, proxy_url.unwrap_or("")],
        );
        let parsed = serde_json::from_slice::<Vec<ModelEntity>>(&body)
            .map_err(|e| format!("Failed to parse ElevenLabs models response: {}", e))?;

        usable_tts_models(parsed)
    }
}

#[async_trait]
impl TtsEngine for ElevenLabsTts {
    async fn synthesize(&self, text: &str) -> Result<Vec<u8>, String> {
        validate_synthesis_input(
            text,
            &self.voice_id,
            &self.model_id,
            &self.output_format,
            self.stability,
            self.similarity_boost,
            self.style,
        )?;
        if self.api_key.trim().is_empty() {
            return Err("ElevenLabs API key is not configured.".to_string());
        }

        let client = self.cached_client()?;
        let url = tts_url(&self.voice_id, &self.output_format)?;

        let request = ElevenLabsTtsRequest {
            text: text.to_string(),
            model_id: self.model_id.clone(),
            voice_settings: ElevenLabsVoiceSettings {
                stability: self.stability,
                similarity_boost: self.similarity_boost,
                style: self.style,
                use_speaker_boost: self.use_speaker_boost,
            },
        };

        log_elevenlabs_http_request(
            "synthesis",
            "POST",
            &url,
            self.proxy_url.as_deref(),
            Duration::from_secs(self.timeout_secs),
            None,
            None,
            false,
        );
        #[cfg(debug_assertions)]
        if elevenlabs_http_logging_enabled() {
            debug!(
                target: "elevenlabs_http",
                operation = "synthesis",
                voice_id = %self.voice_id,
                model_id = %self.model_id,
                output_format = %self.output_format,
                stability = self.stability,
                similarity_boost = self.similarity_boost,
                style = self.style,
                use_speaker_boost = self.use_speaker_boost,
                text_length = text.len(),
                content_type = "application/json",
                request_body = "<text redacted>",
                "ElevenLabs HTTP request body metadata",
            );
        }

        debug!(
            voice_id = %self.voice_id,
            model_id = %self.model_id,
            text_length = text.len(),
            "ElevenLabs TTS request started"
        );

        let started = Instant::now();
        let response = client
            .post(url)
            .header("xi-api-key", self.api_key.trim())
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                log_elevenlabs_http_transport_error("synthesis", started.elapsed(), &e);
                if e.is_timeout() {
                    format!("ElevenLabs timeout ({}s)", self.timeout_secs)
                } else if e.is_connect() {
                    format!("ElevenLabs connection failed: {}", e)
                } else {
                    format!("Failed to send ElevenLabs TTS request: {}", e)
                }
            })?;

        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let content_length = response.content_length();
        if !status.is_success() {
            error!(
                status_code = status.as_u16(),
                "ElevenLabs TTS request failed"
            );
            let body = read_bounded_error_body(response).await;
            log_elevenlabs_http_response(
                "synthesis",
                status.as_u16(),
                started.elapsed(),
                content_type.as_deref(),
                content_length,
                Some(&body),
                &[&self.api_key, self.proxy_url.as_deref().unwrap_or(""), text],
            );
            return Err(map_elevenlabs_error(status.as_u16(), &body));
        }

        let audio_data = response
            .bytes()
            .await
            .map_err(|e| format!("Failed to read ElevenLabs audio data: {}", e))?
            .to_vec();

        log_elevenlabs_http_response(
            "synthesis",
            status.as_u16(),
            started.elapsed(),
            content_type.as_deref(),
            content_length,
            None,
            &[],
        );

        if audio_data.is_empty() {
            return Err("ElevenLabs returned an empty audio response.".to_string());
        }

        debug!(bytes = audio_data.len(), "ElevenLabs audio received");
        Ok(audio_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn voice(id: &str, name: &str) -> ElevenLabsVoice {
        ElevenLabsVoice {
            voice_id: id.to_string(),
            name: name.to_string(),
            category: None,
            labels: Vec::new(),
            preview_url: None,
            classification: None,
        }
    }

    #[test]
    fn http_diagnostic_flag_requires_exact_one() {
        assert!(should_log_elevenlabs_http(Some("1")));
        for value in [
            None,
            Some(""),
            Some("0"),
            Some("true"),
            Some(" 1"),
            Some("1 "),
        ] {
            assert!(!should_log_elevenlabs_http(value));
        }
    }

    #[test]
    fn http_diagnostic_url_keeps_routing_and_redacts_page_token() {
        let url = voices_url("community", Some("opaque-secret-token")).unwrap();
        let safe = safe_elevenlabs_url_for_log(&url);
        assert!(safe.contains("/v2/voices"));
        assert!(safe.contains("voice_type=community"));
        assert!(safe.contains("next_page_token=%3Credacted%3E"));
        assert!(!safe.contains("opaque-secret-token"));
    }

    #[test]
    fn http_diagnostic_body_is_bounded_and_single_line() {
        let mut body = vec![b'a'; MAX_HTTP_DIAGNOSTIC_BODY_BYTES + 20];
        body[1] = b'\n';
        body[2] = b'\r';
        body[3] = 0;
        let rendered = bounded_http_body_for_log(&body, &[]);
        assert!(!rendered.contains('\n'));
        assert!(!rendered.contains('\r'));
        assert!(rendered.contains("\\n\\r\\0"));
        assert!(rendered.ends_with("...[truncated]"));
        assert!(rendered.len() <= MAX_HTTP_DIAGNOSTIC_BODY_BYTES + 32);
    }

    #[test]
    fn http_diagnostic_body_redacts_supplied_secrets_before_truncation() {
        let body = br#"{"key":"secret-api-key","proxy":"socks5://user:pass@host:1080","text":"private phrase"}"#;
        let rendered = bounded_http_body_for_log(
            body,
            &[
                "secret-api-key",
                "socks5://user:pass@host:1080",
                "private phrase",
            ],
        );
        assert!(!rendered.contains("secret-api-key"));
        assert!(!rendered.contains("user:pass"));
        assert!(!rendered.contains("private phrase"));
        assert_eq!(rendered.matches("<redacted>").count(), 3);
    }

    // ── validation ──

    #[test]
    fn generation_settings_reject_empty_model() {
        let err = validate_generation_settings("", "mp3_44100_128", 0.5, 0.75, 0.0).unwrap_err();
        assert!(err.contains("model"));

        let err = validate_generation_settings("   ", "mp3_44100_128", 0.5, 0.75, 0.0).unwrap_err();
        assert!(err.contains("model"));
    }

    #[test]
    fn generation_settings_accept_any_non_empty_model() {
        assert!(
            validate_generation_settings("eleven_flash_v2_5", "mp3_44100_128", 0.5, 0.75, 0.0)
                .is_ok()
        );
        // Models are account-backed now; any non-empty ID passes the model gate.
        assert!(validate_generation_settings(
            "some_account_model",
            "mp3_44100_128",
            0.5,
            0.75,
            0.0
        )
        .is_ok());
    }

    #[test]
    fn connection_settings_do_not_require_a_model() {
        // The connection form is saved before a catalog exists, so it must
        // accept the other fields without a model.
        assert!(validate_connection_settings("mp3_44100_128", 0.5, 0.75, 0.0).is_ok());
        assert!(validate_connection_settings("wav_44100", 0.5, 0.75, 0.0).is_err());
        assert!(validate_connection_settings("mp3_44100_128", f32::NAN, 0.75, 0.0).is_err());
    }

    #[test]
    fn generation_settings_reject_unknown_output_format() {
        let err = validate_generation_settings("eleven_flash_v2_5", "wav_44100", 0.5, 0.75, 0.0)
            .unwrap_err();
        assert!(err.contains("output format"));
    }

    #[test]
    fn generation_settings_reject_non_finite_or_out_of_range() {
        assert!(validate_generation_settings(
            "eleven_flash_v2_5",
            "mp3_44100_128",
            f32::NAN,
            0.5,
            0.0
        )
        .is_err());
        assert!(validate_generation_settings(
            "eleven_flash_v2_5",
            "mp3_44100_128",
            f32::INFINITY,
            0.5,
            0.0
        )
        .is_err());
        assert!(
            validate_generation_settings("eleven_flash_v2_5", "mp3_44100_128", -0.1, 0.5, 0.0)
                .is_err()
        );
        assert!(
            validate_generation_settings("eleven_flash_v2_5", "mp3_44100_128", 0.5, 1.1, 0.0)
                .is_err()
        );
        assert!(
            validate_generation_settings("eleven_flash_v2_5", "mp3_44100_128", 0.5, 0.75, 2.0)
                .is_err()
        );
    }

    #[test]
    fn generation_settings_accept_valid() {
        assert!(
            validate_generation_settings("eleven_flash_v2_5", "mp3_44100_128", 0.5, 0.75, 0.0)
                .is_ok()
        );
        assert!(validate_generation_settings(
            "eleven_multilingual_v2",
            "mp3_44100_96",
            0.0,
            1.0,
            1.0
        )
        .is_ok());
    }

    #[test]
    fn synthesis_input_rejects_empty_text_and_voice() {
        assert!(validate_synthesis_input(
            "",
            "v1",
            "eleven_flash_v2_5",
            "mp3_44100_128",
            0.5,
            0.75,
            0.0
        )
        .is_err());
        assert!(validate_synthesis_input(
            "   ",
            "v1",
            "eleven_flash_v2_5",
            "mp3_44100_128",
            0.5,
            0.75,
            0.0
        )
        .is_err());
        assert!(validate_synthesis_input(
            "hi",
            "",
            "eleven_flash_v2_5",
            "mp3_44100_128",
            0.5,
            0.75,
            0.0
        )
        .is_err());
        assert!(validate_synthesis_input(
            "hi",
            "v1",
            "eleven_flash_v2_5",
            "mp3_44100_128",
            0.5,
            0.75,
            0.0
        )
        .is_ok());
    }

    // ── request serialization ──

    #[test]
    fn request_serializes_expected_fields() {
        let request = ElevenLabsTtsRequest {
            text: "hello".to_string(),
            model_id: "eleven_flash_v2_5".to_string(),
            voice_settings: ElevenLabsVoiceSettings {
                stability: 0.5,
                similarity_boost: 0.75,
                style: 0.25,
                use_speaker_boost: true,
            },
        };
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["text"], "hello");
        assert_eq!(value["model_id"], "eleven_flash_v2_5");
        assert_eq!(value["voice_settings"]["stability"], 0.5);
        assert_eq!(value["voice_settings"]["similarity_boost"], 0.75);
        assert_eq!(value["voice_settings"]["style"], 0.25);
        assert_eq!(value["voice_settings"]["use_speaker_boost"], true);
    }

    // ── URL encoding ──

    #[test]
    fn tts_url_encodes_voice_id_and_appends_query() {
        let url = tts_url("a b/c", "mp3_44100_128").unwrap();
        let s = url.as_str();
        assert_eq!(
            s,
            "https://api.elevenlabs.io/v1/text-to-speech/a%20b%2Fc?output_format=mp3_44100_128"
        );
        assert!(!s.contains("a b/c"), "raw voice id must be encoded: {s}");
    }

    #[test]
    fn tts_url_plain_voice_id() {
        let url = tts_url("voice123", "mp3_44100_96").unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.elevenlabs.io/v1/text-to-speech/voice123?output_format=mp3_44100_96"
        );
    }

    // ── classification serde ──

    #[test]
    fn classification_serializes_lowercase_and_none_round_trips() {
        assert_eq!(
            serde_json::to_string(&ElevenLabsVoiceClassification::Default).unwrap(),
            "\"default\""
        );
        assert_eq!(
            serde_json::to_string(&ElevenLabsVoiceClassification::Library).unwrap(),
            "\"library\""
        );
        assert_eq!(
            serde_json::from_str::<ElevenLabsVoiceClassification>("\"default\"").unwrap(),
            ElevenLabsVoiceClassification::Default
        );
        assert_eq!(
            serde_json::from_str::<ElevenLabsVoiceClassification>("\"library\"").unwrap(),
            ElevenLabsVoiceClassification::Library
        );
    }

    #[test]
    fn voice_deserializes_without_classification_field() {
        let json =
            r#"{"voice_id":"v1","name":"Voice","category":null,"labels":[],"preview_url":null}"#;
        let v: ElevenLabsVoice = serde_json::from_str(json).unwrap();
        assert!(v.classification.is_none());
    }

    // ── merge_voice_partitions ──

    fn classified_voice(
        id: &str,
        classification: Option<ElevenLabsVoiceClassification>,
    ) -> ElevenLabsVoice {
        let mut v = voice(id, id);
        v.classification = classification;
        v
    }

    #[test]
    fn merge_orders_default_personal_library() {
        let default = vec![
            classified_voice("d1", Some(ElevenLabsVoiceClassification::Default)),
            classified_voice("d2", Some(ElevenLabsVoiceClassification::Default)),
        ];
        let personal = vec![classified_voice("p1", None), classified_voice("p2", None)];
        let library = vec![
            classified_voice("l1", Some(ElevenLabsVoiceClassification::Library)),
            classified_voice("l2", Some(ElevenLabsVoiceClassification::Library)),
        ];

        let merged = merge_voice_partitions(default, personal, library);
        let ids: Vec<&str> = merged.iter().map(|v| v.voice_id.as_str()).collect();
        assert_eq!(ids, vec!["d1", "d2", "p1", "p2", "l1", "l2"]);
    }

    #[test]
    fn merge_deduplicates_by_voice_id() {
        let default = vec![
            classified_voice("a", Some(ElevenLabsVoiceClassification::Default)),
            classified_voice("b", Some(ElevenLabsVoiceClassification::Default)),
        ];
        let personal = vec![classified_voice("a", None)];
        let library = vec![classified_voice(
            "b",
            Some(ElevenLabsVoiceClassification::Library),
        )];

        let merged = merge_voice_partitions(default, personal, library);
        let ids: Vec<&str> = merged.iter().map(|v| v.voice_id.as_str()).collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn merge_prefers_default_then_library_then_unmarked() {
        // The same id appears in several partitions: default wins.
        let default = vec![classified_voice(
            "x",
            Some(ElevenLabsVoiceClassification::Default),
        )];
        let personal = vec![classified_voice("x", None)];
        let library = vec![classified_voice(
            "x",
            Some(ElevenLabsVoiceClassification::Library),
        )];
        let merged = merge_voice_partitions(default, personal, library);
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].classification,
            Some(ElevenLabsVoiceClassification::Default)
        );

        // library beats unmarked.
        let merged = merge_voice_partitions(
            Vec::new(),
            vec![classified_voice("y", None)],
            vec![classified_voice(
                "y",
                Some(ElevenLabsVoiceClassification::Library),
            )],
        );
        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].classification,
            Some(ElevenLabsVoiceClassification::Library)
        );
    }

    #[test]
    fn merge_preserves_source_order_within_each_partition() {
        let default = vec![
            classified_voice("d2", Some(ElevenLabsVoiceClassification::Default)),
            classified_voice("d1", Some(ElevenLabsVoiceClassification::Default)),
        ];
        let personal = vec![classified_voice("p2", None), classified_voice("p1", None)];
        let library = vec![
            classified_voice("l2", Some(ElevenLabsVoiceClassification::Library)),
            classified_voice("l1", Some(ElevenLabsVoiceClassification::Library)),
        ];

        let merged = merge_voice_partitions(default, personal, library);
        let ids: Vec<&str> = merged.iter().map(|v| v.voice_id.as_str()).collect();
        assert_eq!(ids, vec!["d2", "d1", "p2", "p1", "l2", "l1"]);
    }

    #[test]
    fn merge_empty_partitions_is_empty() {
        assert!(merge_voice_partitions(Vec::new(), Vec::new(), Vec::new()).is_empty());
    }

    // ── voices_url ──

    #[test]
    fn voices_url_includes_voice_type_and_pagination_token() {
        let url = voices_url("community", Some("tok+123/45")).unwrap();
        let s = url.as_str();
        assert_eq!(
            s,
            "https://api.elevenlabs.io/v2/voices?voice_type=community&next_page_token=tok%2B123%2F45"
        );
        // Count the query pairs: voice_type and next_page_token exactly once each.
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        assert_eq!(
            pairs,
            vec![
                ("voice_type".to_string(), "community".to_string()),
                ("next_page_token".to_string(), "tok+123/45".to_string()),
            ]
        );
    }

    #[test]
    fn voices_url_without_token_has_only_voice_type() {
        let url = voices_url("default", None).unwrap();
        assert_eq!(
            url.as_str(),
            "https://api.elevenlabs.io/v2/voices?voice_type=default"
        );
        assert_eq!(url.query_pairs().count(), 1);
    }

    // ── labels_to_vec ──

    #[test]
    fn labels_from_object_and_array_are_sorted_and_deduped() {
        let obj =
            serde_json::json!({ "accent": "american", "gender": "female", "dup": "american" });
        assert_eq!(
            labels_to_vec(&obj),
            vec!["american".to_string(), "female".to_string()]
        );

        let arr = serde_json::json!(["b", "a", "b"]);
        assert_eq!(labels_to_vec(&arr), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn labels_empty_for_other_shapes() {
        assert!(labels_to_vec(&serde_json::json!(null)).is_empty());
        assert!(labels_to_vec(&serde_json::json!("just-a-string")).is_empty());
    }

    // ── models parsing / filtering ──

    fn models_json(models: serde_json::Value) -> Vec<ModelEntity> {
        serde_json::from_value(models).unwrap()
    }

    #[test]
    fn models_filter_keeps_only_text_to_speech() {
        let resp = models_json(serde_json::json!([
                { "model_id": "eleven_flash_v2_5", "name": "Flash", "can_use_style": true, "can_use_speaker_boost": true, "can_do_text_to_speech": true },
                { "model_id": "eleven_multilingual_v2", "name": "Multilingual", "can_use_style": true, "can_use_speaker_boost": true, "can_do_text_to_speech": true },
                { "model_id": "eleven_sts_v2", "name": "STS", "can_use_style": true, "can_use_speaker_boost": false, "can_do_text_to_speech": false }
        ]));

        let models = usable_tts_models(resp).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].model_id, "eleven_flash_v2_5");
        assert!(models[0].can_use_style);
        assert!(models[0].can_use_speaker_boost);
        assert_eq!(models[1].model_id, "eleven_multilingual_v2");
    }

    #[test]
    fn models_filter_empty_and_all_non_tts_are_rejected() {
        assert!(usable_tts_models(vec![]).is_err());

        let resp = models_json(serde_json::json!([
                { "model_id": "eleven_sts_v2", "name": "STS", "can_do_text_to_speech": false }
        ]));
        let err = usable_tts_models(resp).unwrap_err();
        assert!(err.contains("no usable"));
    }

    #[test]
    fn models_response_defaults_missing_fields_safely() {
        let resp = models_json(serde_json::json!([
                { "model_id": "eleven_flash_v2_5", "can_do_text_to_speech": true }
        ]));
        let models = usable_tts_models(resp).unwrap();
        assert_eq!(models.len(), 1);
        assert!(models[0].name.is_empty());
        assert!(!models[0].can_use_style);
        assert!(!models[0].can_use_speaker_boost);
    }

    #[test]
    fn models_response_rejects_malformed_body() {
        assert!(serde_json::from_slice::<Vec<ModelEntity>>(b"not json at all").is_err());
        assert!(serde_json::from_slice::<Vec<ModelEntity>>(b"{\"models\": []}").is_err());
    }

    // ── model selection after refresh ──

    fn model(id: &str) -> ElevenLabsModel {
        ElevenLabsModel {
            model_id: id.to_string(),
            name: id.to_string(),
            can_use_style: true,
            can_use_speaker_boost: true,
        }
    }

    #[test]
    fn select_model_after_refresh_preserves_still_present_selection() {
        let models = vec![model("a"), model("b")];
        assert_eq!(select_model_after_refresh("b", &models).unwrap(), "b");
    }

    #[test]
    fn select_model_after_refresh_falls_back_to_first() {
        let models = vec![model("a"), model("b")];
        assert_eq!(select_model_after_refresh("gone", &models).unwrap(), "a");
    }

    #[test]
    fn select_model_after_refresh_rejects_empty_catalog() {
        assert!(select_model_after_refresh("anything", &[]).is_err());
    }

    // ── voice selection after refresh ──

    #[test]
    fn select_voice_after_refresh_preserves_still_present_selection() {
        let voices = vec![voice("a", "A"), voice("b", "B")];
        assert_eq!(select_voice_after_refresh("b", &voices), "b");
    }

    #[test]
    fn select_voice_after_refresh_falls_back_to_first() {
        let voices = vec![voice("a", "A"), voice("b", "B")];
        assert_eq!(select_voice_after_refresh("gone", &voices), "a");
        assert_eq!(select_voice_after_refresh("", &voices), "a");
    }

    #[test]
    fn select_voice_after_refresh_returns_empty_for_empty_catalog() {
        assert_eq!(select_voice_after_refresh("anything", &[]), "");
        assert_eq!(select_voice_after_refresh("", &[]), "");
    }

    // ── VoicePager ──

    #[test]
    fn pager_stops_on_no_more() {
        let mut pager = VoicePager::new(50);
        assert_eq!(pager.after_page(false, None).unwrap(), None);
    }

    #[test]
    fn pager_caps_at_max_pages() {
        let mut pager = VoicePager::new(2);
        assert_eq!(
            pager.after_page(true, Some("t1".into())).unwrap(),
            Some("t1".into())
        );
        // second page: has_more true but cap reached -> stop
        assert_eq!(pager.after_page(true, Some("t2".into())).unwrap(), None);
    }

    #[test]
    fn pager_errors_on_repeated_token() {
        let mut pager = VoicePager::new(5);
        assert_eq!(
            pager.after_page(true, Some("t1".into())).unwrap(),
            Some("t1".into())
        );
        assert!(pager.after_page(true, Some("t1".into())).is_err());
    }

    #[test]
    fn pager_errors_on_missing_token_when_has_more() {
        let mut pager = VoicePager::new(5);
        assert!(pager.after_page(true, None).is_err());
    }

    // ── error mapping ──

    /// Legacy error shape: `detail.status` plus a long `detail.message` that
    /// must never leak into a mapped message.
    fn err_json(status: &str) -> Vec<u8> {
        serde_json::json!({ "detail": { "status": status, "message": "x".repeat(10000) } })
            .to_string()
            .into_bytes()
    }

    /// Current ElevenLabs error shape: `detail.{type,code,message,request_id}`.
    fn err_json_current(type_: &str, code: &str) -> Vec<u8> {
        serde_json::json!({
            "detail": {
                "type": type_,
                "code": code,
                "message": "x".repeat(10000),
                "request_id": "req_secret_id"
            }
        })
        .to_string()
        .into_bytes()
    }

    /// Current schema body with a custom message and request id.
    fn err_json_current_full(type_: &str, code: &str, message: &str, request_id: &str) -> Vec<u8> {
        serde_json::json!({
            "detail": {
                "type": type_,
                "code": code,
                "message": message,
                "request_id": request_id,
            }
        })
        .to_string()
        .into_bytes()
    }

    /// Scoped-key permission failure body: legacy `detail.status` is
    /// `missing_permissions`, current `detail.code` is `unauthorized`.
    fn err_json_missing_permissions(message: &str, request_id: &str) -> Vec<u8> {
        serde_json::json!({
            "detail": {
                "status": "missing_permissions",
                "code": "unauthorized",
                "type": "authorization_error",
                "message": message,
                "request_id": request_id,
            }
        })
        .to_string()
        .into_bytes()
    }

    /// The exact `detail.message` ElevenLabs returns for a missing permission.
    const VERBOSE_PERMISSION_MSG: &str =
        "The API key you used is missing the permission voices_read to execute this operation.";

    /// Assert a mapped message keeps to the safe vocabulary (never leaks the
    /// long `message`, the `request_id` or any raw JSON).
    fn assert_safe(msg: &str) {
        assert!(
            !msg.contains("xxxx"),
            "mapped message must not leak detail.message: {msg}"
        );
        assert!(
            !msg.contains("request_id") && !msg.contains("req_secret_id"),
            "mapped message must not leak request_id: {msg}"
        );
        assert!(
            !msg.contains("detail") && !msg.contains('{'),
            "mapped message must not leak raw JSON: {msg}"
        );
    }

    #[test]
    fn error_maps_auth_by_status_and_detail() {
        assert!(map_elevenlabs_error(401, b"{}").contains("authentication"));
        assert!(map_elevenlabs_error(403, b"{}").contains("authentication"));
        let body = err_json("invalid_api_key");
        assert!(map_elevenlabs_error(401, &body).contains("authentication"));
        // long message never leaks
        assert!(!map_elevenlabs_error(401, &body).contains('x'));
    }

    #[test]
    fn error_maps_voice_not_found() {
        // A bodyless/unclassified HTTP 404 is NOT a missing voice; it falls
        // back to the generic status message.
        let bare = map_elevenlabs_error(404, b"");
        assert_eq!(bare, "ElevenLabs request failed (HTTP 404).");
        assert!(
            !bare.contains("voice"),
            "bare 404 must not claim a voice issue: {bare}"
        );

        // An explicit voice-not-found classification still maps to the voice
        // branch even on a 404.
        let body = err_json("voice_not_found");
        assert!(map_elevenlabs_error(404, &body).contains("voice not found"));
    }

    #[test]
    fn error_maps_model_denied() {
        let body = err_json("model_not_available");
        assert!(map_elevenlabs_error(400, &body).contains("model"));
    }

    #[test]
    fn error_maps_insufficient_credits() {
        let body = err_json("not_enough_credits");
        assert!(map_elevenlabs_error(400, &body).contains("credits"));
        assert!(map_elevenlabs_error(402, b"{}").contains("credits"));
    }

    #[test]
    fn error_maps_concurrency_and_rate() {
        assert!(
            map_elevenlabs_error(429, &err_json("too_many_concurrent_requests"))
                .contains("concurrency")
        );
        assert!(map_elevenlabs_error(429, &err_json("too_many_requests")).contains("rate limit"));
        assert!(map_elevenlabs_error(429, b"{}").contains("rate limit"));
    }

    #[test]
    fn error_maps_input_length() {
        assert!(map_elevenlabs_error(422, &err_json("text_too_long")).contains("length"));
    }

    // ── current {detail:{type,code,message,request_id}} schema ──

    #[test]
    fn current_schema_maps_authentication() {
        let msg = map_elevenlabs_error(
            401,
            &err_json_current("authentication_error", "invalid_api_key"),
        );
        assert!(msg.contains("authentication"));
        assert_safe(&msg);
    }

    #[test]
    fn current_schema_maps_authorization_model_access() {
        let msg = map_elevenlabs_error(
            403,
            &err_json_current("authorization_error", "model_access_denied"),
        );
        assert!(msg.contains("model"));
        assert_safe(&msg);

        let feature = map_elevenlabs_error(
            403,
            &err_json_current("authorization_error", "feature_not_available"),
        );
        assert!(feature.contains("model"));
    }

    #[test]
    fn current_schema_maps_insufficient_credits() {
        let msg = map_elevenlabs_error(
            402,
            &err_json_current("payment_required", "insufficient_credits"),
        );
        assert!(msg.contains("credits"));
        assert_safe(&msg);
    }

    #[test]
    fn current_schema_paid_plan_required_maps_to_plan_message_with_identifiers() {
        // `payment_required` + `paid_plan_required` is a plan-gating failure,
        // distinct from a plain insufficient-credits failure.
        let request_id = "0123456789abcdef0123456789abcdef";
        let body = serde_json::json!({
            "detail": {
                "type": "payment_required",
                "code": "paid_plan_required",
                "status": serde_json::Value::Null,
                "message": "Upgrade your plan to access this voice.",
                "request_id": request_id,
            }
        })
        .to_string()
        .into_bytes();
        let msg = map_elevenlabs_error(402, &body);
        assert!(msg.contains("plan"), "plan message expected: {msg}");
        assert!(
            msg.contains("upgrade your plan"),
            "actionable upgrade hint expected: {msg}"
        );
        assert!(
            msg.contains("choose an available voice or feature"),
            "actionable choose-hint expected: {msg}"
        );
        assert!(
            msg.contains("paid_plan_required"),
            "safe diagnostic code must be surfaced: {msg}"
        );
        assert!(
            msg.contains(request_id),
            "valid request id must be surfaced: {msg}"
        );
        assert!(
            !msg.contains("Upgrade your plan to access this voice"),
            "raw detail.message leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn current_schema_paid_plan_required_legacy_status_is_also_recognized() {
        // Legacy `detail.status` alone (without `detail.code`) still maps to
        // the plan message and never to a voice/credits branch.
        let body = err_json("paid_plan_required");
        let msg = map_elevenlabs_error(404, &body);
        assert!(
            msg.contains("upgrade your plan"),
            "legacy plan-gating must be actionable: {msg}"
        );
        assert!(
            !msg.contains("voice not found"),
            "must not read as a missing voice: {msg}"
        );
        assert!(
            !msg.contains("insufficient credits"),
            "must not read as credits: {msg}"
        );
    }

    #[test]
    fn current_schema_payment_required_insufficient_credits_stays_credits() {
        // `paid_plan_required` is distinct: `insufficient_credits` keeps the
        // credits classification even under `payment_required`.
        let msg = map_elevenlabs_error(
            402,
            &err_json_current_full(
                "payment_required",
                "insufficient_credits",
                "Out of credits.",
                "deadbeefcafef00d",
            ),
        );
        assert!(msg.contains("credits"), "credits message expected: {msg}");
        assert!(
            !msg.contains("upgrade your plan"),
            "plan message leaked into a credits failure: {msg}"
        );
        assert!(
            msg.contains("insufficient_credits"),
            "safe diagnostic code must be surfaced: {msg}"
        );
        assert!(
            !msg.contains("Out of credits"),
            "detail.message leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn unclassified_404_is_not_a_missing_voice() {
        let msg = map_elevenlabs_error(404, b"{}");
        assert_eq!(msg, "ElevenLabs request failed (HTTP 404).");
        assert!(
            !msg.contains("voice"),
            "unclassified 404 read as a voice issue: {msg}"
        );
    }

    #[test]
    fn unsafe_or_oversized_codes_and_params_are_never_surfaced() {
        // Strict token validation used by both surfacing and logging helpers.
        assert!(valid_api_token("paid_plan_required"));
        assert!(valid_api_token("rate-limit"));
        assert!(!valid_api_token(""));
        assert!(!valid_api_token("Bad;Code"));
        assert!(!valid_api_token("has space"));
        assert!(!valid_api_token("UpperCase"));
        let oversized = "a".repeat(MAX_DIAGNOSTIC_TOKEN_LEN + 1);
        assert!(!valid_api_token(&oversized));
        assert!(valid_api_token(
            "a".repeat(MAX_DIAGNOSTIC_TOKEN_LEN).as_str()
        ));

        // A malicious code is classified safely but never echoed verbatim.
        let malicious = "x\"<script>alert(1)</script>";
        let msg = map_elevenlabs_error(
            429,
            &err_json_current_full("rate_limit_error", malicious, "nope", "deadbeef"),
        );
        assert!(msg.contains("rate limit"), "classification lost: {msg}");
        assert!(!msg.contains("<script>"), "malicious code surfaced: {msg}");
        assert!(!msg.contains("alert(1)"), "malicious code surfaced: {msg}");
        assert_safe(&msg);

        // An oversized but well-formed code is dropped, never surfaced.
        let msg = map_elevenlabs_error(
            429,
            &err_json_current_full("rate_limit_error", &oversized, "nope", ""),
        );
        assert!(!msg.contains(&oversized), "oversized code surfaced: {msg}");
        assert!(!msg.contains("code "), "invalid code still appended: {msg}");

        // `detail.param` is parsed for trace metadata only; unsafe values are
        // rewritten by the logging helper and never surface in a message.
        let with_param = serde_json::json!({
            "detail": {
                "type": "validation_error",
                "code": "invalid_parameters",
                "param": "text\"<script>alert(1)</script>",
                "message": "Bad parameter.",
                "request_id": "cafef00d",
            }
        })
        .to_string()
        .into_bytes();
        let detail = ElevenLabsErrorDetail::parse(&with_param);
        assert_eq!(detail.param, "text\"<script>alert(1)</script>");
        assert_eq!(loggable_token(&detail.param), "-");
        assert_eq!(loggable_token("voice_id"), "voice_id");

        let msg = map_elevenlabs_error(422, &with_param);
        assert!(msg.contains("invalid input"), "classification lost: {msg}");
        assert!(!msg.contains("<script>"), "unsafe param surfaced: {msg}");
        assert!(
            !msg.contains("Bad parameter"),
            "detail.message leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn current_schema_maps_rate_and_concurrency() {
        let rate = map_elevenlabs_error(
            429,
            &err_json_current("rate_limit_error", "rate_limit_exceeded"),
        );
        assert!(rate.contains("rate limit"));
        assert_safe(&rate);

        let concurrent = map_elevenlabs_error(
            429,
            &err_json_current("rate_limit_error", "concurrent_limit_exceeded"),
        );
        assert!(concurrent.contains("concurrency"));
        assert_safe(&concurrent);
    }

    #[test]
    fn current_schema_maps_system_busy_and_service_unavailable() {
        let busy = map_elevenlabs_error(429, &err_json_current("rate_limit_error", "system_busy"));
        assert!(busy.contains("busy"));
        assert_safe(&busy);

        let unavailable = map_elevenlabs_error(
            503,
            &err_json_current("service_unavailable", "service_unavailable"),
        );
        assert!(unavailable.contains("temporarily unavailable"));
        assert_safe(&unavailable);

        let maintenance =
            map_elevenlabs_error(503, &err_json_current("service_unavailable", "maintenance"));
        assert!(maintenance.contains("temporarily unavailable"));
    }

    #[test]
    fn current_schema_maps_input_errors() {
        let too_long =
            map_elevenlabs_error(400, &err_json_current("validation_error", "text_too_long"));
        assert!(too_long.contains("length"));
        assert_safe(&too_long);

        let bad_params = map_elevenlabs_error(
            400,
            &err_json_current("validation_error", "invalid_parameters"),
        );
        assert!(bad_params.contains("invalid input"));
        assert_safe(&bad_params);

        let malformed =
            map_elevenlabs_error(400, &err_json_current("invalid_request", "malformed_json"));
        assert!(malformed.contains("invalid input"));
    }

    #[test]
    fn current_schema_unknown_values_stay_safe() {
        // Unknown code under a documented authentication type.
        let msg = map_elevenlabs_error(
            401,
            &err_json_current("authentication_error", "some_unknown"),
        );
        assert!(msg.contains("authentication"));
        assert_safe(&msg);

        // Unknown code under a validation type maps to a generic input error.
        let unknown = map_elevenlabs_error(
            400,
            &err_json_current("validation_error", "unexpected_code"),
        );
        assert!(unknown.contains("invalid input"));
        assert_safe(&unknown);
    }

    // ── missing-permission (scoped API key) schema ──

    #[test]
    fn missing_permission_names_voices_read_and_valid_request_id() {
        let request_id = "0123456789abcdef0123456789abcdef";
        let msg = map_elevenlabs_error(
            401,
            &err_json_missing_permissions(VERBOSE_PERMISSION_MSG, request_id),
        );
        assert!(
            msg.contains("missing the required permission voices_read"),
            "must name the missing permission: {msg}"
        );
        assert!(
            msg.contains(request_id),
            "valid request id must be surfaced: {msg}"
        );
        assert!(
            !msg.contains("to execute this operation"),
            "raw detail.message leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn missing_permission_without_extractable_identifier_stays_generic() {
        // The response clearly says a permission is missing, but no identifier
        // can be validated out of the message, so a safe generic message is
        // returned instead of the raw remote text.
        let msg = map_elevenlabs_error(
            401,
            &err_json_missing_permissions("Your API key cannot access this resource.", ""),
        );
        assert!(
            msg.contains("missing a required permission"),
            "generic missing-permission message expected: {msg}"
        );
        assert!(
            !msg.contains("cannot access this resource"),
            "raw detail.message leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn valid_request_id_is_appended_to_classified_error() {
        let request_id = "deadbeefcafef00d";
        let msg = map_elevenlabs_error(
            429,
            &err_json_current_full(
                "rate_limit_error",
                "rate_limit_exceeded",
                "Slow down.",
                request_id,
            ),
        );
        assert!(msg.contains("rate limit"), "classification lost: {msg}");
        assert!(
            msg.contains(request_id),
            "valid request id must be surfaced: {msg}"
        );
        assert!(
            !msg.contains("Slow down"),
            "raw detail.message leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn missing_permission_rejects_malicious_message_and_request_id() {
        // Only the tightly validated permission is surfaced; secret-looking
        // payloads, markup and the malicious request id must never leak.
        let nasty_message = "Missing voices_read; api_key=sk_0123456789abcdef0123456789abcdef<script>alert(1)</script>";
        let body =
            err_json_missing_permissions(nasty_message, "sk_0123456789abcdef; DROP TABLE models");
        let msg = map_elevenlabs_error(401, &body);
        assert!(
            msg.contains("missing the required permission voices_read"),
            "must name the missing permission: {msg}"
        );
        assert!(
            !msg.contains("sk_0123456789abcdef"),
            "secret-looking payload leaked: {msg}"
        );
        assert!(!msg.contains("<script>"), "markup leaked: {msg}");
        assert!(
            !msg.contains("DROP TABLE"),
            "malicious request id leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn missing_permission_rejects_uppercase_and_oversized_request_id() {
        // Uppercase identifiers do not match the strict lowercase shape, so the
        // message stays generic and never echoes the raw identifier.
        let upper = map_elevenlabs_error(
            401,
            &err_json_missing_permissions("The API key is missing the permission Voices_Read.", ""),
        );
        assert!(
            upper.contains("missing a required permission"),
            "generic missing-permission message expected: {upper}"
        );
        assert!(
            !upper.contains("Voices_Read"),
            "raw identifier leaked: {upper}"
        );
        assert_safe(&upper);

        // An oversized hex request id is dropped, not surfaced.
        let long_request_id = "a".repeat(MAX_REQUEST_ID_LEN + 1);
        let body = err_json_missing_permissions(
            "The API key is missing the permission voices_read.",
            &long_request_id,
        );
        let msg = map_elevenlabs_error(401, &body);
        assert!(msg.contains("missing the required permission voices_read"));
        assert!(
            !msg.contains(&long_request_id),
            "oversized request id leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn missing_permission_rejects_oversized_identifier() {
        // A lowercase, otherwise-valid `*_read` token longer than the surfacing
        // cap is dropped so a near-body-cap remote token is never echoed into
        // the UI; the generic missing-permission message is returned instead.
        let oversized = format!("{}_read", "a".repeat(MAX_PERMISSION_ID_LEN));
        let body = err_json_missing_permissions(
            &format!("The API key is missing the permission {}.", oversized),
            "",
        );
        let msg = map_elevenlabs_error(401, &body);
        assert!(
            msg.contains("missing a required permission"),
            "generic missing-permission message expected: {msg}"
        );
        assert!(
            !msg.contains(&oversized),
            "oversized permission identifier leaked: {msg}"
        );
        assert_safe(&msg);
    }

    #[test]
    fn non_missing_permission_auth_responses_stay_generic() {
        // `missing_permissions` alone (without the `unauthorized` code) keeps
        // the existing generic authentication classification.
        let status_only = serde_json::json!({
            "detail": {
                "status": "missing_permissions",
                "message": "The API key is missing user_read to execute this operation."
            }
        })
        .to_string()
        .into_bytes();
        let msg = map_elevenlabs_error(401, &status_only);
        assert!(msg.contains("authentication"), "msg: {msg}");
        assert!(
            !msg.contains("user_read"),
            "raw detail.message leaked: {msg}"
        );

        // Invalid-key legacy shape remains generic.
        let invalid_key = err_json("invalid_api_key");
        let msg = map_elevenlabs_error(401, &invalid_key);
        assert!(msg.contains("authentication"));
        assert!(!msg.contains('x'));
    }

    #[test]
    fn legacy_plain_string_detail_and_truncated_body_stay_safe() {
        // `detail` as a plain legacy status string is still recognized.
        let plain = serde_json::json!({ "detail": "quota_exceeded" })
            .to_string()
            .into_bytes();
        assert!(map_elevenlabs_error(400, &plain).contains("credits"));

        // A truncated JSON body (as produced when the 4096 cap cuts mid-object)
        // falls back to the HTTP-status classification without leaking JSON.
        let truncated = br#"{ "detail": { "type": "rate_limit_error", "code": "rate_limit"#;
        let msg = map_elevenlabs_error(429, truncated);
        assert!(msg.contains("rate limit"));
        assert!(!msg.contains("detail") && !msg.contains('{'));
    }

    #[test]
    fn error_maps_temporary_service_errors() {
        for code in [500, 502, 503, 504] {
            assert!(map_elevenlabs_error(code, b"{}").contains("temporarily unavailable"));
        }
    }

    #[test]
    fn error_falls_back_to_status_only_without_body() {
        let msg = map_elevenlabs_error(400, b"not json at all");
        assert!(msg.contains("HTTP 400"));
        assert!(!msg.contains("not json"));
    }
}
