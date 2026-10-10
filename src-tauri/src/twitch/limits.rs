//! Provider-aware политика длины Twitch-доставки (ROADMAP-106).
//!
//! Один запрос пользователя может быть длиннее одного сообщения Twitch:
//! здесь текст раскладывается на части ДО отправки. Границы частей — только
//! между словами: слово не разрезается и целиком переносится в следующую
//! часть. Единственный отказ на этапе планирования — слово, которое не
//! помещается одно в лимит части.

/// Символьный лимит одного сообщения Twitch (Send Chat Message API).
pub(crate) const MAX_MESSAGE_CHARS: usize = 500;

/// Лимит IRC wire-строки в байтах, включая завершающий CRLF (RFC 1459).
pub(crate) const MAX_WIRE_BYTES: usize = 512;

/// Длина префикса `". "`, который twitch-irc добавляет в `TwitchIRCClient::say`
/// к тексту части против исполнения slash-команд: Twitch стрипает ведущую
/// точку, поэтому видимый текст не меняется, но wire-длина и символьный счёт
/// сообщения растут на 2 — бюджет планировщика обязан это учитывать.
pub(crate) const SAY_PREFIX_LEN: usize = 2;

/// Причина, по которой текст нельзя разбить на доставимые части.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlanError {
    /// Одно слово длиннее символьного лимита Twitch.
    WordExceedsCharLimit,
    /// Одно слово не помещается в IRC wire budget (символов хватает, байтов —
    /// нет: кириллица/CJK/emoji занимают 2–4 байта на символ).
    WordExceedsWireBudget,
}

/// Полная длина wire-строки `PRIVMSG #<channel> :. <text>\r\n` с CRLF,
/// включая префикс экранирования `". "` от twitch-irc `say()`: Twitch-клиент
/// отправляет текст именно в этой обвязке.
pub(crate) fn wire_frame_len(channel: &str, text_len: usize) -> usize {
    "PRIVMSG #".len() + channel.len() + " :".len() + SAY_PREFIX_LEN + text_len + "\r\n".len()
}

/// Общий жадный алгоритм планирования: проходит текст по словам в исходном
/// порядке, нормализуя whitespace в один пробел, и копит часть до тех пор, пока
/// `fits(текущая_часть, слово)` истинно. `validate(слово)` вызывается для
/// каждого слова до раскладки и отвергает слово, которое не помещается в часть
/// одно (разбиение невозможно без разрезания слова).
///
/// Помощник параметризован политикой длины, но не является транспортом: он
/// ничего не знает ни об IRC, ни об HTTP API, а лишь применяет переданные
/// проверки.
fn plan_words<Validate, Fits>(
    clean_text: &str,
    validate: Validate,
    fits: Fits,
) -> Result<Vec<String>, PlanError>
where
    Validate: Fn(&str) -> Result<(), PlanError>,
    Fits: Fn(usize, usize, usize, usize) -> bool,
{
    let mut parts: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_chars = 0;

    for word in clean_text.split_whitespace() {
        validate(word)?;
        let word_chars = word.chars().count();
        // Резерв только для уже начатой части: `fits` вызывается, когда
        // добавление пойдёт через пробел-разделитель.
        if !current.is_empty() && !fits(current_chars, current.len(), word_chars, word.len()) {
            parts.push(std::mem::take(&mut current));
            current_chars = 0;
        }
        if !current.is_empty() {
            current.push(' ');
            current_chars += 1;
        }
        current.push_str(word);
        current_chars += word_chars;
    }
    if !current.is_empty() {
        parts.push(current);
    }
    Ok(parts)
}

