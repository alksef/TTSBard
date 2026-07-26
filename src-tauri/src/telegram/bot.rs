use super::client::TelegramClient;
use super::types::{CurrentVoice, Limits, TtsResult};
use grammers_session::updates::UpdatesLike;
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, trace, warn};

/// Имя бота Silero TTS в Telegram
const BOT_USERNAME: &str = "silero_voice_bot";

/// Структура для результата голосового сообщения
#[derive(Debug, Clone)]
struct VoiceMessageResult {
    file_id: String,
    msg_id: i32,
    mime_type: String,
}

/// Структура для работы с ботом Silero TTS
pub struct SileroTtsBot {
    _client: Option<TelegramClient>,
}

impl SileroTtsBot {
    pub fn new() -> Self {
        Self { _client: None }
    }

    /// Синтез речи через Telegram бота
    /// Возвращает путь к скачанному аудиофайлу
    pub async fn synthesize(client: &TelegramClient, text: &str) -> Result<TtsResult, String> {
        info!("Starting TTS synthesis");

        let text = text.trim();
        if text.is_empty() {
            return Ok(TtsResult::error("Text cannot be empty".to_string()));
        }

        if text.len() > 4000 {
            return Ok(TtsResult::error(
                "Text too long (max 4000 characters)".to_string(),
            ));
        }

        let mut rx = client.subscribe_updates().await?;

        let (sent_msg_id, bot_user_id) = Self::send_text_to_bot(client, text).await?;

        let voice_result =
            Self::wait_for_voice_message(&mut rx, 30, sent_msg_id, bot_user_id).await?;

        // 3. Скачиваем аудиофайл во временную папку
        let audio_path = Self::download_voice_to_temp(client, &voice_result).await?;

        info!(?audio_path, "TTS synthesis completed");

        Ok(TtsResult::success(audio_path))
    }

    /// Отправить текст боту
    /// Возвращает (message_id, bot_user_id)
    async fn send_text_to_bot(client: &TelegramClient, text: &str) -> Result<(i32, i64), String> {
        info!("Sending text to bot");

        let client_inner = {
            let guard = client.client.lock().await;
            guard
                .clone()
                .ok_or_else(|| "Client not initialized".to_string())?
        };

        let bot = client_inner
            .resolve_username(BOT_USERNAME)
            .await
            .map_err(|e| format!("Failed to resolve bot: {}", e))?
            .ok_or_else(|| "Bot not found".to_string())?;

        let bot_user_id = bot
            .id()
            .bare_id()
            .ok_or_else(|| "Bot PeerId is the self-user sentinel".to_string())?;

        let bot_ref = bot
            .to_ref()
            .await
            .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
        let result = client_inner
            .send_message(bot_ref, text)
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        let msg_id = result.id();

        Ok((msg_id, bot_user_id))
    }

