use super::client::TelegramClient;
use super::types::{TtsResult, CurrentVoice, Limits};
use grammers_session::updates::UpdatesLike;
use std::path::PathBuf;
use tracing::{info, error, debug, warn, trace};

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
        Self {
            _client: None,
        }
    }

    /// Синтез речи через Telegram бота
    /// Возвращает путь к скачанному аудиофайлу
    pub async fn synthesize(
        client: &TelegramClient,
        text: &str,
    ) -> Result<TtsResult, String> {
        info!(text, "Starting TTS synthesis");

        // Валидация входного текста
        let text = text.trim();
        if text.is_empty() {
            return Ok(TtsResult::error("Text cannot be empty".to_string()));
        }

        if text.len() > 4000 {
            return Ok(TtsResult::error(
                "Text too long (max 4000 characters)".to_string(),
            ));
        }

        // 1. Отправляем текст боту
        Self::send_text_to_bot(client, text).await?;

        // 2. Ждем голосовое сообщение от бота
        let voice_result = Self::wait_for_voice_message(client, 30).await?;

        // 3. Скачиваем аудиофайл во временную папку
        let audio_path = Self::download_voice_to_temp(client, &voice_result).await?;

        info!(?audio_path, "TTS synthesis completed");

        Ok(TtsResult::success(audio_path))
    }

    /// Отправить текст боту
    async fn send_text_to_bot(client: &TelegramClient, text: &str) -> Result<(), String> {
        info!(text, "Sending text to bot");

        let client_inner = client.client.lock().await;
        let client_inner = client_inner
            .as_ref()
            .ok_or_else(|| "Client not initialized".to_string())?;

        // Разрешаем username бота
        let bot = client_inner
            .resolve_username(BOT_USERNAME)
            .await
            .map_err(|e| format!("Failed to resolve bot: {}", e))?
            .ok_or_else(|| "Bot not found".to_string())?;

        debug!(username = ?bot.username(), "Bot resolved");

        // Отправляем сообщение - используем bot.to_ref() для получения PeerRef
        let bot_ref = bot.to_ref().await
            .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
        let result = client_inner
            .send_message(bot_ref, text)
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        trace!(?result, "Message sent");

        Ok(())
    }

    /// Ожидать голосовое сообщение от бота с таймаутом
    async fn wait_for_voice_message(
        client: &TelegramClient,
        timeout_secs: u64,
    ) -> Result<VoiceMessageResult, String> {
        use tokio::sync::mpsc::UnboundedReceiver;

        info!(timeout_secs, "Waiting for voice message");

        let start_time = std::time::Instant::now();
        let total_timeout = std::time::Duration::from_secs(timeout_secs);

        loop {
            let mut updates_opt = client.updates.lock().await;
            let updates: &mut UnboundedReceiver<UpdatesLike> = updates_opt
                .as_mut()
                .ok_or_else(|| "Updates channel not initialized".to_string())?;

            // Проверяем общий таймаут
            let elapsed = start_time.elapsed();
            if elapsed >= total_timeout {
                warn!("Timeout waiting for voice message");
                return Err("Timeout waiting for voice message".to_string());
            }

            let remaining = total_timeout.saturating_sub(elapsed);

            match tokio::time::timeout(remaining, updates.recv()).await {
                Ok(Some(update_like)) => {
                    if let Some(result) = Self::extract_voice_from_update(&update_like) {
                        debug!(
                            "[SILORO] Voice message found: file_id={}, msg_id={}, mime={}",
                            result.file_id, result.msg_id, result.mime_type
                        );
                        return Ok(result);
                    }
                }
                Ok(None) => {
                    warn!("Updates channel closed");
                    return Err("Updates channel closed".to_string());
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
    fn extract_voice_from_update(update_like: &UpdatesLike) -> Option<VoiceMessageResult> {
        if let UpdatesLike::Updates(updates_enum) = update_like {
            if let grammers_tl_types::enums::Updates::Updates(u) = updates_enum {
                for update in &u.updates {
                    if let grammers_tl_types::enums::Update::NewMessage(msg) = update {
                        if let grammers_tl_types::enums::Message::Message(m) = &msg.message {
                            // Проверяем, что это сообщение от бота и содержит голосовое
                            if let Some(media) = &m.media {
                                if let grammers_tl_types::enums::MessageMedia::Document(
                                    doc_media,
                                ) = media
                                {
                                    if let Some(grammers_tl_types::enums::Document::Document(
                                        doc,
                                    )) = &doc_media.document
                                    {
                                        let mime = &doc.mime_type;
                                        // Принимаем и OGG и MP3
                                        if mime == "audio/ogg" || mime == "audio/mpeg" {
                                            return Some(VoiceMessageResult {
                                                file_id: doc.id.to_string(),
                                                msg_id: m.id,
                                                mime_type: mime.clone(),
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

        let client_inner = client.client.lock().await;
        let client_inner = client_inner
            .as_ref()
            .ok_or_else(|| "Client not initialized".to_string())?;

        // Разрешаем бота
        let bot = client_inner
            .resolve_username(BOT_USERNAME)
            .await
            .map_err(|e| format!("Failed to resolve bot: {}", e))?
            .ok_or_else(|| "Bot not found".to_string())?;

        // Находим сообщение с нужным file_id
        let bot_ref = bot.to_ref().await
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
            let appdata = std::env::var("APPDATA")
                .map_err(|e| format!("Failed to get APPDATA: {}", e))?;
            PathBuf::from(appdata).join("ttsbard").join("temp")
        } else if cfg!(target_os = "macos") {
            let home = std::env::var("HOME")
                .map_err(|e| format!("Failed to get HOME: {}", e))?;
            PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("ttsbard")
                .join("temp")
        } else {
            // Linux
            let home = std::env::var("HOME")
                .map_err(|e| format!("Failed to get HOME: {}", e))?;
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

impl Default for SileroTtsBot {
    fn default() -> Self {
        Self::new()
    }
}

/// Отправить /speaker и дождаться текстового ответа с текущим голосом
/// Парсит: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
/// Таймаут 1 минута на ожидание ответа
pub async fn get_current_voice(client: &TelegramClient) -> Result<Option<CurrentVoice>, String> {
    use tokio::sync::mpsc::UnboundedReceiver;

    info!("Getting current voice from bot");

    // 1. Отправляем /speaker и получаем ID сообщения
    let sent_message_id = send_speaker_command(client).await?;

    info!(sent_message_id, "/speaker sent, waiting for text response");

    // 2. Ждем текстовое сообщение (ответ на наше сообщение)
    let start_time = std::time::Instant::now();
    let total_timeout = std::time::Duration::from_secs(60);  // 1 минута

    loop {
        let mut updates_opt = client.updates.lock().await;
        let updates: &mut UnboundedReceiver<UpdatesLike> = updates_opt
            .as_mut()
            .ok_or_else(|| "Updates channel not initialized".to_string())?;

        // Проверяем общий таймаут
        let elapsed = start_time.elapsed();
        if elapsed >= total_timeout {
            warn!("Timeout (60s) waiting for voice info");
            return Ok(None);  // Таймаут - возвращаем None
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, updates.recv()).await {
            Ok(Some(update_like)) => {
                // Логируем все incoming updates для отладки
                trace!(?update_like, "Received update");

                // Проверяем, есть ли текстовое сообщение с информацией о голосе
                // Передаем expected_msg_id чтобы проверить что это ответ на наше сообщение
                if let Some(voice_info) = extract_voice_info_from_update(&update_like, sent_message_id) {
                    info!(voice_info.name, voice_info.id, "Voice info found");
                    return Ok(Some(voice_info));  // Нашли - возвращаем Some
                }
            }
            Ok(None) => {
                warn!("Updates channel closed");
                return Err("Updates channel closed".to_string());
            }
            Err(_) => {
                // Таймаут одной итерации - продолжаем ждать
                continue;
            }
        }
    }
}

/// Отправить команду /speaker боту
/// Возвращает ID отправленного сообщения
async fn send_speaker_command(client: &TelegramClient) -> Result<i32, String> {
    info!("Sending /speaker to bot");

    let client_inner = client.client.lock().await;
    let client_inner = client_inner
        .as_ref()
        .ok_or_else(|| "Client not initialized".to_string())?;

    // Разрешаем username бота
    let bot = client_inner
        .resolve_username(BOT_USERNAME)
        .await
        .map_err(|e| format!("Failed to resolve bot: {}", e))?
        .ok_or_else(|| "Bot not found".to_string())?;

    debug!(username = ?bot.username(), "Bot resolved");

    // Отправляем сообщение
    let bot_ref = bot.to_ref().await
        .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
    let result = client_inner
        .send_message(bot_ref, "/speaker")
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    let msg_id = result.id();
    trace!(msg_id, "Message sent");

    Ok(msg_id)
}

/// Извлечь информацию о текущем голосе из текстового сообщения
/// Парсит: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
/// Проверяет что сообщение является ответом на сообщение с expected_msg_id
#[allow(clippy::collapsible_match)]
fn extract_voice_info_from_update(update_like: &UpdatesLike, expected_msg_id: i32) -> Option<CurrentVoice> {
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

                        // Проверяем что это ответ на наше сообщение
                        match &m.reply_to {
                            Some(grammers_tl_types::enums::MessageReplyHeader::Header(h)) if h.reply_to_msg_id == Some(expected_msg_id) => {
                                // Это ответ на наше сообщение - обрабатываем
                            }
                            _ => {
                                // Не ответ на наше сообщение - пропускаем
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
                                trace!(text, "Attempting to parse voice info");
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
    trace!(text, "Parsing text");

    // Ищем строки с ключевыми словами
    let mut voice_id: Option<String> = None;
    let mut voice_name: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();

        // Парсим "Выбранный голос: /speaker hamster_clerk"
        if line.contains("Выбранный голос:") || line.contains("Выбраний голос:") {
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
        if line.contains("Находится в паке:") || line.contains("Знаходиться в паке:")
           || line.contains("находится в паке:") || line.contains("знаходиться в паке:") {
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
        trace!(id, name, "Parsed voice info");
        Some(CurrentVoice { name, id })
    } else {
        warn!("Failed to parse voice info from text");
        None
    }
}

/// Отправить /limits и дождаться текстового ответа с лимитами
/// Парсит: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
/// Таймаут 60 секунд на ожидание ответа
pub async fn get_limits(client: &TelegramClient) -> Result<Option<Limits>, String> {
    use tokio::sync::mpsc::UnboundedReceiver;

    info!("Getting limits from bot");

    // 1. Отправляем /limits
    send_limits_command(client).await?;

    info!("/limits sent, waiting for text response");

    // 2. Ждем текстовое сообщение (не меню, не голос)
    let start_time = std::time::Instant::now();
    let total_timeout = std::time::Duration::from_secs(60);  // 60 секунд

    loop {
        let mut updates_opt = client.updates.lock().await;
        let updates: &mut UnboundedReceiver<UpdatesLike> = updates_opt
            .as_mut()
            .ok_or_else(|| "Updates channel not initialized".to_string())?;

        // Проверяем общий таймаут
        let elapsed = start_time.elapsed();
        if elapsed >= total_timeout {
            warn!("Timeout (60s) waiting for limits info");
            return Ok(None);  // Таймаут - возвращаем None
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, updates.recv()).await {
            Ok(Some(update_like)) => {
                // Проверяем, есть ли текстовое сообщение с информацией о лимитах
                if let Some(limits_info) = extract_limits_info_from_update(&update_like) {
                    info!(limits_info.voices, limits_info.gifs, "Limits info found");
                    return Ok(Some(limits_info));  // Нашли - возвращаем Some
                }
            }
            Ok(None) => {
                warn!("Updates channel closed");
                return Err("Updates channel closed".to_string());
            }
            Err(_) => {
                // Таймаут одной итерации - продолжаем ждать
                continue;
            }
        }
    }
}

/// Отправить команду /limits боту
async fn send_limits_command(client: &TelegramClient) -> Result<(), String> {
    info!("Sending /limits to bot");

    let client_inner = client.client.lock().await;
    let client_inner = client_inner
        .as_ref()
        .ok_or_else(|| "Client not initialized".to_string())?;

    // Разрешаем username бота
    let bot = client_inner
        .resolve_username(BOT_USERNAME)
        .await
        .map_err(|e| format!("Failed to resolve bot: {}", e))?
        .ok_or_else(|| "Bot not found".to_string())?;

    debug!(username = ?bot.username(), "Bot resolved");

    // Отправляем сообщение
    let bot_ref = bot.to_ref().await
        .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
    let result = client_inner
        .send_message(bot_ref, "/limits")
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    trace!(?result, "Message sent");

    Ok(())
}

/// Извлечь информацию о лимитах из текстового сообщения
/// Парсит: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
#[allow(clippy::collapsible_match)]
fn extract_limits_info_from_update(update_like: &UpdatesLike) -> Option<Limits> {
    if let UpdatesLike::Updates(updates_enum) = update_like {
        if let grammers_tl_types::enums::Updates::Updates(u) = updates_enum {
            for update in &u.updates {
                if let grammers_tl_types::enums::Update::NewMessage(msg) = update {
                    if let grammers_tl_types::enums::Message::Message(m) = &msg.message {
                        // Ищем текстовое сообщение (без медиа, без меню)
                        if m.media.is_none() && m.reply_markup.is_none() {
                            // В TL типе Message текст находится в поле message
                            let text = &m.message;
                            if !text.is_empty() {
                                // Парсим текст
                                if let Some(limits) = parse_limits_info(text) {
                                    return Some(limits);
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

/// Парсит текст ответа бота для получения информации о лимитах
/// Формат: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
fn parse_limits_info(text: &str) -> Option<Limits> {
    trace!(text, "Parsing text");

    let mut voices: Option<String> = None;
    let mut gifs: Option<String> = None;

    for line in text.lines() {
        let line = line.trim();

        // Парсим "🔓 Открытые голоса: 0 / 666 символов;"
        if line.contains("Открытые голоса:") || line.contains("Відкриті голоси:") {
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
        if line.contains("Кружки/гифки:") || line.contains("Кружки/гіфки:") || line.contains("Гифки:") {
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
    use tokio::sync::mpsc::UnboundedReceiver;

    info!("Setting speaker to '{}'", voice_code);

    // 1. Сначала получаем receiver для updates, чтобы не пропустить ответ
    let mut updates_opt = client.updates.lock().await;
    let updates: &mut UnboundedReceiver<UpdatesLike> = updates_opt
        .as_mut()
        .ok_or_else(|| "Updates channel not initialized".to_string())?;

    trace!("Updates receiver locked, ready to send command");

    // 2. Отправить "/speaker {code}" и получить ID сообщения
    let sent_message_id = send_speaker_command_with_code(client, voice_code).await?;

    info!("Waiting for bot response to msg_id={}", sent_message_id);

    // 3. Ждем текстовое сообщение (ответ на наше сообщение)
    let start_time = std::time::Instant::now();
    let total_timeout = std::time::Duration::from_secs(30);  // 30 секунд

    loop {

        // Проверяем общий таймаут
        let elapsed = start_time.elapsed();
        if elapsed >= total_timeout {
            warn!("Timeout (30s) waiting for set_speaker response");
            return Err("Timeout waiting for speaker change response".to_string());
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, updates.recv()).await {
            Ok(Some(update_like)) => {
                trace!("Received update while waiting for set_speaker response");
                // Проверяем, есть ли текстовое сообщение с ответом на наше сообщение
                if let Some(result) = extract_set_speaker_response_from_update(&update_like, sent_message_id) {
                    info!("Set speaker response: {}", result);
                    if result {
                        return Ok(true);
                    } else {
                        return Err("Invalid voice code".to_string());
                    }
                }
            }
            Ok(None) => {
                warn!("Updates channel closed");
                return Err("Updates channel closed".to_string());
            }
            Err(_) => {
                // Таймаут одной итерации - продолжаем ждать
                continue;
            }
        }
    }
}

/// Отправить команду "/speaker {code}" боту
/// Возвращает ID отправленного сообщения
async fn send_speaker_command_with_code(client: &TelegramClient, voice_code: &str) -> Result<i32, String> {
    info!("Sending /speaker {} to bot", voice_code);

    let client_inner = client.client.lock().await;
    let client_inner = client_inner
        .as_ref()
        .ok_or_else(|| "Client not initialized".to_string())?;

    // Разрешаем username бота
    let bot = client_inner
        .resolve_username(BOT_USERNAME)
        .await
        .map_err(|e| format!("Failed to resolve bot: {}", e))?
        .ok_or_else(|| "Bot not found".to_string())?;

    debug!(username = ?bot.username(), "Bot resolved");

    // Формируем команду
    let command = format!("/speaker {}", voice_code);
    trace!("Sending command: '{}'", command);

    // Отправляем сообщение
    let bot_ref = bot.to_ref().await
        .ok_or_else(|| "Failed to get bot peer ref".to_string())?;
    let result = client_inner
        .send_message(bot_ref, command)
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    let msg_id = result.id();
    info!(msg_id, "Sent /speaker {} msg_id={}, waiting for text response", voice_code, msg_id);

    Ok(msg_id)
}

/// Извлечь ответ об установке спикера из текстового сообщения
/// Проверяет что сообщение является ответом на сообщение с expected_msg_id
fn extract_set_speaker_response_from_update(update_like: &UpdatesLike, expected_msg_id: i32) -> Option<bool> {
    trace!("Extracting set_speaker response, expected_msg_id={}", expected_msg_id);

    match update_like {
        // UpdatesLike::Updates - enum с массивом updates
        UpdatesLike::Updates(updates_enum) => {
            trace!("Processing UpdatesLike::Updates, variant: {:?}", std::mem::discriminant(updates_enum));
            match updates_enum {
                grammers_tl_types::enums::Updates::Updates(u) => {
                    trace!("Processing updates enum, {} updates", u.updates.len());
                    for (idx, update) in u.updates.iter().enumerate() {
                        trace!("Update[{}]: checking type", idx);

                        // Паттерн-матчинг для всех типов Update которые могут содержать текст
                        match update {
                            // NewMessage - обычное сообщение
                            grammers_tl_types::enums::Update::NewMessage(msg) => {
                                trace!("Update[{}]: is NewMessage", idx);
                                if let Some(result) = process_message(&msg.message, expected_msg_id, idx) {
                                    return Some(result);
                                }
                            }

                            // NewChannelMessage - сообщение в канале/супергруппе
                            grammers_tl_types::enums::Update::NewChannelMessage(msg) => {
                                trace!("Update[{}]: is NewChannelMessage", idx);
                                if let Some(result) = process_message(&msg.message, expected_msg_id, idx) {
                                    return Some(result);
                                }
                            }

                            // EditMessage - редактированное сообщение
                            grammers_tl_types::enums::Update::EditMessage(msg) => {
                                trace!("Update[{}]: is EditMessage", idx);
                                if let Some(result) = process_message(&msg.message, expected_msg_id, idx) {
                                    return Some(result);
                                }
                            }

                            // EditChannelMessage - редактированное сообщение в канале
                            grammers_tl_types::enums::Update::EditChannelMessage(msg) => {
                                trace!("Update[{}]: is EditChannelMessage", idx);
                                if let Some(result) = process_message(&msg.message, expected_msg_id, idx) {
                                    return Some(result);
                                }
                            }

                            // Other types - логируем детально для отладки
                            other => {
                                // Подробное логирование unhandled types
                                trace!("Update[{}]: is unhandled type: {:?}, full data: {:?}", idx, std::mem::discriminant(other), other);
                            }
                        }
                    }
                }
                grammers_tl_types::enums::Updates::UpdateShortMessage(msg) => {
                    trace!("Processing Updates::UpdateShortMessage");
                    trace!("UpdateShortMessage: out={}, message='{}', reply_to={:?}",
                        msg.out, msg.message, msg.reply_to);

                    // Игнорируем исходящие сообщения
                    if msg.out {
                        trace!("Skipping outgoing UpdateShortMessage");
                        return None;
                    }

                    // Проверяем что это ответ на наше сообщение
                    let is_reply_to_our_msg = match &msg.reply_to {
                        Some(grammers_tl_types::enums::MessageReplyHeader::Header(h))
                            if h.reply_to_msg_id == Some(expected_msg_id) => {
                            trace!("UpdateShortMessage IS a reply to our message (expected_msg_id={})", expected_msg_id);
                            true
                        }
                        _ => {
                            trace!("UpdateShortMessage is NOT a reply to our message, reply_to={:?}, expected_msg_id={}",
                                msg.reply_to.as_ref().and_then(|h| {
                                    if let grammers_tl_types::enums::MessageReplyHeader::Header(header) = h {
                                        header.reply_to_msg_id
                                    } else {
                                        None
                                    }
                                }), expected_msg_id);
                            false
                        }
                    };

                    if is_reply_to_our_msg && !msg.message.is_empty() {
                        trace!("Parsing text from UpdateShortMessage: '{}'", msg.message);
                        let result = parse_message_text_with_validation(&msg.message);
                        return Some(result);
                    }
                }
                grammers_tl_types::enums::Updates::UpdateShortChatMessage(msg) => {
                    trace!("Processing Updates::UpdateShortChatMessage");
                    trace!("UpdateShortChatMessage: out={}, message='{}'", msg.out, msg.message);

                    // Игнорируем исходящие сообщения
                    if msg.out {
                        trace!("Skipping outgoing UpdateShortChatMessage");
                        return None;
                    }

                    // UpdateShortChatMessage не содержит reply_to, поэтому проверяем по другому
                    // Для простоты просто парсим текст
                    if !msg.message.is_empty() {
                        trace!("Parsing text from UpdateShortChatMessage: '{}'", msg.message);
                        let result = parse_message_text_with_validation(&msg.message);
                        return Some(result);
                    }
                }
                other => {
                    trace!("Updates is not Updates::Updates, UpdateShortMessage, or UpdateShortChatMessage, it's: {:?}", std::mem::discriminant(other));
                    trace!("Full data: {:?}", other);
                }
            }
        }

        // Other variants
        other => {
            trace!("Unhandled UpdatesLike variant: {:?}", std::mem::discriminant(other));
            trace!("Full data: {:?}", other);
        }
    }

    trace!("No valid set_speaker response found in this update");
    None
}

/// Обработать Message enum (используется для NewMessage, EditMessage и т.д.)
fn process_message(message: &grammers_tl_types::enums::Message, expected_msg_id: i32, idx: usize) -> Option<bool> {
    match message {
        grammers_tl_types::enums::Message::Message(m) => {
            trace!("Update[{}]: is Message, out={}, msg_id={}", idx, m.out, m.id);

            // Игнорируем исходящие сообщения (наши собственные)
            if m.out {
                trace!("Skipping outgoing message msg_id={}", m.id);
                return None;
            }

            trace!("Processing incoming message msg_id={}, has_reply_to={}",
                m.id, m.reply_to.is_some());

            // Проверяем что это ответ на наше сообщение
            match &m.reply_to {
                Some(grammers_tl_types::enums::MessageReplyHeader::Header(h)) if h.reply_to_msg_id == Some(expected_msg_id) => {
                    trace!("Message IS a reply to our message (expected_msg_id={})", expected_msg_id);
                    // Это ответ на наше сообщение - обрабатываем
                }
                other => {
                    // Не ответ на наше сообщение - пропускаем
                    let reply_to_id = other.as_ref().and_then(|h| {
                        if let grammers_tl_types::enums::MessageReplyHeader::Header(header) = h {
                            header.reply_to_msg_id
                        } else {
                            None
                        }
                    });
                    trace!("Skipping message - not a reply to our message, reply_to={:?}, expected_msg_id={}",
                        reply_to_id, expected_msg_id);
                    return None;
                }
            }

            trace!("Message has_media={}, message_len={}", m.media.is_some(), m.message.len());

            // Ищем текстовое сообщение (без медиа)
            // reply_markup может быть (инлайн-кнопки бота)
            if m.media.is_none() {
                let text = &m.message;
                if !text.is_empty() {
                    trace!("Parsing text from reply: '{}'", text);
                    // Парсим ответ
                    return Some(parse_message_text_with_validation(text));
                } else {
                    trace!("Reply message has empty text, continuing");
                }
            } else {
                trace!("Reply message has media, skipping");
            }
        }
        other => {
            trace!("Update[{}]: Message variant is not Message, it's {:?}", idx, std::mem::discriminant(other));
        }
    }
    None
}

/// Спарсить текст сообщения с валидацией
fn parse_message_text_with_validation(text: &str) -> bool {
    trace!("Parsing text: '{}'", text);
    match parse_set_speaker_response(text) {
        Ok(result) => {
            info!("Successfully parsed set_speaker response: result={}", result);
            result
        }
        Err(_) => {
            trace!("Failed to parse as known response format, checking for invalid voice error");
            // Для ошибки "Invalid voice code" возвращаем false
            if text.contains("Указан неверный голос")
                || text.contains("Вказано невірний голос") {
                info!("Detected 'invalid voice' error in response");
                false
            } else {
                trace!("Unknown response format, continuing to wait");
                // Неизвестный формат - возвращаем false чтобы caller продолжил ждать
                false
            }
        }
    }
}

/// Парсит текст ответа бота для set_speaker
/// Возвращает Ok(true) если успешно, Err если неверный код
fn parse_set_speaker_response(text: &str) -> Result<bool, String> {
    trace!("Parsing set_speaker response text: '{}'", text);

    // Проверить варианты успешного ответа
    if text.contains("Успешно выбран спикер")
        || text.contains("Успішно обрано спікера")
        || text.contains("Successfully selected speaker") {
        trace!("Matched success pattern 'Успешно выбран спикер'");
        return Ok(true);
    }

    if text.contains("Успешно выбран тот же самый спикер")
        || text.contains("Успішно обрано того самого спікера") {
        trace!("Matched success pattern 'Успешно выбран тот же самый спикер'");
        return Ok(true);
    }

    if text.contains("Указан неверный голос")
        || text.contains("Вказано невірний голос") {
        trace!("Matched error pattern 'Указан неверный голос'");
        return Err("Invalid voice code".to_string());
    }

    trace!("No pattern matched, returning unknown format error");
    Err("Unknown response format".to_string())
}