/// Раскладывает ОЧИЩЕННЫЙ текст на доставимые части.
///
/// Инварианты (проверяются тестами):
/// - каждая часть укладывается и в `MAX_MESSAGE_CHARS` символов, и в
///   `MAX_WIRE_BYTES` вместе с обвязкой `PRIVMSG #<channel> :. …\r\n`
///   (включая префикс `". "` от `say()`);
/// - слова никогда не разрезаются: разрез возможен только по whitespace,
///   поэтому emoji с ZWJ/variation selector и combining marks не повреждаются;
/// - порядок слов сохранён, хвост не теряется, пустых частей нет;
/// - разделитель между словами нормализуется в один пробел (в т.ч. табы и
///   Unicode-whitespace);
/// - результат детерминирован.
///
/// Пустой/из одних пробелов текст возвращает пустой Vec (пустоту обязан
/// отвергнуть вызывающий ДО планирования).
pub(crate) fn plan_message_parts(
    clean_text: &str,
    channel: &str,
) -> Result<Vec<String>, PlanError> {
    // Обвязка PRIVMSG (включая префикс say()) вокруг текста: её байты не
    // зависят от содержимого части.
    let overhead = wire_frame_len(channel, 0);

    plan_words(
        clean_text,
        |word| {
            let word_chars = word.chars().count();
            let word_bytes = word.len();

            // Слово обязано помещаться одно в пустую часть — иначе разбиение
            // невозможно без разрезания слова. Символьный лимит проверяется
            // раньше байтового (как и в исходной политике).
            if word_chars + SAY_PREFIX_LEN > MAX_MESSAGE_CHARS {
                return Err(PlanError::WordExceedsCharLimit);
            }
            if overhead + word_bytes > MAX_WIRE_BYTES {
                return Err(PlanError::WordExceedsWireBudget);
            }
            Ok(())
        },
        |current_chars, current_bytes, word_chars, word_bytes| {
            // +1 символ/байт на пробел-разделитель, обвязка — на всю строку.
            current_chars + 1 + word_chars + SAY_PREFIX_LEN <= MAX_MESSAGE_CHARS
                && overhead + current_bytes + 1 + word_bytes <= MAX_WIRE_BYTES
        },
    )
}

