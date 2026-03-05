use super::client::TelegramClient;
use super::types::{TtsResult, CurrentVoice, Limits};
use grammers_session::updates::UpdatesLike;
use std::path::PathBuf;

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
        println!("[SILORO] Starting TTS synthesis for text: '{}'", text);

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

        println!("[SILORO] TTS synthesis completed: {}", audio_path);

        Ok(TtsResult::success(audio_path))
    }

    /// Отправить текст боту
    async fn send_text_to_bot(client: &TelegramClient, text: &str) -> Result<(), String> {
        println!("[SILORO] Sending text to bot: {}", text);

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

        println!("[SILORO] Bot resolved: {:?}", bot.username());

        // Отправляем сообщение
        let result = client_inner
            .send_message(&bot, text)
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        println!("[SILORO] Message sent: {:?}", result);

        Ok(())
    }

    /// Ожидать голосовое сообщение от бота с таймаутом
    async fn wait_for_voice_message(
        client: &TelegramClient,
        timeout_secs: u64,
    ) -> Result<VoiceMessageResult, String> {
        use tokio::sync::mpsc::UnboundedReceiver;

        println!("[SILORO] Waiting for voice message (timeout: {}s)...", timeout_secs);

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
                println!("[SILORO] Timeout waiting for voice message");
                return Err("Timeout waiting for voice message".to_string());
            }

            let remaining = total_timeout.saturating_sub(elapsed);

            match tokio::time::timeout(remaining, updates.recv()).await {
                Ok(Some(update_like)) => {
                    if let Some(result) = Self::extract_voice_from_update(&update_like) {
                        println!(
                            "[SILORO] Voice message found: file_id={}, msg_id={}, mime={}",
                            result.file_id, result.msg_id, result.mime_type
                        );
                        return Ok(result);
                    }
                }
                Ok(None) => {
                    println!("[SILORO] Updates channel closed");
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
    fn extract_voice_from_update(update_like: &UpdatesLike) -> Option<VoiceMessageResult> {
        match update_like {
            UpdatesLike::Updates(updates_enum) => {
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
            _ => {}
        }
        None
    }

    /// Скачать голосовое сообщение во временную папку
    async fn download_voice_to_temp(
        client: &TelegramClient,
        voice: &VoiceMessageResult,
    ) -> Result<String, String> {
        println!(
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
        let mut iter = client_inner.iter_messages(&bot);
        let mut msg_count = 0;

        loop {
            match iter.next().await {
                Ok(Some(msg)) => {
                    msg_count += 1;
                    if msg.id() == voice.msg_id {
                        println!(
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

                            println!("[SILORO] Downloading to: {}", dest_path.display());

                            // Скачиваем медиа
                            client_inner
                                .download_media(&media, &dest_path)
                                .await
                                .map_err(|e| format!("Download failed: {}", e))?;

                            println!("[SILORO] Download completed: {}", dest_path.display());

                            return Ok(dest_path
                                .to_str()
                                .ok_or_else(|| "Invalid path".to_string())?
                                .to_string());
                        }
                    }
                }
                Ok(None) => {
                    println!(
                        "[SILORO] Message {} not found after checking {} messages",
                        voice.msg_id, msg_count
                    );
                    return Err(format!("Message {} not found", voice.msg_id));
                }
                Err(e) => {
                    eprintln!("[SILORO] Error iterating messages: {}", e);
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

    println!("[SILERO VOICE] Getting current voice from bot");

    // 1. Отправляем /speaker
    send_speaker_command(client).await?;

    println!("[SILERO VOICE] /speaker sent, waiting for text response...");

    // 2. Ждем текстовое сообщение (не меню, не голос)
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
            println!("[SILERO VOICE] Timeout (60s) waiting for voice info");
            return Ok(None);  // Таймаут - возвращаем None
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, updates.recv()).await {
            Ok(Some(update_like)) => {
                // Проверяем, есть ли текстовое сообщение с информацией о голосе
                if let Some(voice_info) = extract_voice_info_from_update(&update_like) {
                    println!("[SILERO VOICE] Voice info found: {} ({})", voice_info.name, voice_info.id);
                    return Ok(Some(voice_info));  // Нашли - возвращаем Some
                }
            }
            Ok(None) => {
                println!("[SILERO VOICE] Updates channel closed");
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
async fn send_speaker_command(client: &TelegramClient) -> Result<(), String> {
    println!("[SILERO VOICE] Sending /speaker to bot");

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

    println!("[SILERO VOICE] Bot resolved: {:?}", bot.username());

    // Отправляем сообщение
    let result = client_inner
        .send_message(&bot, "/speaker")
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    println!("[SILERO VOICE] Message sent: {:?}", result);

    Ok(())
}

/// Извлечь информацию о текущем голосе из текстового сообщения
/// Парсит: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
fn extract_voice_info_from_update(update_like: &UpdatesLike) -> Option<CurrentVoice> {
    match update_like {
        UpdatesLike::Updates(updates_enum) => {
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
        _ => {}
    }
    None
}

/// Парсит текст ответа бота для получения информации о голосе
/// Формат: "Выбранный голос: /speaker hamster_clerk\nНаходится в паке: Хомяки"
fn parse_voice_info(text: &str) -> Option<CurrentVoice> {
    println!("[SILERO VOICE] Parsing text: '{}'", text);

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
        println!("[SILERO VOICE] Parsed: id={}, name={}", id, name);
        Some(CurrentVoice { name, id })
    } else {
        println!("[SILERO VOICE] Failed to parse voice info from text");
        None
    }
}

/// Отправить /limits и дождаться текстового ответа с лимитами
/// Парсит: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
/// Таймаут 60 секунд на ожидание ответа
pub async fn get_limits(client: &TelegramClient) -> Result<Option<Limits>, String> {
    use tokio::sync::mpsc::UnboundedReceiver;

    println!("[SILERO LIMITS] Getting limits from bot");

    // 1. Отправляем /limits
    send_limits_command(client).await?;

    println!("[SILERO LIMITS] /limits sent, waiting for text response...");

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
            println!("[SILERO LIMITS] Timeout (60s) waiting for limits info");
            return Ok(None);  // Таймаут - возвращаем None
        }

        let remaining = total_timeout.saturating_sub(elapsed);

        match tokio::time::timeout(remaining, updates.recv()).await {
            Ok(Some(update_like)) => {
                // Проверяем, есть ли текстовое сообщение с информацией о лимитах
                if let Some(limits_info) = extract_limits_info_from_update(&update_like) {
                    println!("[SILERO LIMITS] Limits info found: voices={}, gifs={}", limits_info.voices, limits_info.gifs);
                    return Ok(Some(limits_info));  // Нашли - возвращаем Some
                }
            }
            Ok(None) => {
                println!("[SILERO LIMITS] Updates channel closed");
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
    println!("[SILERO LIMITS] Sending /limits to bot");

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

    println!("[SILERO LIMITS] Bot resolved: {:?}", bot.username());

    // Отправляем сообщение
    let result = client_inner
        .send_message(&bot, "/limits")
        .await
        .map_err(|e| format!("Failed to send message: {}", e))?;

    println!("[SILERO LIMITS] Message sent: {:?}", result);

    Ok(())
}

/// Извлечь информацию о лимитах из текстового сообщения
/// Парсит: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
fn extract_limits_info_from_update(update_like: &UpdatesLike) -> Option<Limits> {
    match update_like {
        UpdatesLike::Updates(updates_enum) => {
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
        _ => {}
    }
    None
}

/// Парсит текст ответа бота для получения информации о лимитах
/// Формат: "🔓 Открытые голоса: 0 / 666 символов;" и "🪩 Кружки/гифки: 0 / 10 сообщений;"
fn parse_limits_info(text: &str) -> Option<Limits> {
    println!("[SILERO LIMITS] Parsing text: '{}'", text);

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
        println!("[SILERO LIMITS] Parsed: voices={}, gifs={}", voices_val, gifs_val);
        Some(Limits {
            voices: voices_val,
            gifs: gifs_val,
        })
    } else {
        println!("[SILERO LIMITS] Failed to parse limits info from text");
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
