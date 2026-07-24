use super::client::TelegramClient;
use super::types::{CurrentVoice, Limits, TtsResult};
use grammers_session::updates::UpdatesLike;
use std::path::PathBuf;
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
                            if let Some(media) = &m.media {
                                if let grammers_tl_types::enums::MessageMedia::Document(doc_media) =
                                    media
                                {
                                    if let Some(grammers_tl_types::enums::Document::Document(doc)) =
                                        &doc_media.document
                                    {
                                        let peer_user_id = match &m.peer_id {
                                            grammers_tl_types::enums::Peer::User(u) => {
                                                Some(u.user_id)
                                            }
                                            _ => None,
                                        };
                                        let reply_to_msg_id = match &m.reply_to {
                                            Some(
                                                grammers_tl_types::enums::MessageReplyHeader::Header(h),
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
                            // Создаем временную папку
                            let temp_dir = Self::get_temp_dir()?;
                            std::fs::create_dir_all(&temp_dir)
                                .map_err(|e| format!("Failed to create temp dir: {}", e))?;

                            // Определяем расширение файла
                            let extension = if voice.mime_type == "audio/mpeg" {
                                "mp3"
                            } else {
                                "ogg"
                            };

                            // Генерируем уникальное имя файла
                            let timestamp = chrono::Utc::now().timestamp();
                            let file_name = format!("silero_tts_{}.{}", timestamp, extension);
                            let dest_path = temp_dir.join(&file_name);

                            info!(?dest_path, "Downloading voice file");

                            // Скачиваем медиа
                            client_inner
                                .download_media(&media, &dest_path)
                                .await
                                .map_err(|e| format!("Download failed: {}", e))?;

                            info!(?dest_path, "Download completed");

                            return Ok(dest_path
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
/// Checks direction, peer, message ID ordering, absence of media and inline menu,
/// and that text parses as limits.
fn is_matching_limits_response(
    msg_out: bool,
    msg_id: i32,
    peer_user_id: Option<i64>,
    has_media: bool,
    has_reply_markup: bool,
    text: &str,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> bool {
    if msg_out {
        return false;
    }
    if peer_user_id != Some(bot_user_id) {
        return false;
    }
    if msg_id <= sent_msg_id {
        return false;
    }
    if has_media || has_reply_markup {
        return false;
    }
    if text.is_empty() {
        return false;
    }
    parse_limits_info(text).is_some()
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
/// Delegates policy to `is_matching_limits_response` to avoid duplicating
/// matching rules.
fn extract_limits_info_from_update(
    update_like: &UpdatesLike,
    sent_msg_id: i32,
    bot_user_id: i64,
) -> Option<Limits> {
    if let UpdatesLike::Updates(updates_enum) = update_like {
        if let grammers_tl_types::enums::Updates::Updates(u) = updates_enum {
            for update in &u.updates {
                if let grammers_tl_types::enums::Update::NewMessage(msg) = update {
                    if let grammers_tl_types::enums::Message::Message(m) = &msg.message {
                        let peer_user_id = match &m.peer_id {
                            grammers_tl_types::enums::Peer::User(u) => Some(u.user_id),
                            _ => None,
                        };
                        if is_matching_limits_response(
                            m.out,
                            m.id,
                            peer_user_id,
                            m.media.is_some(),
                            m.reply_markup.is_some(),
                            &m.message,
                            sent_msg_id,
                            bot_user_id,
                        ) {
                            // parse_limits_info will succeed since is_matching_limits_response validated it
                            return parse_limits_info(&m.message);
                        }
                    }
                }
            }
        }
    }
    None
}

/// Парсит текст ответа бота для получения информации о лимитах
/// Формат: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
fn parse_limits_info(text: &str) -> Option<Limits> {
    trace!("Parsing limits info");

    let mut voices: Option<String> = None;
    let mut gifs: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();

        // Парсим "🔓 Открытые голоса: 0 / 666 символов;"
        if line.contains("Открытые голоса:") || line.contains("Відкриті голоси:")
        {
            // Извлекаем часть "0 / 666"
            if let Some(colon_pos) = line.find(':') {
                let after_colon = line[colon_pos + 1..].trim();
                // Ищем шаблон "число / число"
                if let Some(slash_pos) = after_colon.find('/') {
                    let before_slash = after_colon[..slash_pos].trim();
                    let after_slash = after_colon[slash_pos + 1..].trim();
                    // Извлекаем числа
                    if let Some(space_pos) = after_slash.find_whitespace() {
                        let limit_num = after_slash[..space_pos].trim();
                        voices = Some(format!("{} / {}", before_slash, limit_num));
                    } else {
                        // Если нет пробела, берем всё до конца
                        voices = Some(format!("{} / {}", before_slash, after_slash.trim()));
                    }
                }
            }
        }

        // Парсим "🪩 Кружки/гифки: 0 / 10 сообщений;"
        if line.contains("Кружки/гифки:")
            || line.contains("Кружки/гіфки:")
            || line.contains("Гифки:")
        {
            // Извлекаем часть "0 / 10"
            if let Some(colon_pos) = line.find(':') {
                let after_colon = line[colon_pos + 1..].trim();
                // Ищем шаблон "число / число"
                if let Some(slash_pos) = after_colon.find('/') {
                    let before_slash = after_colon[..slash_pos].trim();
                    let after_slash = after_colon[slash_pos + 1..].trim();
                    // Извлекаем числа
                    if let Some(space_pos) = after_slash.find_whitespace() {
                        let limit_num = after_slash[..space_pos].trim();
                        gifs = Some(format!("{} / {}", before_slash, limit_num));
                    } else {
                        // Если нет пробела, берем всё до конца
                        gifs = Some(format!("{} / {}", before_slash, after_slash.trim()));
                    }
                }
            }
        }
    }

    // Возвращаем результат если нашли оба поля
    if let (Some(voices_val), Some(gifs_val)) = (voices, gifs) {
        trace!(voices_val, gifs_val, "Parsed limits info");
        Some(Limits {
            voices: voices_val,
            gifs: gifs_val,
        })
    } else {
        warn!("Failed to parse limits info from text");
        None
    }
}

/// Трейт для поиска первого пробела в строке
trait FindWhitespace {
    fn find_whitespace(&self) -> Option<usize>;
}

impl FindWhitespace for &str {
    fn find_whitespace(&self) -> Option<usize> {
        self.chars().position(|c| c.is_whitespace())
    }
}

/// Отправить "/speaker {code}" боту и дождаться текстового ответа
/// Возвращает true если успешно, иначе ошибку
pub async fn set_speaker(client: &TelegramClient, voice_code: &str) -> Result<bool, String> {
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
                        return Ok(true);
                    } else {
                        return Err("Invalid voice code".to_string());
                    }
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
            false,
            "Открытые голоса: 0 / 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }

    #[test]
    fn limits_matching_rejects_reply_markup() {
        assert!(!is_matching_limits_response(
            false,
            43,
            Some(BOT_ID),
            false,
            true,
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
            false,
            "Открытые голоса: 666 символов;\nКружки/гифки: 0 / 10 сообщений;",
            SENT_ID,
            BOT_ID
        ));
    }
}