/// Раскладывает ОЧИЩЕННЫЙ текст на части для Send Chat Message API.
///
/// Политика API отличается от IRC: лимит — ровно [`MAX_MESSAGE_CHARS`] Unicode
/// scalar values на сообщение, без префикса `". "` от `say()` и без байтового
/// wire-бюджета. Поэтому 500 кириллических символов (1000 байт) принимаются
/// целиком, тогда как IRC-планировщик такой же текст отвергнет.
///
/// Инварианты совпадают с [`plan_message_parts`] за вычетом wire-счётчиков:
/// части не превышают `MAX_MESSAGE_CHARS`, слова (`split_whitespace`) не
/// разрезаются, порядок сохранён, хвост не теряется, разделители нормализованы
/// в один пробел, пустой/пробельный ввод даёт пустой Vec. Слово длиннее
/// `MAX_MESSAGE_CHARS` отвергается как [`PlanError::WordExceedsCharLimit`]
/// ДО любой отправки.
pub(crate) fn plan_api_message_parts(clean_text: &str) -> Result<Vec<String>, PlanError> {
    plan_words(
        clean_text,
        |word| {
            if word.chars().count() > MAX_MESSAGE_CHARS {
                Err(PlanError::WordExceedsCharLimit)
            } else {
                Ok(())
            }
        },
        |current_chars, _, word_chars, _| current_chars + 1 + word_chars <= MAX_MESSAGE_CHARS,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Канал для тестов: overhead = 15 + 4 = 19, бюджет текста 493 байта.
    const CHANNEL: &str = "test";

    /// Текст из слов не длиннее `word_len` символов с одним пробелом между
    /// словами, суммарно ровно `total_chars` символов (последние слова могут
    /// быть короче).
    fn words_text(total_chars: usize, word_len: usize) -> String {
        // Минимальное n слов, при котором n слов + (n-1) пробелов хватает на
        // total_chars: (n-1) + n*word_len >= total_chars.
        let n = total_chars.div_ceil(word_len + 1);
        assert!(n >= 1, "total_chars too small for the given word_len");
        let words_total = total_chars - (n - 1);
        let base = words_total / n;
        let extra = words_total % n;
        let words: Vec<String> = (0..n)
            .map(|i| "a".repeat(base + if i < extra { 1 } else { 0 }))
            .collect();
        let text = words.join(" ");
        assert_eq!(text.chars().count(), total_chars);
        text
    }

    /// Все инварианты планировщика на произвольном тексте.
    fn assert_plan_invariants(text: &str, parts: &[String]) {
        assert!(!parts.is_empty(), "non-empty text must yield parts");
        for part in parts {
            assert!(!part.is_empty(), "no empty parts");
            assert!(!part.starts_with(' ') && !part.ends_with(' '));
            assert!(part.chars().count() <= MAX_MESSAGE_CHARS);
            assert!(wire_frame_len(CHANNEL, part.len()) <= MAX_WIRE_BYTES);
        }
        // Порядок слов сохранён и хвост не потерян (разделители нормализованы
        // в один пробел).
        let words: Vec<&str> = text.split_whitespace().collect();
        let joined: Vec<&str> = parts.iter().flat_map(|p| p.split_whitespace()).collect();
        assert_eq!(joined, words);
    }

    #[test]
    fn short_text_is_one_part() {
        let parts = plan_message_parts("hello world", CHANNEL).unwrap();
        assert_eq!(parts, vec!["hello world".to_string()]);
    }

    #[test]
    fn empty_and_whitespace_text_yield_no_parts() {
        assert!(plan_message_parts("", CHANNEL).unwrap().is_empty());
        assert!(plan_message_parts("   ", CHANNEL).unwrap().is_empty());
    }

    #[test]
    fn boundaries_499_500_501_chars_split_at_word_edges() {
        for total in [499usize, 500, 501] {
            let text = words_text(total, 4);
            let parts = plan_message_parts(&text, CHANNEL).unwrap();
            // Бюджет текста 493 байта < 499 символов: одного сообщения мало.
            assert!(parts.len() >= 2, "total={total} must split");
            assert_plan_invariants(&text, &parts);
        }
    }

    #[test]
    fn greedy_ascii_split_point_is_deterministic() {
        // 100 слов по 4 символа = 499 символов. В часть помещается 5k-1 <= 493
        // байт => 98 слов (489), остаток «aaaa aaaa» переносится целиком.
        let text = words_text(499, 4);
        let parts = plan_message_parts(&text, CHANNEL).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].chars().count(), 489);
        assert_eq!(parts[0].split_whitespace().count(), 98);
        assert_eq!(parts[1], "aaaa aaaa");
        assert_plan_invariants(&text, &parts);
    }

    #[test]
    fn multibyte_cyrillic_respects_wire_budget() {
        // 100 слов «яяя»: часть вмещает 7k-1 <= 493 байт => 70 слов (489 байт).
        let text = vec!["яяя"; 100].join(" ");
        let parts = plan_message_parts(&text, CHANNEL).unwrap();
        assert!(parts.len() >= 2);
        assert_plan_invariants(&text, &parts);
    }

    #[test]
    fn cjk_and_emoji_words_are_never_cut() {
        // 80 повторов «世界» = 160 символов / 480 байт: слово помещается в
        // wire-бюджет одно (19 + 480 = 499 <= 512), но текст в целом всё ещё
        // разбивается на несколько частей.
        let text = format!("{} {}", "世界".repeat(80), "👨‍👩‍👧‍👦 тест 🏻︎");
        let parts = plan_message_parts(&text, CHANNEL).unwrap();
        assert_plan_invariants(&text, &parts);
        // ZWJ-семья — одно «слово» (нет whitespace внутри) и должна лежать
        // целиком в одной части.
        assert!(parts.iter().any(|p| p.contains("👨‍👩‍👧‍👦")));
    }

    #[test]
    fn separators_are_normalized_to_single_space() {
        let parts = plan_message_parts("a\tb  c   d", CHANNEL).unwrap();
        assert_eq!(parts, vec!["a b c d".to_string()]);
    }

    #[test]
    fn single_word_over_char_limit_is_rejected() {
        let word = "a".repeat(MAX_MESSAGE_CHARS + 1);
        assert_eq!(
            plan_message_parts(&word, CHANNEL),
            Err(PlanError::WordExceedsCharLimit)
        );
        // Кириллица: 501 символ = 1002 байта — падает по СИМВОЛЬНОМУ лимиту
        // раньше wire-проверки (она тоже не прошла бы, но char-лимит первичен).
        let cyr = "я".repeat(501);
        assert_eq!(
            plan_message_parts(&cyr, CHANNEL),
            Err(PlanError::WordExceedsCharLimit)
        );
    }

    #[test]
    fn word_within_char_limit_but_over_wire_budget_is_rejected() {
        // 497 ASCII-символов + префикс = 499 <= 500 символов, но
        // 19 + 497 = 516 > 512 байт.
        let word = "a".repeat(497);
        assert_eq!(
            plan_message_parts(&word, CHANNEL),
            Err(PlanError::WordExceedsWireBudget)
        );
    }

    #[test]
    fn word_exactly_at_wire_budget_fits_alone() {
        // 19 + 493 = 512 — ровно лимит wire-строки.
        let word = "a".repeat(493);
        let parts = plan_message_parts(&word, CHANNEL).unwrap();
        assert_eq!(parts, vec![word]);
        // 19 + 495 = 514 > 512: прежняя граница больше не проходит — префикс
        // say() съедает 2 байта бюджета.
        assert_eq!(
            plan_message_parts(&"a".repeat(495), CHANNEL),
            Err(PlanError::WordExceedsWireBudget)
        );
    }

    #[test]
    fn word_fits_only_alone_forces_it_into_own_part() {
        // Слово на 489 символов не помещается в часть вместе с соседним словом
        // (19 + 489 + 1 + 4 = 513 > 512), хотя одно — помещается (508 <= 512).
        let long_word = "b".repeat(489);
        let text = format!("{} aaaa", long_word);
        let parts = plan_message_parts(&text, CHANNEL).unwrap();
        assert_eq!(
            parts,
            vec![long_word.clone(), "aaaa".to_string()],
            "слово переносится целиком, а не дожимается в часть"
        );
    }

    #[test]
    fn long_channel_name_shrinks_text_budget() {
        // Канал из 25 символов: overhead 40, бюджет текста 472 байта.
        let long_channel = "c".repeat(25);
        let word = "a".repeat(475); // 40 + 475 = 515 > 512
        assert_eq!(
            plan_message_parts(&word, &long_channel),
            Err(PlanError::WordExceedsWireBudget)
        );
        // То же слово с коротким каналом проходит.
        assert_eq!(plan_message_parts(&word, CHANNEL).unwrap(), vec![word]);
    }

    #[test]
    fn many_parts_preserve_order_and_tail() {
        let text = words_text(4999, 5); // ~ 4KB — Silero-подобный максимум ввода
        let parts = plan_message_parts(&text, CHANNEL).unwrap();
        assert!(parts.len() >= 8, "expected many parts, got {}", parts.len());
        assert_plan_invariants(&text, &parts);
    }

    #[test]
    fn wire_frame_len_counts_envelope_and_crlf() {
        assert_eq!(wire_frame_len("", 0), 15);
        assert_eq!(wire_frame_len("test", 0), 19);
        assert_eq!(wire_frame_len("test", 10), 29);
    }

    // --- API mode (Send Chat Message API): только символьный лимит. ---

    /// Инварианты API-планировщика: 500 скалярных символов на часть и ничего
    /// больше — ни префикса `say()`, ни байтового wire-бюджета.
    fn assert_api_plan_invariants(text: &str, parts: &[String]) {
        assert!(!parts.is_empty(), "non-empty text must yield parts");
        for part in parts {
            assert!(!part.is_empty(), "no empty parts");
            assert!(!part.starts_with(' ') && !part.ends_with(' '));
            assert!(part.chars().count() <= MAX_MESSAGE_CHARS);
        }
        let words: Vec<&str> = text.split_whitespace().collect();
        let joined: Vec<&str> = parts.iter().flat_map(|p| p.split_whitespace()).collect();
        assert_eq!(joined, words);
    }

    #[test]
    fn api_short_text_is_one_part() {
        let parts = plan_api_message_parts("hello world").unwrap();
        assert_eq!(parts, vec!["hello world".to_string()]);
    }

    #[test]
    fn api_empty_and_whitespace_text_yield_no_parts() {
        assert!(plan_api_message_parts("").unwrap().is_empty());
        assert!(plan_api_message_parts("   ").unwrap().is_empty());
        assert!(plan_api_message_parts("\t\n  ").unwrap().is_empty());
    }

    #[test]
    fn api_separators_are_normalized_to_single_space() {
        let parts = plan_api_message_parts("a\tb  c   d").unwrap();
        assert_eq!(parts, vec!["a b c d".to_string()]);
    }

    #[test]
    fn api_500_cyrillic_chars_accepted_while_irc_wire_rejects() {
        // 500 символов = 1000 байт: API не считает байты и принимает слово
        // целиком, IRC-планировщик тот же текст отвергает по char/wire-бюджету.
        let word = "я".repeat(MAX_MESSAGE_CHARS);
        assert_eq!(word.chars().count(), 500);
        assert!(word.len() > MAX_WIRE_BYTES, "cyrillic word is >512 bytes");
        let parts = plan_api_message_parts(&word).unwrap();
        assert_eq!(parts, vec![word.clone()]);
        assert_eq!(
            plan_message_parts(&word, CHANNEL),
            Err(PlanError::WordExceedsCharLimit)
        );
    }

    #[test]
    fn api_single_word_over_500_chars_is_rejected() {
        let ascii = "a".repeat(MAX_MESSAGE_CHARS + 1);
        assert_eq!(
            plan_api_message_parts(&ascii),
            Err(PlanError::WordExceedsCharLimit)
        );
        let cyrillic = "я".repeat(MAX_MESSAGE_CHARS + 1);
        assert_eq!(
            plan_api_message_parts(&cyrillic),
            Err(PlanError::WordExceedsCharLimit)
        );
    }

    #[test]
    fn api_budget_is_larger_than_irc_wire_budget() {
        // 497 ASCII-символов: IRC отклоняет по wire-бюджету (19 + 497 > 512),
        // API принимает одним сообщением (497 <= 500).
        let word = "a".repeat(497);
        assert_eq!(
            plan_message_parts(&word, CHANNEL),
            Err(PlanError::WordExceedsWireBudget)
        );
        assert_eq!(plan_api_message_parts(&word).unwrap(), vec![word]);
    }

    #[test]
    fn api_multiword_over_500_chars_splits_in_order_without_truncation() {
        // 501 символ: API обязан разбить строго по границам слов, сохранив
        // порядок и хвост. IRC-бюджет меньше, поэтому тот же текст он режет
        // мельче — политики действительно различаются.
        let text = words_text(501, 4);
        let api_parts = plan_api_message_parts(&text).unwrap();
        assert!(api_parts.len() >= 2, "501 chars must split");
        assert_api_plan_invariants(&text, &api_parts);

        let irc_parts = plan_message_parts(&text, CHANNEL).unwrap();
        assert!(
            api_parts[0].chars().count() > irc_parts[0].chars().count(),
            "API part must hold more scalar chars than IRC part"
        );
    }

    #[test]
    fn api_greedy_split_point_is_deterministic() {
        // 100 слов по 5 символов = 599 символов. 83 слова дают 497 символов,
        // 84-е (503) не помещается — переносится; остаток 17 слов в одной части.
        let text = words_text(599, 5);
        let parts = plan_api_message_parts(&text).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].chars().count(), 497);
        assert_eq!(parts[0].split_whitespace().count(), 83);
        assert_eq!(parts[1].split_whitespace().count(), 17);
        assert_api_plan_invariants(&text, &parts);
    }

    #[test]
    fn api_emoji_zwj_and_combining_word_is_never_cut() {
        // ZWJ-семья и «e + combining acute» — отдельные слова без whitespace
        // внутри; длинное слово из CJK переносится целиком в свою часть.
        let family = "👨‍👩‍👧‍👦";
        let combining = "e\u{0301}";
        let text = format!("{} {} {}", family, combining, "字".repeat(495));
        let parts = plan_api_message_parts(&text).unwrap();
        assert_api_plan_invariants(&text, &parts);
        assert_eq!(parts.len(), 2);
        assert!(parts[0].contains(family), "ZWJ family must stay intact");
        assert!(
            parts[0].contains(combining),
            "combining sequence must stay intact"
        );
        assert_eq!(parts[1].chars().count(), 495);
    }
}