    /// Ожидать голосовое сообщение от бота с таймаутом
    async fn wait_for_voice_message(
        rx: &mut broadcast::Receiver<Arc<UpdatesLike>>,
        timeout_secs: u64,
        sent_msg_id: i32,
        bot_user_id: i64,
    ) -> Result<VoiceMessageResult, String> {
        info!(timeout_secs, sent_msg_id, "Waiting for voice message");

        let start_time = std::time::Instant::now();
        let total_timeout = std::time::Duration::from_secs(timeout_secs);

        loop {
            let elapsed = start_time.elapsed();
            if elapsed >= total_timeout {
                warn!("Timeout waiting for voice message");
                return Err("Timeout waiting for voice message".to_string());
            }

            let remaining = total_timeout.saturating_sub(elapsed);

            match tokio::time::timeout(remaining, rx.recv()).await {
                Ok(Ok(update)) => {
                    if let Some(result) =
                        Self::extract_voice_from_update(&update, sent_msg_id, bot_user_id)
                    {
                        debug!(
                            "[SILORO] Voice message found: file_id={}, msg_id={}, mime={}",
                            result.file_id, result.msg_id, result.mime_type
                        );
                        return Ok(result);
                    }
                }
                Ok(Err(broadcast::error::RecvError::Closed)) => {
                    warn!("Update stream closed");
                    return Err("Update stream closed".to_string());
                }
                Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                    warn!(skipped = n, "Broadcast receiver lagged");
                    return Err(format!("Update stream lagged (skipped {} messages)", n));
                }
                Err(_) => {
                    // Таймаут одной итерации - продолжаем ждать
                    continue;
                }
            }
        }
    }

    /// Извлечь информацию о голосовом сообщении из обновления
    #[allow(clippy::collapsible_match)]
    fn extract_voice_from_update(
        update_like: &UpdatesLike,
        sent_msg_id: i32,
        bot_user_id: i64,
    ) -> Option<VoiceMessageResult> {
        if let UpdatesLike::Updates(updates_enum) = update_like {
            if let grammers_tl_types::enums::Updates::Updates(u) = updates_enum {
                for update in &u.updates {
                    if let grammers_tl_types::enums::Update::NewMessage(msg) = update {
                        if let grammers_tl_types::enums::Message::Message(m) = &msg.message {
                            let peer_user_id = match &m.peer_id {
                                grammers_tl_types::enums::Peer::User(u) => Some(u.user_id),
                                _ => None,
                            };
                            log_bot_text(
                                "voice",
                                m.out,
                                m.id,
                                peer_user_id,
                                bot_user_id,
                                &m.message,
                            );
                            if let Some(media) = &m.media {
                                if let grammers_tl_types::enums::MessageMedia::Document(doc_media) =
                                    media
                                {
                                    if let Some(grammers_tl_types::enums::Document::Document(doc)) =
                                        &doc_media.document
                                    {
                                        let reply_to_msg_id = match &m.reply_to {
                                            Some(
                                                grammers_tl_types::enums::MessageReplyHeader::Header(
                                                    h,
                                                ),
                                            ) => h.reply_to_msg_id,
                                            _ => None,
                                        };
                                        let matches_request = is_matching_audio_response(
                                            m.out,
                                            m.id,
                                            peer_user_id,
                                            reply_to_msg_id,
                                            &doc.mime_type,
                                            sent_msg_id,
                                            bot_user_id,
                                        );
                                        trace!(
                                            target: "silero_correlation",
                                            candidate_msg_id = m.id,
                                            sent_msg_id,
                                            peer_user_id,
                                            bot_user_id,
                                            reply_to_msg_id,
                                            outgoing = m.out,
                                            mime = %doc.mime_type,
                                            matches_request,
                                            "Evaluated Silero audio response candidate"
                                        );
                                        if matches_request {
                                            return Some(VoiceMessageResult {
                                                file_id: doc.id.to_string(),
                                                msg_id: m.id,
                                                mime_type: doc.mime_type.clone(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Скачать голосовое сообщение во временную папку
    async fn download_voice_to_temp(
        client: &TelegramClient,
        voice: &VoiceMessageResult,
    ) -> Result<String, String> {
        debug!(
            "[SILORO] Downloading voice file_id={}, msg_id={}, mime={}",
            voice.file_id, voice.msg_id, voice.mime_type
        );

        let client_inner = {
            let guard = client.client.lock().await;
            guard
                .clone()
                .ok_or_else(|| "Client not initialized".to_string())?
        };

        let bot = client_inner
            .resolve_username(BOT_USERNAME)
            .await
            .map_err(|e| format!("Failed to resolve bot: {}", e))?
            .ok_or_else(|| "Bot not found".to_string())?;

        // Находим сообщение с нужным file_id
        let bot_ref = bot
            .to_ref()
            .await
            .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
        let mut iter = client_inner.iter_messages(bot_ref);
        let mut msg_count = 0;

        loop {
            match iter.next().await {
                Ok(Some(msg)) => {
                    msg_count += 1;
                    if msg.id() == voice.msg_id {
                        debug!(
                            "[SILORO] Found message {} after checking {} messages",
                            voice.msg_id, msg_count
                        );

                        if let Some(media) = msg.media() {
                            let extension = voice_extension(&voice.mime_type);
                            let temp_dir = Self::get_temp_dir()?;
                            let final_path = voice_final_path(&temp_dir, voice.msg_id, extension);
                            let part_path = voice_part_path(&temp_dir, voice.msg_id, extension);

                            match std::fs::remove_file(&part_path) {
                                Ok(()) => {
                                    debug!(
                                        "Removed stale part file: {}",
                                        crate::secret_log::safe_path_for_log(&part_path)
                                    );
                                }
                                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                                Err(e) => {
                                    return Err(format!(
                                        "Failed to remove stale part file {}: {}",
                                        crate::secret_log::safe_path_for_log(&part_path),
                                        e
                                    ));
                                }
                            }

                            debug!(
                                "Downloading voice file to {}",
                                crate::secret_log::safe_path_for_log(&part_path)
                            );

                            if let Err(e) = client_inner.download_media(&media, &part_path).await {
                                let _ = std::fs::remove_file(&part_path);
                                return Err(format!("Download failed: {}", e));
                            }

                            let (published_path, file_size) =
                                publish_voice_file(&part_path, &final_path)?;

                            info!(
                                msg_id = voice.msg_id,
                                mime = %voice.mime_type,
                                size = file_size,
                                path = %crate::secret_log::safe_path_for_log(&published_path),
                                "Voice file published"
                            );

                            return Ok(published_path
                                .to_str()
                                .ok_or_else(|| "Invalid path".to_string())?
                                .to_string());
                        }
                    }
                }
                Ok(None) => {
                    debug!(
                        "[SILORO] Message {} not found after checking {} messages",
                        voice.msg_id, msg_count
                    );
                    return Err(format!("Message {} not found", voice.msg_id));
                }
                Err(e) => {
                    error!("[SILORO] Error iterating messages: {}", e);
                    continue;
                }
            }
        }
    }

    /// Получить путь к временной папке приложения
    fn get_temp_dir() -> Result<PathBuf, String> {
        let temp_dir = if cfg!(target_os = "windows") {
            let appdata =
                std::env::var("APPDATA").map_err(|e| format!("Failed to get APPDATA: {}", e))?;
            PathBuf::from(appdata).join("ttsbard").join("temp")
        } else if cfg!(target_os = "macos") {
            let home = std::env::var("HOME").map_err(|e| format!("Failed to get HOME: {}", e))?;
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("ttsbard")
                .join("temp")
        } else {
            // Linux
            let home = std::env::var("HOME").map_err(|e| format!("Failed to get HOME: {}", e))?;
            if let Ok(xdg_data) = std::env::var("XDG_DATA_HOME") {
                PathBuf::from(xdg_data).join("ttsbard").join("temp")
            } else {
                PathBuf::from(home)
                    .join(".local")
                    .join("share")
                    .join("ttsbard")
                    .join("temp")
            }
        };

        // Создаем директорию если не существует
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("Failed to create temp dir: {}", e))?;

        Ok(temp_dir)
    }
}

/// Determine file extension from the MIME type of a voice message.
fn voice_extension(mime_type: &str) -> &str {
    if mime_type == "audio/mpeg" {
        "mp3"
    } else {
        "ogg"
    }
}

/// Full path for the published voice file: `silero_tts_<msg_id>.<ext>`.
fn voice_final_path(temp_dir: &Path, msg_id: i32, ext: &str) -> PathBuf {
    temp_dir.join(format!("silero_tts_{}.{}", msg_id, ext))
}

/// Path for the in-progress `.part` download file.
fn voice_part_path(temp_dir: &Path, msg_id: i32, ext: &str) -> PathBuf {
    let final_name = format!("silero_tts_{}.{}", msg_id, ext);
    temp_dir.join(format!("{}.part", final_name))
}

/// Atomically publish a downloaded `.part` file:
///   1. Read metadata – reject zero-byte files and clean up on failure.
///   2. Rename (atomic on the same filesystem) – clean up on failure.
/// Returns the final path and file size on success.
fn publish_voice_file(part_path: &Path, final_path: &Path) -> Result<(PathBuf, u64), String> {
    let metadata = std::fs::metadata(part_path).map_err(|e| {
        let _ = std::fs::remove_file(part_path);
        format!("Failed to read metadata of downloaded file: {}", e)
    })?;
    let file_size = metadata.len();
    if file_size == 0 {
        let _ = std::fs::remove_file(part_path);
        return Err("Downloaded file is empty".to_string());
    }
    std::fs::rename(part_path, final_path).map_err(|e| {
        let _ = std::fs::remove_file(part_path);
        format!("Failed to rename downloaded file: {}", e)
    })?;
    Ok((final_path.to_path_buf(), file_size))
}

/// Pure matching rules for Silero audio responses.
/// Checks peer, message ID ordering, reply-to header, and MIME type.
fn is_matching_audio_response(
    msg_out: bool,
    msg_id: i32,
    peer_user_id: Option<i64>,
    reply_to_msg_id: Option<i32>,
    mime_type: &str,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> bool {
    if msg_out {
        return false;
    }
    if mime_type != "audio/ogg" && mime_type != "audio/mpeg" {
        return false;
    }
    if peer_user_id != Some(bot_user_id) {
        return false;
    }
    if msg_id <= sent_msg_id {
        return false;
    }
    if reply_to_msg_id != Some(sent_msg_id) {
        return false;
    }
    true
}

/// Pure matching rules for reply-less /limits responses.
/// Checks direction, peer, message ID ordering, absence of media,
/// and that text parses as limits.  `reply_markup` is allowed — the
/// content discriminator is `parse_limits_info`.
fn is_matching_limits_response(
    msg_out: bool,
    msg_id: i32,
    peer_user_id: Option<i64>,
    has_media: bool,
    text: &str,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> bool {
    let peer_matched = peer_user_id == Some(bot_user_id);
    let text_len = text.len();
    if msg_out {
        trace!(msg_id, sent_msg_id, "limits candidate rejected: outgoing");
        return false;
    }
    if !peer_matched {
        trace!(
            msg_id,
            peer_user_id,
            bot_user_id,
            "limits candidate rejected: peer mismatch"
        );
        return false;
    }
    if msg_id <= sent_msg_id {
        trace!(
            msg_id,
            sent_msg_id,
            "limits candidate rejected: stale msg_id"
        );
        return false;
    }
    if has_media {
        trace!(msg_id, sent_msg_id, "limits candidate rejected: has media");
        return false;
    }
    if text.is_empty() {
        trace!(msg_id, text_len, "limits candidate rejected: empty text");
        return false;
    }
    let parsed = parse_limits_info(text).is_some();
    trace!(
        msg_id,
        sent_msg_id,
        peer_matched,
        has_media,
        text_len,
        parsed,
        "limits candidate evaluation complete"
    );
    parsed
}

/// Pure matching rules for text replies (used by set-speaker).
/// Checks direction, peer user ID, message ID ordering, and reply-to header.
/// Shared by full message (NewMessage/EditMessage) and short message (UpdateShortMessage) paths.
fn is_matching_text_reply(
    msg_out: bool,
    msg_id: i32,
    user_id: i64,
    reply_to_msg_id: Option<i32>,
    expected_msg_id: i32,
    bot_user_id: i64,
) -> bool {
    !msg_out
        && user_id == bot_user_id
        && msg_id > expected_msg_id
        && reply_to_msg_id == Some(expected_msg_id)
}

impl Default for SileroTtsBot {
    fn default() -> Self {
        Self::new()
    }
}

/// Отправить /speaker и дождаться текстового ответа с текущим голосом
/// Парсит: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
/// Таймаут 1 минута на ожидание ответа
pub async fn get_current_voice(client: &TelegramClient) -> Result<Option<CurrentVoice>, String> {
    info!("Getting current voice from bot");

    // Serialize with other speaker-mutating operations (set_speaker)
    let _ser = client.speaker_serializer.lock().await;

    // 0. Subscribe BEFORE sending to avoid missing fast response
    let mut rx = client.subscribe_updates().await?;

    // 1. Отправляем /speaker и получаем ID сообщения и bot_user_id
    let (sent_message_id, bot_user_id) = send_speaker_command(client).await?;

    info!(sent_message_id, "/speaker sent, waiting for text response");

    // 2. Ждем текстовое сообщение (ответ на наше сообщение)
    let start_time = std::time::Instant::now();
    let total_timeout = std::time::Duration::from_secs(60); // 1 минута

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= total_timeout {
            warn!("Timeout (60s) waiting for voice info");
            return Ok(None); // Таймаут - возвращаем None
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(update)) => {
                trace!("Received update");
                if let Some(voice_info) =
                    extract_voice_info_from_update(&update, sent_message_id, bot_user_id)
                {
                    info!("Voice info found");
                    if !voice_info.id.is_empty() {
                        let mut cache = client.confirmed_speaker.lock().await;
                        *cache = Some(voice_info.id.clone());
                    }
                    return Ok(Some(voice_info));
                }
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                warn!("Update stream closed");
                return Err("Update stream closed".to_string());
            }
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                warn!(skipped = n, "Broadcast receiver lagged");
                return Err(format!("Update stream lagged (skipped {} messages)", n));
            }
            Err(_) => {
                continue;
            }
        }
    }
}

/// Отправить команду /speaker боту
/// Возвращает (sent_message_id, bot_user_id)
async fn send_speaker_command(client: &TelegramClient) -> Result<(i32, i64), String> {
    info!("Sending /speaker to bot");

    let client_inner = {
        let guard = client.client.lock().await;
        guard
            .clone()
            .ok_or_else(|| "Client not initialized".to_string())?
    };

    let bot = client_inner
        .resolve_username(BOT_USERNAME)
        .await
        .map_err(|e| format!("Failed to resolve bot: {}", e))?
        .ok_or_else(|| "Bot not found".to_string())?;

    let bot_user_id = bot
        .id()
        .bare_id()
        .ok_or_else(|| "Bot PeerId is the self-user sentinel".to_string())?;

    let bot_ref = bot
        .to_ref()
        .await
        .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
    let result = client_inner
        .send_message(bot_ref, "/speaker")
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    let msg_id = result.id();

    Ok((msg_id, bot_user_id))
}

/// Извлечь информацию о текущем голосе из текстового сообщения
/// Парсит: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
/// Проверяет что сообщение является ответом на сообщение с expected_msg_id,
/// что оно входящее, и от Silero бота.
#[allow(clippy::collapsible_match)]
fn extract_voice_info_from_update(
    update_like: &UpdatesLike,
    expected_msg_id: i32,
    bot_user_id: i64,
) -> Option<CurrentVoice> {
    if let UpdatesLike::Updates(updates_enum) = update_like {
        if let grammers_tl_types::enums::Updates::Updates(u) = updates_enum {
            for update in &u.updates {
                if let grammers_tl_types::enums::Update::NewMessage(msg) = update {
                    if let grammers_tl_types::enums::Message::Message(m) = &msg.message {
                        // Игнорируем исходящие сообщения (наши собственные)
                        if m.out {
                            trace!("Skipping outgoing message");
                            continue;
                        }

                        // Validate peer is Silero bot
                        let peer_user_id = match &m.peer_id {
                            grammers_tl_types::enums::Peer::User(u) => Some(u.user_id),
                            _ => None,
                        };
                        if peer_user_id != Some(bot_user_id) {
                            trace!(
                                peer_user_id,
                                bot_user_id,
                                "Skipping message from wrong peer"
                            );
                            continue;
                        }

                        log_bot_text(
                            "speaker",
                            m.out,
                            m.id,
                            peer_user_id,
                            bot_user_id,
                            &m.message,
                        );

                        // Проверяем что это ответ на наше сообщение
                        match &m.reply_to {
                            Some(grammers_tl_types::enums::MessageReplyHeader::Header(h))
                                if h.reply_to_msg_id == Some(expected_msg_id) =>
                            {
                                // Это ответ на наше сообщение - обрабатываем
                            }
                            _ => {
                                trace!(
                                    has_reply_to = m.reply_to.is_some(),
                                    expected = expected_msg_id,
                                    "Skipping message - not a reply to our message"
                                );
                                continue;
                            }
                        }

                        trace!(
                            has_media = m.media.is_some(),
                            has_reply_markup = m.reply_markup.is_some(),
                            text_len = m.message.len(),
                            "Processing reply to our message"
                        );

                        // Ищем текстовое сообщение (без медиа)
                        // reply_markup может быть (инлайн-кнопки бота)
                        if m.media.is_none() {
                            // В TL типе Message текст находится в поле message
                            let text = &m.message;
                            if !text.is_empty() {
                                trace!("Attempting to parse voice info");
                                // Парсим текст
                                if let Some(voice) = parse_voice_info(text) {
                                    return Some(voice);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Парсит текст ответа бота для получения информации о голосе
/// Формат: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
fn parse_voice_info(text: &str) -> Option<CurrentVoice> {
    trace!("Parsing voice info");

    // Ищем строки с ключевыми словами
    let mut voice_id: Option<String> = None;
    let mut voice_name: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();

        // Парсим "Выбранный голос: /speaker hamster_clerk"
        if line.contains("Выбранный голос:") || line.contains("Выбраний голос:")
        {
            let parts: Vec<&str> = line.split_whitespace().collect();
            for (i, part) in parts.iter().enumerate() {
                if *part == "/speaker" {
                    if let Some(id) = parts.get(i + 1) {
                        voice_id = Some(id.to_string());
                    }
                    break;
                }
            }
        }

        // Парсим "Находится в паке: Хомяки" или "Знаходиться в паке:"
        if line.contains("Находится в паке:")
            || line.contains("Знаходиться в паке:")
            || line.contains("находится в паке:")
            || line.contains("знаходиться в паке:")
        {
            if let Some(idx) = line.find(':') {
                let name = line[idx + 1..].trim();
                if !name.is_empty() {
                    voice_name = Some(name.to_string());
                }
            }
        }
    }

    // Возвращаем результат если нашли оба поля
    if let (Some(id), Some(name)) = (voice_id, voice_name) {
        trace!("Parsed voice info");
        Some(CurrentVoice { name, id })
    } else {
        warn!("Failed to parse voice info from text");
        None
    }
}

/// Отправить /limits и дождаться текстового ответа с лимитами
/// Парсит: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
/// Таймаут 60 секунд на ожидание ответа
/// Serialized via TelegramClient::limits_mutex to prevent two concurrent /
/// limits calls from accepting each other's responses.
pub async fn get_limits(client: &TelegramClient) -> Result<Option<Limits>, String> {
    // Serialize all /limits request-response pairs
    let _limits_guard = client.limits_mutex.lock().await;

    info!("Getting limits from bot");

    // 0. Subscribe BEFORE sending
    let mut rx = client.subscribe_updates().await?;

    // 1. Отправляем /limits
    let (sent_msg_id, bot_user_id) = send_limits_command(client).await?;

    info!(sent_msg_id, "/limits sent, waiting for text response");

    // 2. Ждем текстовое сообщение (не меню, не голос), от Silero бота, incoming,
    //    msg_id > sent_msg_id
    let start_time = std::time::Instant::now();
    let total_timeout = std::time::Duration::from_secs(60); // 60 секунд

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= total_timeout {
            warn!("Timeout (60s) waiting for limits info");
            return Ok(None);
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(update)) => {
                if let Some(limits_info) =
                    extract_limits_info_from_update(&update, sent_msg_id, bot_user_id)
                {
                    info!(limits_info.voices, limits_info.gifs, "Limits info found");
                    return Ok(Some(limits_info));
                }
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                warn!("Update stream closed");
                return Err("Update stream closed".to_string());
            }
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                warn!(skipped = n, "Broadcast receiver lagged");
                return Err(format!("Update stream lagged (skipped {} messages)", n));
            }
            Err(_) => {
                continue;
            }
        }
    }
}

/// Отправить команду /limits боту
/// Возвращает (sent_message_id, bot_user_id)
async fn send_limits_command(client: &TelegramClient) -> Result<(i32, i64), String> {
    info!("Sending /limits to bot");

    let client_inner = {
        let guard = client.client.lock().await;
        guard
            .clone()
            .ok_or_else(|| "Client not initialized".to_string())?
    };

    let bot = client_inner
        .resolve_username(BOT_USERNAME)
        .await
        .map_err(|e| format!("Failed to resolve bot: {}", e))?
        .ok_or_else(|| "Bot not found".to_string())?;

    let bot_user_id = bot
        .id()
        .bare_id()
        .ok_or_else(|| "Bot PeerId is the self-user sentinel".to_string())?;

    let bot_ref = bot
        .to_ref()
        .await
        .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
    let result = client_inner
        .send_message(bot_ref, "/limits")
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    let msg_id = result.id();

    Ok((msg_id, bot_user_id))
}

/// Извлечь информацию о лимитах из текстового сообщения.
/// Delegates to specialised helpers based on the `UpdatesLike` variant.
fn extract_limits_info_from_update(
    update_like: &UpdatesLike,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> Option<Limits> {
    match update_like {
        UpdatesLike::Updates(updates_enum) => match updates_enum {
            grammers_tl_types::enums::Updates::Updates(u) => {
                extract_limits_info_from_updates(&u.updates, sent_msg_id, bot_user_id)
            }
            grammers_tl_types::enums::Updates::Combined(u) => {
                extract_limits_info_from_updates(&u.updates, sent_msg_id, bot_user_id)
            }
            grammers_tl_types::enums::Updates::UpdateShortMessage(m) => {
                extract_limits_info_from_short_message(m, sent_msg_id, bot_user_id)
            }
            _ => None,
        },
        _ => None,
    }
}

fn extract_limits_info_from_updates(
    updates: &[grammers_tl_types::enums::Update],
    sent_msg_id: i32,
    bot_user_id: i64,
) -> Option<Limits> {
    for update in updates {
        match update {
            grammers_tl_types::enums::Update::NewMessage(msg) => {
                if let Some(result) =
                    extract_limits_info_from_message(&msg.message, sent_msg_id, bot_user_id)
                {
                    return Some(result);
                }
            }
            grammers_tl_types::enums::Update::EditMessage(msg) => {
                if let Some(result) =
                    extract_limits_info_from_message(&msg.message, sent_msg_id, bot_user_id)
                {
                    return Some(result);
                }
            }
            _ => {}
        }
    }
    None
}

fn extract_limits_info_from_message(
    message: &grammers_tl_types::enums::Message,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> Option<Limits> {
    match message {
        grammers_tl_types::enums::Message::Message(m) => {
            let peer_user_id = match &m.peer_id {
                grammers_tl_types::enums::Peer::User(u) => Some(u.user_id),
                _ => None,
            };
            log_bot_text("limits", m.out, m.id, peer_user_id, bot_user_id, &m.message);
            trace!(
                candidate_msg_id = m.id,
                sent_msg_id,
                peer = peer_user_id,
                bot = bot_user_id,
                has_media = m.media.is_some(),
                has_reply_markup = m.reply_markup.is_some(),
                text_len = m.message.len(),
                "Evaluating limits candidate"
            );
            if is_matching_limits_response(
                m.out,
                m.id,
                peer_user_id,
                m.media.is_some(),
                &m.message,
                sent_msg_id,
                bot_user_id,
            ) {
                trace!(msg_id = m.id, "Limits candidate accepted");
                return parse_limits_info(&m.message);
            }
        }
        _ => {}
    }
    None
}

fn extract_limits_info_from_short_message(
    m: &grammers_tl_types::types::UpdateShortMessage,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> Option<Limits> {
    log_bot_text(
        "limits",
        m.out,
        m.id,
        Some(m.user_id),
        bot_user_id,
        &m.message,
    );
    trace!(
        candidate_msg_id = m.id,
        sent_msg_id,
        peer = m.user_id,
        bot = bot_user_id,
        has_media = false,
        text_len = m.message.len(),
        "Evaluating limits candidate (short message)"
    );
    if is_matching_limits_response(
        m.out,
        m.id,
        Some(m.user_id),
        false,
        &m.message,
        sent_msg_id,
        bot_user_id,
    ) {
        trace!(msg_id = m.id, "Limits candidate accepted (short message)");
        return parse_limits_info(&m.message);
    }
    None
}

/// Парсит текст ответа бота для получения информации о лимитах.
/// Supports two layouts:
/// 1. Legacy inline: header and counter on the same line
///    "Открытые голоса: 0 / 666 символов;"
/// 2. Multiline: header on one line, counter on the next non-empty line
///    "🔓 Открытые голоса:\n       34 / 666 символов;"
/// Markdown emphasis (`**label:**`) is stripped before matching section headers.
/// Both open-voices and gifs counters are required; other sections are ignored.
fn parse_limits_info(text: &str) -> Option<Limits> {
    trace!("Parsing limits info");

    let mut voices: Option<String> = None;
    let mut gifs: Option<String> = None;

    #[derive(PartialEq)]
    enum Pending {
        Voices,
        Gifs,
        Other,
    }

    let mut pending: Option<Pending> = None;
    let lines: Vec<&str> = text.lines().collect();

    for i in 0..lines.len() {
        let raw = lines[i];
        let line = raw.trim();

        if line.is_empty() {
            continue;
        }

        // If we have a pending section, try to match a counter on this line first.
        if let Some(ref section) = pending {
            if let Some(counter) = try_parse_slash_counter(line) {
                match section {
                    Pending::Voices => voices = Some(counter),
                    Pending::Gifs => gifs = Some(counter),
                    Pending::Other => { /* ignored */ }
                }
                pending = None;
                continue;
            }
            // No counter on this line, reset pending and re-evaluate as header.
            pending = None;
        }

        // Strip Markdown bold/italic markers for header matching.
        let cleaned = strip_markdown_emphasis(line);

        if is_voices_header(cleaned) {
            if let Some(counter) = try_parse_counter_after_colon(cleaned) {
                voices = Some(counter);
            } else {
                pending = Some(Pending::Voices);
            }
        } else if is_gifs_header(cleaned) {
            if let Some(counter) = try_parse_counter_after_colon(cleaned) {
                gifs = Some(counter);
            } else {
                pending = Some(Pending::Gifs);
            }
        } else if is_other_section_header(cleaned) {
            // Recognised section that we deliberately ignore — mark as Other
            // so we skip the next line without binding it to voices/gifs.
            pending = Some(Pending::Other);
        } else {
            // Unrecognised line — clear any stale pending.
            pending = None;
        }
    }

    if let (Some(voices_val), Some(gifs_val)) = (voices, gifs) {
        let reset_timestamp = parse_reset_timestamp(text);
        trace!(voices_val, gifs_val, "Parsed limits info");
        Some(Limits {
            voices: voices_val,
            gifs: gifs_val,
            reset_timestamp,
        })
    } else {
        trace!("Failed to parse limits info from text");
        None
    }
}

/// Strip surrounding `**` or `*` Markdown emphasis markers from a trimmed line.
fn strip_markdown_emphasis(s: &str) -> &str {
    if s.starts_with("**") && s.ends_with("**") && s.len() >= 4 {
        &s[2..s.len() - 2]
    } else if s.starts_with('*') && s.ends_with('*') && s.len() >= 2 {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

/// Check if a trimmed, stripped line is a recognised voices section header.
fn is_voices_header(line: &str) -> bool {
    line.contains("Открытые голоса:") || line.contains("Відкриті голоси:")
}

/// Check if a trimmed, stripped line is a recognised gifs section header.
fn is_gifs_header(line: &str) -> bool {
    line.contains("Кружки/гифки:") || line.contains("Кружки/гіфки:") || line.contains("Гифки:")
}

/// Recognised section headers that we intentionally ignore.
fn is_other_section_header(line: &str) -> bool {
    line.starts_with("🎙")
        && (line.contains("Переозвучка:")
            || line.contains("Переозвучення:")
            || line.contains("Только текст:")
            || line.contains("Тільки текст:"))
}

/// Try to parse a `number / number` counter from a trimmed line.
/// Both sides must consist purely of decimal digits.
fn try_parse_slash_counter(line: &str) -> Option<String> {
    let slash_pos = line.find('/')?;

    let before = line[..slash_pos].trim();
    let after = line[slash_pos + 1..].trim();

    // The "total" side may be followed by text (e.g. " символов;"), so find the first whitespace.
    let total_str = if let Some(ws) = after.find(|c: char| c.is_whitespace() || c == ';') {
        &after[..ws]
    } else {
        after
    };

    if !before.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if !total_str.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if before.is_empty() || total_str.is_empty() {
        return None;
    }

    Some(format!("{} / {}", before, total_str))
}

/// Try to parse a counter from the part after the colon on a header line.
fn try_parse_counter_after_colon(line: &str) -> Option<String> {
    let colon_pos = line.find(':')?;
    let after_colon = line[colon_pos + 1..].trim();
    if after_colon.is_empty() {
        return None;
    }
    try_parse_slash_counter(after_colon)
}

/// Извлекает опциональный timestamp сброса лимитов из текста.
/// Поддерживает русский "обновится" и украинский "оновлюється".
/// Formats:
///   1. Full-year:  ключ YYYY-MM-DD HH:mm:ss UTC±N
///   2. Legacy:     ключ MM-DD HH:mm:ss UTC±N
/// Возвращает None если суффикс отсутствует или невалидный.
fn parse_reset_timestamp(text: &str) -> Option<String> {
    // Try full-year format first.
    if let Some(result) = try_parse_reset_full_year(text) {
        return Some(result);
    }
    // Fall back to legacy yearless format.
    try_parse_reset_legacy(text)
}

const FULL_YEAR_TS_RE: &str = r"(?i)(?:обновится|оновлюється)\s+(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\s+UTC([+-])(\d{1,2})";

const LEGACY_TS_RE: &str = r"(?i)(?:обновится|оновлюється)\s+(\d{2})-(\d{2})\s+(\d{2}):(\d{2}):(\d{2})\s+UTC([+-])(\d{1,2})";

fn try_parse_reset_full_year(text: &str) -> Option<String> {
    let re = Regex::new(FULL_YEAR_TS_RE).ok()?;
    let caps = re.captures(text)?;
    let full_match = caps.get(0)?;

    let end = full_match.end();
    if let Some(next) = text[end..].chars().next() {
        if next.is_alphanumeric() || next == '-' {
            return None;
        }
    }

    let year: u32 = caps.get(1)?.as_str().parse().ok()?;
    let month: u32 = caps.get(2)?.as_str().parse().ok()?;
    let day: u32 = caps.get(3)?.as_str().parse().ok()?;
    let hour: u32 = caps.get(4)?.as_str().parse().ok()?;
    let minute: u32 = caps.get(5)?.as_str().parse().ok()?;
    let second: u32 = caps.get(6)?.as_str().parse().ok()?;
    let sign = caps.get(7)?.as_str();
    let offset_val: i32 = caps.get(8)?.as_str().parse().ok()?;

    if year < 2000 || year > 2099 {
        return None;
    }
    if month < 1 || month > 12 {
        return None;
    }
    if day < 1 || day > 31 {
        return None;
    }
    if hour > 23 {
        return None;
    }
    if minute > 59 {
        return None;
    }
    if second > 59 {
        return None;
    }

    let offset = if sign == "-" { -offset_val } else { offset_val };
    if offset < -12 || offset > 14 {
        return None;
    }

    Some(format!(
        "{}-{:02}-{:02} {:02}:{:02}:{:02} UTC{}{}",
        year, month, day, hour, minute, second, sign, offset_val,
    ))
}

fn try_parse_reset_legacy(text: &str) -> Option<String> {
    let re = Regex::new(LEGACY_TS_RE).ok()?;
    let caps = re.captures(text)?;
    let full_match = caps.get(0)?;

    let end = full_match.end();
    if let Some(next) = text[end..].chars().next() {
        if next.is_alphanumeric() || next == '-' {
            return None;
        }
    }

    let month: u32 = caps.get(1)?.as_str().parse().ok()?;
    let day: u32 = caps.get(2)?.as_str().parse().ok()?;
    let hour: u32 = caps.get(3)?.as_str().parse().ok()?;
    let minute: u32 = caps.get(4)?.as_str().parse().ok()?;
    let second: u32 = caps.get(5)?.as_str().parse().ok()?;
    let sign = caps.get(6)?.as_str();
    let offset_val: i32 = caps.get(7)?.as_str().parse().ok()?;

    if month < 1 || month > 12 {
        return None;
    }
    if day < 1 || day > 31 {
        return None;
    }
    if hour > 23 {
        return None;
    }
    if minute > 59 {
        return None;
    }
    if second > 59 {
        return None;
    }

    let offset = if sign == "-" { -offset_val } else { offset_val };
    if offset < -12 || offset > 14 {
        return None;
    }

    Some(format!(
        "{:02}-{:02} {:02}:{:02}:{:02} UTC{}{}",
        month, day, hour, minute, second, sign, offset_val,
    ))
}

/// Отправить "/speaker {code}" боту и дождаться текстового ответа
/// Возвращает true если успешно, иначе ошибку
pub async fn set_speaker(client: &TelegramClient, voice_code: &str) -> Result<bool, String> {
    // Serialize the full check-request-update cycle; avoid holding the
    // cache mutex across nested network helpers.
    let _ser = client.speaker_serializer.lock().await;

    // Check only while serialized so the cached state cannot change underneath us.
    {
        let cached = client.confirmed_speaker.lock().await;
        if cached.as_deref() == Some(voice_code) {
            debug!(voice_code, "Speaker already confirmed, skipping /speaker");
            return Ok(true);
        }
    }

    info!("Setting speaker voice");

    // 0. Subscribe BEFORE sending
    let mut rx = client.subscribe_updates().await?;

    // 1. Отправить "/speaker {code}" и получить ID сообщения и bot_user_id
    let (sent_message_id, bot_user_id) = send_speaker_command_with_code(client, voice_code).await?;

    info!("Waiting for bot response to msg_id={}", sent_message_id);

    // 2. Ждем текстовое сообщение (ответ на наше сообщение)
    let start_time = std::time::Instant::now();
    let total_timeout = std::time::Duration::from_secs(30); // 30 секунд

    loop {
        let elapsed = start_time.elapsed();
        if elapsed >= total_timeout {
            warn!("Timeout (30s) waiting for set_speaker response");
            *client.confirmed_speaker.lock().await = None;
            return Err("Timeout waiting for speaker change response".to_string());
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, rx.recv()).await {
            Ok(Ok(update)) => {
                if let Some(result) =
                    extract_set_speaker_response_from_update(&update, sent_message_id, bot_user_id)
                {
                    info!("Set speaker response: {}", result);
                    if result {
                        let mut cache = client.confirmed_speaker.lock().await;
                        *cache = Some(voice_code.to_string());
                        return Ok(true);
                    } else {
                        *client.confirmed_speaker.lock().await = None;
                        return Err("Invalid voice code".to_string());
                    }
                }
            }
            Ok(Err(broadcast::error::RecvError::Closed)) => {
                warn!("Update stream closed");
                *client.confirmed_speaker.lock().await = None;
                return Err("Update stream closed".to_string());
            }
            Ok(Err(broadcast::error::RecvError::Lagged(n))) => {
                warn!(skipped = n, "Broadcast receiver lagged");
                *client.confirmed_speaker.lock().await = None;
                return Err(format!("Update stream lagged (skipped {} messages)", n));
            }
            Err(_) => {
                continue;
            }
        }
    }
}

/// Отправить команду "/speaker {code}" боту
/// Возвращает (sent_message_id, bot_user_id)
async fn send_speaker_command_with_code(
    client: &TelegramClient,
    voice_code: &str,
) -> Result<(i32, i64), String> {
    info!("Sending set_speaker command to bot");

    let client_inner = {
        let guard = client.client.lock().await;
        guard
            .clone()
            .ok_or_else(|| "Client not initialized".to_string())?
    };

    let bot = client_inner
        .resolve_username(BOT_USERNAME)
        .await
        .map_err(|e| format!("Failed to resolve bot: {}", e))?
        .ok_or_else(|| "Bot not found".to_string())?;

    let bot_user_id = bot
        .id()
        .bare_id()
        .ok_or_else(|| "Bot PeerId is the self-user sentinel".to_string())?;

    let command = format!("/speaker {}", voice_code);

    let bot_ref = bot
        .to_ref()
        .await
        .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
    let result = client_inner
        .send_message(bot_ref, command)
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    let msg_id = result.id();
    info!(
        msg_id,
        "Sent set_speaker command, waiting for text response"
    );

    Ok((msg_id, bot_user_id))
}

/// Извлечь ответ об установке спикера из текстового сообщения.
/// Validates peer, direction, ID ordering, and reply-to match via
/// `is_matching_text_reply`. Rejects `UpdateShortChatMessage` entirely
/// (a chat update is not the private Silero user peer).
fn extract_set_speaker_response_from_update(
    update_like: &UpdatesLike,
    expected_msg_id: i32,
    bot_user_id: i64,
) -> Option<bool> {
    match update_like {
        UpdatesLike::Updates(updates_enum) => match updates_enum {
            grammers_tl_types::enums::Updates::Updates(u) => {
                for update in &u.updates {
                    match update {
                        grammers_tl_types::enums::Update::NewMessage(msg) => {
                            if let Some(result) =
                                process_message(&msg.message, expected_msg_id, bot_user_id)
                            {
                                return Some(result);
                            }
                        }
                        grammers_tl_types::enums::Update::NewChannelMessage(msg) => {
                            if let Some(result) =
                                process_message(&msg.message, expected_msg_id, bot_user_id)
                            {
                                return Some(result);
                            }
                        }
                        grammers_tl_types::enums::Update::EditMessage(msg) => {
                            if let Some(result) =
                                process_message(&msg.message, expected_msg_id, bot_user_id)
                            {
                                return Some(result);
                            }
                        }
                        grammers_tl_types::enums::Update::EditChannelMessage(msg) => {
                            if let Some(result) =
                                process_message(&msg.message, expected_msg_id, bot_user_id)
                            {
                                return Some(result);
                            }
                        }
                        _ => {}
                    }
                }
            }
            grammers_tl_types::enums::Updates::UpdateShortMessage(msg) => {
                let reply_to_msg_id = match &msg.reply_to {
                    Some(grammers_tl_types::enums::MessageReplyHeader::Header(h)) => {
                        h.reply_to_msg_id
                    }
                    _ => None,
                };
                log_bot_text(
                    "set_speaker",
                    msg.out,
                    msg.id,
                    Some(msg.user_id),
                    bot_user_id,
                    &msg.message,
                );
                if is_matching_text_reply(
                    msg.out,
                    msg.id,
                    msg.user_id,
                    reply_to_msg_id,
                    expected_msg_id,
                    bot_user_id,
                ) && !msg.message.is_empty()
                {
                    return Some(parse_message_text_with_validation(&msg.message));
                }
            }
            grammers_tl_types::enums::Updates::UpdateShortChatMessage(_) => {
                // Chat update: not the private Silero user peer, reject.
            }
            _ => {}
        },
        _ => {}
    }
    None
}

/// Обработать Message enum (используется для NewMessage, EditMessage и т.д.)
/// Uses `is_matching_text_reply` for peer/direction/id/reply validation,
/// then checks for plain text (no media).
fn process_message(
    message: &grammers_tl_types::enums::Message,
    expected_msg_id: i32,
    bot_user_id: i64,
) -> Option<bool> {
    match message {
        grammers_tl_types::enums::Message::Message(m) => {
            // Extract user_id from peer
            let user_id = match &m.peer_id {
                grammers_tl_types::enums::Peer::User(u) => u.user_id,
                _ => return None,
            };
            let reply_to_msg_id = match &m.reply_to {
                Some(grammers_tl_types::enums::MessageReplyHeader::Header(h)) => h.reply_to_msg_id,
                _ => None,
            };
            log_bot_text(
                "set_speaker",
                m.out,
                m.id,
                Some(user_id),
                bot_user_id,
                &m.message,
            );
            if !is_matching_text_reply(
                m.out,
                m.id,
                user_id,
                reply_to_msg_id,
                expected_msg_id,
                bot_user_id,
            ) {
                return None;
            }
            if m.media.is_none() && !m.message.is_empty() {
                return Some(parse_message_text_with_validation(&m.message));
            }
        }
        _ => {}
    }
    None
}

/// Спарсить текст сообщения с валидацией
fn parse_message_text_with_validation(text: &str) -> bool {
    match parse_set_speaker_response(text) {
        Ok(result) => result,
        Err(_) => false,
    }
}

/// Парсит текст ответа бота для set_speaker
/// Возвращает Ok(true) если успешно, Err если неверный код
fn parse_set_speaker_response(text: &str) -> Result<bool, String> {
    if text.contains("Успешно выбран спикер")
        || text.contains("Успішно обрано спікера")
        || text.contains("Successfully selected speaker")
    {
        return Ok(true);
    }

    if text.contains("Успешно выбран тот же самый спикер")
        || text.contains("Успішно обрано того самого спікера")
    {
        return Ok(true);
    }

    if text.contains("Указан неверный голос") || text.contains("Вказано невірний голос")
    {
        return Err("Invalid voice code".to_string());
    }

    Err("Unknown response format".to_string())
}

/// Pure predicate: should we log inbound Silero bot text?
/// All four conditions must be met:
///   - message is incoming (`out == false`)
///   - peer is the resolved Silero bot (`peer_user_id == Some(bot_user_id)`)
///   - environment variable `TTSBARD_LOG_TELEGRAM_TEXT` is exactly `"1"`
#[cfg(any(debug_assertions, test))]
fn should_log_bot_text(
    out: bool,
    peer_user_id: Option<i64>,
    bot_user_id: i64,
    env_val: Option<&str>,
) -> bool {
    !out && peer_user_id == Some(bot_user_id) && env_val == Some("1")
}

/// Log the full text of an inbound Silero bot message when all security
/// conditions are satisfied.  Compile-time no-op in release builds.
#[cfg(debug_assertions)]
fn log_bot_text(
    ctx: &str,
    out: bool,
    msg_id: i32,
    peer_user_id: Option<i64>,
    bot_user_id: i64,
    text: &str,
) {
    let env_val = std::env::var("TTSBARD_LOG_TELEGRAM_TEXT").ok();
    if should_log_bot_text(out, peer_user_id, bot_user_id, env_val.as_deref()) {
        debug!(
            target: "silero_inbound_text",
            ctx,
            msg_id,
            bot_user_id,
            text,
            "Inbound Silero bot text",
        );
    }
}

#[cfg(not(debug_assertions))]
fn log_bot_text(
    _ctx: &str,
    _out: bool,
    _msg_id: i32,
    _peer_user_id: Option<i64>,
    _bot_user_id: i64,
    _text: &str,
) {
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_voice_info ──────────────────────────────────────────────

    #[test]
    fn parse_voice_info_ru_both_fields() {
        let text = "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки";
        let v = parse_voice_info(text).expect("must parse");
        assert_eq!(v.id, "hamster_clerk");
        assert_eq!(v.name, "Хомяки");
    }

    #[test]
    fn parse_voice_info_ua_both_fields() {
        let text = "Выбраний голос: /speaker baya\nЗнаходиться в паке: Бая";
        let v = parse_voice_info(text).expect("must parse");
        assert_eq!(v.id, "baya");
        assert_eq!(v.name, "Бая");
    }

    #[test]
    fn parse_voice_info_ua_lowercase_name_label() {
        // The UA name label has a registered lowercase variant in production:
        //   line.contains("знаходиться в паке:")
        let text = "Выбраний голос: /speaker xenia\nзнаходиться в паке: Ксенія";
        let v = parse_voice_info(text).expect("must parse");
        assert_eq!(v.id, "xenia");
        assert_eq!(v.name, "Ксенія");
    }

    #[test]
    fn parse_voice_info_ua_lowercase_name_label_no_match() {
        // The UA voice label does NOT have a lowercase variant registered.
        // Only the name label has both uppercase and lowercase patterns.
        let text = "вибраний голос: /speaker dog_bark\nЗнаходиться в паке: Гавкуни";
        assert!(parse_voice_info(text).is_none());
    }

    #[test]
    fn parse_voice_info_whitespace_and_noise() {
        let text = "  \tВыбранный голос: /speaker   glide  \n  extra junk  \n  Находится в паке:  Плавный  \n";
        let v = parse_voice_info(text).expect("must parse");
        assert_eq!(v.id, "glide");
        assert_eq!(v.name, "Плавный");
    }

    #[test]
    fn parse_voice_info_missing_id() {
        let text = "Выбранный голос: \nНаходится в паке: Хомяки";
        assert!(parse_voice_info(text).is_none());
    }

    #[test]
    fn parse_voice_info_missing_name() {
        let text = "Выбранный голос: /speaker dog\n";
        assert!(parse_voice_info(text).is_none());
    }

    #[test]
    fn parse_voice_info_empty_values() {
        let text = "Выбранный голос: /speaker \nНаходится в паке: ";
        assert!(parse_voice_info(text).is_none());
    }

    #[test]
    fn parse_voice_info_unrelated_input() {
        assert!(parse_voice_info("Hello world").is_none());
        assert!(parse_voice_info("/limits 0 / 666").is_none());
        assert!(parse_voice_info("").is_none());
    }

    // ── parse_limits_info ──────────────────────────────────────────────

    #[test]
    fn parse_limits_info_ru_both_sections() {
        let text = "🔓 Открытые голоса: 0 / 666 символов;\n🪩 Кружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
    }

    #[test]
    fn parse_limits_info_ua_both_sections() {
        let text = "🔓 Відкриті голоси: 5 / 333 символів;\n🪩 Кружки/гіфки: 2 / 10 повідомлень;";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "5 / 333");
        assert_eq!(l.gifs, "2 / 10");
    }

    #[test]
    fn parse_limits_info_gifs_label_gifki() {
        let text = "Открытые голоса: 1 / 500 символов;\nГифки: 3 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "1 / 500");
        assert_eq!(l.gifs, "3 / 10");
    }

    #[test]
    fn parse_limits_info_whitespace_variations() {
        let text =
            "  \tОткрытые голоса:  10  /  200  символов;  \nextra stuff\nКружки/гифки:  7  /  15  сообщений;  ";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "10 / 200");
        assert_eq!(l.gifs, "7 / 15");
    }

    #[test]
    fn parse_limits_info_missing_voices() {
        let text = "Кружки/гифки: 0 / 10 сообщений;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_info_missing_gifs() {
        let text = "Открытые голоса: 0 / 666 символов;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_info_malformed_no_slash_voices() {
        let text = "Открытые голоса: 666 символов;\nКружки/гифки: 0 / 10 сообщений;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_info_malformed_no_slash_gifs() {
        let text = "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 10 сообщений;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_info_unrelated_input() {
        assert!(parse_limits_info("Hello world").is_none());
        assert!(parse_limits_info("random text").is_none());
        assert!(parse_limits_info("").is_none());
    }

    #[test]
    fn parse_limits_info_reset_ru_positive_offset() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:47:27 UTC+3\nКружки/гифки: 0 / 10 сообщений; обновится 07-27 00:47:27 UTC+3";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert_eq!(l.reset_timestamp.as_deref(), Some("07-27 00:47:27 UTC+3"));
    }

    #[test]
    fn parse_limits_info_reset_ua_negative_offset() {
        let text = "Відкриті голоси: 5 / 333 символів;\nКружки/гіфки: 2 / 10 повідомлень; оновлюється 12-31 23:59:59 UTC-5";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "5 / 333");
        assert_eq!(l.gifs, "2 / 10");
        assert_eq!(l.reset_timestamp.as_deref(), Some("12-31 23:59:59 UTC-5"));
    }

    #[test]
    fn parse_limits_info_reset_absent() {
        let text = "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_malformed_missing_field() {
        let text = "Открытые голоса: 1 / 500 символов; обновится 07-27 00:47 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "1 / 500");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_malformed_garbage_after_keyword() {
        let text =
            "Открытые голоса: 0 / 666 символов; обновится garbage\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_full_year_ru() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 2025-07-27 00:47:27 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert_eq!(
            l.reset_timestamp.as_deref(),
            Some("2025-07-27 00:47:27 UTC+3")
        );
    }

    #[test]
    fn parse_limits_info_reset_malformed_no_utc_offset() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:47:27\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_month() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 13-01 00:00:00 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_day() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-32 00:00:00 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_hour() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 24:00:00 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_minute() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:60:00 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_second() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:00:60 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_utc_offset_too_large() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:00:00 UTC+15\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_impossible_utc_offset_too_negative() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:00:00 UTC-13\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_trailing_junk() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:47:27 UTC+3extra\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_info_reset_trailing_digits() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 07-27 00:47:27 UTC+312\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert!(l.reset_timestamp.is_none());
    }

    // ── Multiline payload tests ────────────────────────────────────────

    #[test]
    fn parse_limits_multiline_real_payload() {
        let text = "🔓 Открытые голоса:\n       34 / 666 символов;\n       Обновится 2026-07-27 00:47:27 UTC+3.\n\n🪩 Кружки/гифки:\n       0 / 10 сообщений;\n\n🎙 Переозвучка:\n       0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse multiline payload");
        assert_eq!(l.voices, "34 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert_eq!(
            l.reset_timestamp.as_deref(),
            Some("2026-07-27 00:47:27 UTC+3")
        );
    }

    #[test]
    fn parse_limits_multiline_markdown_emphasis() {
        let text = "**🔓 Открытые голоса:**\n       34 / 666 символов;\n\n**🪩 Кружки/гифки:**\n       0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse Markdown-emphasized headers");
        assert_eq!(l.voices, "34 / 666");
        assert_eq!(l.gifs, "0 / 10");
    }

    #[test]
    fn parse_limits_multiline_single_asterisk_emphasis() {
        let text = "*🔓 Открытые голоса:*\n       34 / 666 символов;\n\n*🪩 Кружки/гифки:*\n       0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse single-asterisk emphasized headers");
        assert_eq!(l.voices, "34 / 666");
        assert_eq!(l.gifs, "0 / 10");
    }

    #[test]
    fn parse_limits_multiline_gifs_gifki_label() {
        let text = "Открытые голоса:\n       5 / 333 символов;\n\nГифки:\n       2 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse gifs via Гифки label");
        assert_eq!(l.voices, "5 / 333");
        assert_eq!(l.gifs, "2 / 10");
    }

    #[test]
    fn parse_limits_multiline_ua() {
        let text = "Відкриті голоси:\n       10 / 500 символів;\n\nКружки/гіфки:\n       3 / 10 повідомлень;";
        let l = parse_limits_info(text).expect("must parse UA multiline");
        assert_eq!(l.voices, "10 / 500");
        assert_eq!(l.gifs, "3 / 10");
    }

    #[test]
    fn parse_limits_multiline_fails_missing_voices_counter() {
        let text =
            "🔓 Открытые голоса:\n       символов;\n🪩 Кружки/гифки:\n       0 / 10 сообщений;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_multiline_fails_missing_gifs_counter() {
        let text = "🔓 Открытые голоса:\n       34 / 666 символов;\n\n🪩 Кружки/гифки:\n       повідомлень;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_multiline_fails_arbitrary_slash_text() {
        let text = "some random / text\nand another line with / numbers";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_multiline_ignores_perevozvuchka() {
        let text = "Открытые голоса:\n       10 / 100 символов;\nКружки/гифки:\n       2 / 10 сообщений;\n🎙 Переозвучка:\n       5 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must parse only voices and gifs");
        assert_eq!(l.voices, "10 / 100");
        assert_eq!(l.gifs, "2 / 10");
    }

    #[test]
    fn parse_limits_multiline_ignores_only_text_section() {
        let text = "Открытые голоса:\n       10 / 100 символов;\nКружки/гифки:\n       2 / 10 сообщений;\n🎙 Только текст:\n       3 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must ignore only-text section");
        assert_eq!(l.voices, "10 / 100");
        assert_eq!(l.gifs, "2 / 10");
    }

    #[test]
    fn parse_limits_multiline_counter_non_numeric_rejected() {
        let text =
            "Открытые голоса:\n       abc / 666 символов;\nКружки/гифки:\n       0 / 10 сообщений;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_multiline_total_non_numeric_rejected() {
        let text =
            "Открытые голоса:\n       34 / xyz символов;\nКружки/гифки:\n       0 / 10 сообщений;";
        assert!(parse_limits_info(text).is_none());
    }

    #[test]
    fn parse_limits_multiline_fails_interleaved_slash_without_header() {
        let text = "🔓 Открытые голоса:\n\n       unrelated line\n\n       34 / 666 символов;\n\n🪩 Кружки/гифки:\n       0 / 10 сообщений;";
        assert!(
            parse_limits_info(text).is_none(),
            "counter not on immediate next non-empty line after header must fail"
        );
    }

    // ── Full-year reset timestamp validation ───────────────────────────

    #[test]
    fn parse_limits_reset_full_year_ua() {
        let text = "Відкриті голоси: 5 / 333 символів; оновлюється 2026-12-31 23:59:59 UTC-5\nКружки/гіфки: 2 / 10 повідомлень;";
        let l = parse_limits_info(text).expect("must parse full-year UA");
        assert_eq!(l.voices, "5 / 333");
        assert_eq!(l.gifs, "2 / 10");
        assert_eq!(
            l.reset_timestamp.as_deref(),
            Some("2026-12-31 23:59:59 UTC-5")
        );
    }

    #[test]
    fn parse_limits_reset_full_year_invalid_year_before_2000() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 1999-07-27 00:47:27 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_reset_full_year_invalid_year_after_2099() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 2100-07-27 00:47:27 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_reset_full_year_impossible_month() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 2026-13-01 00:00:00 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_reset_full_year_trailing_junk() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 2026-07-27 00:47:27 UTC+3extra\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_reset_full_year_garbage_after_keyword() {
        let text =
            "Открытые голоса: 0 / 666 символов; обновится garbage2026\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("must still parse counters");
        assert!(l.reset_timestamp.is_none());
    }

    #[test]
    fn parse_limits_reset_legacy_still_works_alongside_full_year() {
        let text = "Открытые голоса: 0 / 666 символов; обновится 2026-07-27 00:47:27 UTC+3\nКружки/гифки: 0 / 10 сообщений;";
        let l = parse_limits_info(text).expect("legacy counter must still parse");
        assert_eq!(l.voices, "0 / 666");
        assert_eq!(l.gifs, "0 / 10");
        assert_eq!(
            l.reset_timestamp.as_deref(),
            Some("2026-07-27 00:47:27 UTC+3")
        );
    }

    // ── parse_set_speaker_response ────────────────────────────────────

    #[test]
    fn parse_set_speaker_success_ru() {
        assert_eq!(
            parse_set_speaker_response("Успешно выбран спикер"),
            Ok(true)
        );
    }

    #[test]
    fn parse_set_speaker_success_ua() {
        assert_eq!(
            parse_set_speaker_response("Успішно обрано спікера"),
            Ok(true)
        );
    }

    #[test]
    fn parse_set_speaker_success_en() {
        assert_eq!(
            parse_set_speaker_response("Successfully selected speaker"),
            Ok(true)
        );
    }

    #[test]
    fn parse_set_speaker_success_same_ru() {
        assert_eq!(
            parse_set_speaker_response("Успешно выбран тот же самый спикер"),
            Ok(true)
        );
    }

    #[test]
    fn parse_set_speaker_success_same_ua() {
        assert_eq!(
            parse_set_speaker_response("Успішно обрано того самого спікера"),
            Ok(true)
        );
    }

    #[test]
    fn parse_set_speaker_invalid_ru() {
        assert_eq!(
            parse_set_speaker_response("Указан неверный голос"),
            Err("Invalid voice code".to_string())
        );
    }

    #[test]
    fn parse_set_speaker_invalid_ua() {
        assert_eq!(
            parse_set_speaker_response("Вказано невірний голос"),
            Err("Invalid voice code".to_string())
        );
    }

    #[test]
    fn parse_set_speaker_unknown_text() {
        assert_eq!(
            parse_set_speaker_response("random garbage"),
            Err("Unknown response format".to_string())
        );
    }

    #[test]
    fn parse_set_speaker_near_miss() {
        // substring but not exact match => unknown format
        assert_eq!(
            parse_set_speaker_response("Успешно выбран"),
            Err("Unknown response format".to_string())
        );
        assert_eq!(
            parse_set_speaker_response("Указан неверный"),
            Err("Unknown response format".to_string())
        );
    }

    // ── parse_message_text_with_validation ────────────────────────────

    #[test]
    fn validation_success_phrases_return_true() {
        assert!(parse_message_text_with_validation("Успешно выбран спикер"));
        assert!(parse_message_text_with_validation("Успішно обрано спікера"));
        assert!(parse_message_text_with_validation(
            "Successfully selected speaker"
        ));
        assert!(parse_message_text_with_validation(
            "Успешно выбран тот же самый спикер"
        ));
        assert!(parse_message_text_with_validation(
            "Успішно обрано того самого спікера"
        ));
    }

    #[test]
    fn validation_invalid_voice_returns_false() {
        assert!(!parse_message_text_with_validation("Указан неверный голос"));
        assert!(!parse_message_text_with_validation(
            "Вказано невірний голос"
        ));
    }

    #[test]
    fn validation_unknown_and_near_miss_returns_false() {
        // production contract: anything not matching a known success phrase
        // returns false (which causes the caller to keep waiting or fall
        // through to error).
        assert!(!parse_message_text_with_validation("random"));
        assert!(!parse_message_text_with_validation("Успешно выбран")); // near-miss
        assert!(!parse_message_text_with_validation("Указан неверный")); // near-miss
        assert!(!parse_message_text_with_validation(""));
    }

    // ── is_matching_audio_response ────────────────────────────────────

    const BOT_ID: i64 = 555000111;
    const SENT_ID: i32 = 42;

    fn match_audio(
        msg_out: bool,
        msg_id: i32,
        peer: Option<i64>,
        reply_to: Option<i32>,
        mime: &str,
    ) -> bool {
        is_matching_audio_response(msg_out, msg_id, peer, reply_to, mime, SENT_ID, BOT_ID)
    }

    #[test]
    fn matching_rejects_outgoing() {
        // outgoing even with valid peer/newer/matching reply
        assert!(!match_audio(
            true,
            43,
            Some(BOT_ID),
            Some(SENT_ID),
            "audio/ogg"
        ));
    }

    #[test]
    fn matching_rejects_wrong_peer() {
        // incoming but wrong peer
        assert!(!match_audio(
            false,
            43,
            Some(999999),
            Some(SENT_ID),
            "audio/ogg"
        ));
        // no peer_user_id at all
        assert!(!match_audio(false, 43, None, Some(SENT_ID), "audio/ogg"));
    }

    #[test]
    fn matching_rejects_stale_msg_id() {
        // same msg_id as sent
        assert!(!match_audio(
            false,
            SENT_ID,
            Some(BOT_ID),
            Some(SENT_ID),
            "audio/ogg"
        ));
        // older than sent
        assert!(!match_audio(
            false,
            SENT_ID - 1,
            Some(BOT_ID),
            Some(SENT_ID),
            "audio/ogg"
        ));
    }

    #[test]
    fn matching_rejects_explicit_reply_to_other_message() {
        // reply_to exists but != sent_msg_id
        assert!(!match_audio(false, 43, Some(BOT_ID), Some(99), "audio/ogg"));
    }

    #[test]
    fn matching_rejects_non_audio_mime() {
        assert!(!match_audio(
            false,
            43,
            Some(BOT_ID),
            Some(SENT_ID),
            "audio/mp4"
        ));
        assert!(!match_audio(
            false,
            43,
            Some(BOT_ID),
            Some(SENT_ID),
            "text/plain"
        ));
        assert!(!match_audio(false, 43, Some(BOT_ID), Some(SENT_ID), ""));
    }

    #[test]
    fn matching_accepts_valid_reply() {
        // incoming, correct peer, newer, matching reply, audio mime
        assert!(match_audio(
            false,
            43,
            Some(BOT_ID),
            Some(SENT_ID),
            "audio/ogg"
        ));
        assert!(match_audio(
            false,
            100,
            Some(BOT_ID),
            Some(SENT_ID),
            "audio/mpeg"
        ));
    }

    #[test]
    fn matching_rejects_replyless_newer_audio() {
        assert!(!match_audio(false, 43, Some(BOT_ID), None, "audio/ogg"));
        assert!(!match_audio(false, 100, Some(BOT_ID), None, "audio/mpeg"));
    }

    // ── is_matching_text_reply ─────────────────────────────────────────

    const ALT_BOT_ID: i64 = 666000777;

    fn match_text(msg_out: bool, msg_id: i32, user_id: i64, reply_to: Option<i32>) -> bool {
        is_matching_text_reply(msg_out, msg_id, user_id, reply_to, SENT_ID, BOT_ID)
    }

    #[test]
    fn text_reply_rejects_outgoing() {
        assert!(!match_text(true, 43, BOT_ID, Some(SENT_ID)));
    }

    #[test]
    fn text_reply_rejects_wrong_peer() {
        assert!(!match_text(false, 43, ALT_BOT_ID, Some(SENT_ID)));
    }

    #[test]
    fn text_reply_rejects_stale_id() {
        assert!(!match_text(false, SENT_ID, BOT_ID, Some(SENT_ID)));
        assert!(!match_text(false, SENT_ID - 1, BOT_ID, Some(SENT_ID)));
    }

    #[test]
    fn text_reply_rejects_wrong_reply_to() {
        assert!(!match_text(false, 43, BOT_ID, Some(99)));
    }

    #[test]
    fn text_reply_rejects_missing_reply_to() {
        assert!(!match_text(false, 43, BOT_ID, None));
    }

    #[test]
    fn text_reply_accepts_valid() {
        assert!(match_text(false, 43, BOT_ID, Some(SENT_ID)));
        assert!(match_text(false, 100, BOT_ID, Some(SENT_ID)));
    }

    // ── is_matching_limits_response ────────────────────────────────────

    #[test]
    fn limits_matching_correct_candidate() {
        assert!(is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_rejects_outgoing() {
        assert!(!is_matching_limits_response(
            true,
            43,
            Some(BOT_ID),
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_rejects_wrong_peer() {
        assert!(!is_matching_limits_response(
            false,
            43,
            Some(999999),
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
        assert!(!is_matching_limits_response(
            false,
            43,
            None,
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_rejects_stale_msg_id() {
        assert!(!is_matching_limits_response(
            false,
            SENT_ID,
            Some(BOT_ID),
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
        assert!(!is_matching_limits_response(
            false,
            SENT_ID - 1,
            Some(BOT_ID),
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_rejects_media() {
        assert!(!is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            true,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_accepts_with_reply_markup() {
        assert!(is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_rejects_malformed_text() {
        assert!(!is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            false,
            "random text",
            SENT_ID,
            BOT_ID
        ));
        assert!(!is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            false,
            "",
            SENT_ID,
            BOT_ID
        ));
        assert!(!is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            false,
            "Открытые голоса: 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    // ── set_speaker cache / serialization tests ──────────────────────────

    #[test]
    fn set_speaker_short_circuits_when_cache_matches() {
        let client = TelegramClient::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let mut cache = client.confirmed_speaker.lock().await;
            *cache = Some("baya_16".to_string());
            drop(cache);

            let result = set_speaker(&client, "baya_16").await;
            assert!(result.is_ok(), "must skip /speaker when cache matches");
            assert!(result.unwrap());
        });
    }

    #[test]
    fn set_speaker_proceeds_when_cache_differs() {
        let client = TelegramClient::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            {
                let mut cache = client.confirmed_speaker.lock().await;
                *cache = Some("old-speaker".to_string());
            }

            let result = set_speaker(&client, "new-speaker").await;
            assert!(
                result.is_err(),
                "must proceed to subscribe when cache differs"
            );
        });
    }

    #[test]
    fn set_speaker_proceeds_when_cache_is_none() {
        let client = TelegramClient::new();
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let result = set_speaker(&client, "any-speaker").await;
            assert!(result.is_err(), "must proceed when cache is None");
        });
    }

    #[test]
    fn set_speaker_serializer_blocks_concurrent_calls() {
        let client = TelegramClient::new();
        let clone = client.clone();
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            {
                let mut cache = client.confirmed_speaker.lock().await;
                *cache = Some("shared-cached".to_string());
                drop(cache);
            }

            let _hold = client.speaker_serializer.lock().await;

            let task_handle =
                tokio::spawn(async move { set_speaker(&clone, "shared-cached").await });

            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            assert!(
                !task_handle.is_finished(),
                "task must be blocked on serializer"
            );

            drop(_hold);

            let result = task_handle.await.unwrap();
            assert!(
                result.is_ok(),
                "cache hit succeeds after serializer released"
            );
        });
    }

    #[test]
    fn set_speaker_double_check_sees_concurrent_update() {
        let client = TelegramClient::new();
        let clone = client.clone();
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async {
            let hold = client.speaker_serializer.lock().await;

            let task_handle =
                tokio::spawn(async move { set_speaker(&clone, "concurrent-speaker").await });

            tokio::task::yield_now().await;
            tokio::task::yield_now().await;

            {
                let mut cache = client.confirmed_speaker.lock().await;
                *cache = Some("concurrent-speaker".to_string());
            }

            drop(hold);

            let result = task_handle.await.unwrap();
            assert!(
                result.is_ok(),
                "double-check after serializer must see the cache update"
            );
        });
    }

    // ── should_log_bot_text ──────────────────────────────────────────

    #[test]
    fn should_log_bot_text_enables_debug_exact_1() {
        assert!(should_log_bot_text(false, Some(42), 42, Some("1")));
    }

    #[test]
    fn should_log_bot_text_disabled_unset() {
        assert!(!should_log_bot_text(false, Some(42), 42, None));
    }

    #[test]
    fn should_log_bot_text_disabled_empty_string() {
        assert!(!should_log_bot_text(false, Some(42), 42, Some("")));
    }

    #[test]
    fn should_log_bot_text_disabled_wrong_value() {
        assert!(!should_log_bot_text(false, Some(42), 42, Some("0")));
        assert!(!should_log_bot_text(false, Some(42), 42, Some("true")));
    }

    #[test]
    fn should_log_bot_text_disabled_outgoing() {
        assert!(!should_log_bot_text(true, Some(42), 42, Some("1")));
    }

    #[test]
    fn should_log_bot_text_disabled_wrong_peer() {
        assert!(!should_log_bot_text(false, Some(99), 42, Some("1")));
    }

    #[test]
    fn should_log_bot_text_disabled_no_peer() {
        assert!(!should_log_bot_text(false, None, 42, Some("1")));
    }

    // ── extract_limits_info_from_update (short message) ─────────────────

    const MULTILINE_PAYLOAD: &str = "🔓 Открытые голоса:\n       34 / 666 символов;\n       Обновится 2026-07-27 00:47:27 UTC+3.\n\n🪩 Кружки/гифки:\n       0 / 10 сообщений;\n\n🎙 Переозвучка:\n       0 / 10 сообщений;";

    fn make_short_update(out: bool, id: i32, user_id: i64, message: &str) -> UpdatesLike {
        UpdatesLike::Updates(grammers_tl_types::enums::Updates::UpdateShortMessage(
            grammers_tl_types::types::UpdateShortMessage {
                out,
                mentioned: false,
                media_unread: false,
                silent: false,
                id,
                user_id,
                message: message.to_string(),
                pts: 0,
                pts_count: 0,
                date: 0,
                fwd_from: None,
                via_bot_id: None,
                reply_to: None,
                entities: None,
                ttl_period: None,
            },
        ))
    }

    #[test]
    fn limits_short_message_accepted() {
        let update = make_short_update(false, 43, BOT_ID, MULTILINE_PAYLOAD);
        let result = extract_limits_info_from_update(&update, SENT_ID, BOT_ID);
        let l = result.expect("must parse limits from short message");
        assert_eq!(l.voices, "34 / 666");
        assert_eq!(l.gifs, "0 / 10");
    }

    #[test]
    fn limits_short_message_rejects_outgoing() {
        let update = make_short_update(true, 43, BOT_ID, MULTILINE_PAYLOAD);
        assert!(extract_limits_info_from_update(&update, SENT_ID, BOT_ID).is_none());
    }

    #[test]
    fn limits_short_message_rejects_wrong_user() {
        let update = make_short_update(false, 43, 999999, MULTILINE_PAYLOAD);
        assert!(extract_limits_info_from_update(&update, SENT_ID, BOT_ID).is_none());
    }

    #[test]
    fn limits_short_message_rejects_stale_id() {
        let update = make_short_update(false, SENT_ID, BOT_ID, MULTILINE_PAYLOAD);
        assert!(extract_limits_info_from_update(&update, SENT_ID, BOT_ID).is_none());
    }

    #[test]
    fn limits_short_message_rejects_malformed_text() {
        let update = make_short_update(false, 43, BOT_ID, "random text");
        assert!(extract_limits_info_from_update(&update, SENT_ID, BOT_ID).is_none());
    }

    // ── voice_extension ────────────────────────────────────────────────

    #[test]
    fn voice_extension_mp3() {
        assert_eq!(voice_extension("audio/mpeg"), "mp3");
    }

    #[test]
    fn voice_extension_ogg_default() {
        assert_eq!(voice_extension("audio/ogg"), "ogg");
        assert_eq!(voice_extension("audio/opus"), "ogg");
        assert_eq!(voice_extension(""), "ogg");
    }

    // ── voice_final_path / voice_part_path ─────────────────────────────

    #[test]
    fn voice_paths_mp3() {
        let dir = PathBuf::from("/tmp/test_tts");
        assert_eq!(
            voice_final_path(&dir, 42, "mp3"),
            PathBuf::from("/tmp/test_tts/silero_tts_42.mp3")
        );
        assert_eq!(
            voice_part_path(&dir, 42, "mp3"),
            PathBuf::from("/tmp/test_tts/silero_tts_42.mp3.part")
        );
    }

    #[test]
    fn voice_paths_ogg() {
        let dir = PathBuf::from("/tmp/test_tts");
        assert_eq!(
            voice_final_path(&dir, 123, "ogg"),
            PathBuf::from("/tmp/test_tts/silero_tts_123.ogg")
        );
        assert_eq!(
            voice_part_path(&dir, 123, "ogg"),
            PathBuf::from("/tmp/test_tts/silero_tts_123.ogg.part")
        );
    }

    #[test]
    fn voice_paths_different_msg_ids() {
        let dir = PathBuf::from("/tmp/test_tts");
        let f1 = voice_final_path(&dir, 1, "mp3");
        let f2 = voice_final_path(&dir, 2, "mp3");
        assert_ne!(f1, f2);
        let p1 = voice_part_path(&dir, 1, "mp3");
        let p2 = voice_part_path(&dir, 2, "mp3");
        assert_ne!(p1, p2);
    }

    // ── publish_voice_file ─────────────────────────────────────────────

    /// Helper: creates a per-test temp directory, returns it, and schedules
    /// best-effort cleanup when the returned `DirGuard` is dropped.
    struct DirGuard(PathBuf);

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn test_dir(label: &str) -> DirGuard {
        let d =
            std::env::temp_dir().join(format!("test_tts_publish_{}_{}", std::process::id(), label));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        DirGuard(d)
    }

    #[test]
    fn publish_non_empty_success() {
        let guard = test_dir("non_empty");
        let part = voice_part_path(&guard.0, 10, "mp3");
        let final_path = voice_final_path(&guard.0, 10, "mp3");

        std::fs::write(&part, b"valid audio data").unwrap();

        let (published, size) =
            publish_voice_file(&part, &final_path).expect("publication must succeed");
        assert!(!part.exists(), "part file must be removed after rename");
        assert!(final_path.exists(), "final file must exist");
        assert_eq!(published, final_path);
        assert_eq!(size, 16);
    }

    #[test]
    fn publish_zero_byte_rejected_and_cleaned() {
        let guard = test_dir("zero_byte");
        let part = voice_part_path(&guard.0, 20, "ogg");
        let final_path = voice_final_path(&guard.0, 20, "ogg");

        std::fs::write(&part, b"").unwrap();

        let err =
            publish_voice_file(&part, &final_path).expect_err("zero-byte file must be rejected");
        assert!(err.contains("empty"), "error must mention empty: {}", err);
        assert!(
            !part.exists(),
            "part file must be cleaned up after zero-byte rejection"
        );
        assert!(!final_path.exists(), "final file must not be created");
    }

    #[test]
    fn publish_missing_part_returns_error() {
        let guard = test_dir("missing_part");
        let part = voice_part_path(&guard.0, 30, "mp3");
        let final_path = voice_final_path(&guard.0, 30, "mp3");

        let err = publish_voice_file(&part, &final_path)
            .expect_err("missing part file must return error");
        assert!(
            err.contains("metadata"),
            "error must mention metadata: {}",
            err
        );
        assert!(
            !part.exists(),
            "no part file must remain after metadata failure"
        );
        assert!(!final_path.exists());
    }

    #[test]
    fn publish_rename_to_existing_behaviour() {
        let guard = test_dir("rename_existing");
        let part = voice_part_path(&guard.0, 40, "mp3");
        let final_path = voice_final_path(&guard.0, 40, "mp3");

        std::fs::write(&part, b"new data").unwrap();
        std::fs::write(&final_path, b"old data").unwrap();

        let result = publish_voice_file(&part, &final_path);
        match result {
            Ok((_path, _size)) => {
                // On Unix rename overwrites; verify the part is gone and
                // final contains the new content.
                assert!(!part.exists());
                let content = std::fs::read_to_string(&final_path).unwrap();
                assert_eq!(content, "new data");
            }
            Err(e) => {
                // On Windows rename fails if destination exists.
                assert!(e.contains("rename"), "error must mention rename: {}", e);
                assert!(!part.exists(), "part must be cleaned up on failure");
            }
        }
    }
}
