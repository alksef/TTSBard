# Задача DeepSeek: реализовать FakeTLS для MTProxy в grammers (Round 1)

**Рабочая директория:** `D:/RustProjects/grammers` (git-репо `alksef/grammers`, ветка `master`).
**Не трогай:** репо `app-tts-v2`. Код пишешь ТОЛЬКО в `D:/RustProjects/grammers`.

## Цель одной фразой

Добавить в grammers поддержку MTProxy-режима **FakeTLS** (секрет длиной >17 байт:
`tag + key[16] + hex(domain)`), чтобы заработал ключ
`argeiphontes.ru:13371` со секретом `ee7b184b3f7c1ace06fa2efbbaa851f1a8617267656970686f6e7465732e7275`,
и при этом не сломался рабочий Simple-ключ `argeiphontes.ru:13370` / `4758456789abcdef0123456789abcdef`.

## Обязательно прочитай перед началом

1. `D:/RustProjects/grammers/docs/faketls/02-root-cause-analysis.md` — **почему прошлая попытка провалилась**.
   Главная ошибка прошлого: после handshake reader/writer уходили в «raw mode» (без TLS-framing),
   а сервер mtg ждёт, что ВЕСЬ трафик остаётся в TLS-Application-records. Никакого raw-mode быть не должно.
2. `D:/RustProjects/grammers/docs/faketls/01-protocol-and-architecture.md` — эталонная слоистость gotd/td.
3. `D:/RustProjects/grammers/docs/faketls/04-implementation-plan.md` — пошаговый план.

## Что УЖЕ есть и работает (восстановить из git-истории, НЕ переписывать)

Из коммита `f534d9a` вернуть в `grammers-mtproto/src/tls/` эти файлы **дословно**:
```bash
git show f534d9a:grammers-mtproto/src/tls/client_hello.rs  > grammers-mtproto/src/tls/client_hello.rs
git show f534d9a:grammers-mtproto/src/tls/server_hello.rs  > grammers-mtproto/src/tls/server_hello.rs
git show f534d9a:grammers-mtproto/src/tls/record.rs        > grammers-mtproto/src/tls/record.rs
git show f534d9a:grammers-mtproto/src/tls/obfuscator.rs    > grammers-mtproto/src/tls/obfuscator.rs
```
Они корректны:
- `build_client_hello(secret: &[u8;16], hostname: &str) -> Vec<u8>` — ClientHello с GREASE+SNI.
- `validate_server_hello(server_hello, client_random, secret) -> Result<usize,_>` — HMAC-verify.
- `obfuscator::client_handshake(secret: &[u8;16], dc_id: i16) -> ([u8;64], Aes256Ctr, Aes256Ctr)`
  возвращает `(encrypted_frame, send_cipher, recv_cipher)`. send_cipher уже сдвинут на 64 байта.
- `record.rs` — `TlsRecordHeader`, константы `TLS_RECORD_{HANDSHAKE,CHANGE_CIPHER,APPLICATION,ALERT}`,
  `TLS_VERSION=[0x03,0x03]`, `MAX_TLS_PLAINTEXT_SIZE`.

**НЕ возвращай** `handshake.rs` (ClientKeyExchange/Finished с fake verify_data) — gotd их не шылёт,
mtg их не требует. Не используй эту логику.

## Что ПЕРЕПИСАТЬ (главное исправление)

### A. `grammers-mtproto/src/tls/mod.rs`
Объявить модули: `client_hello, server_hello, record, obfuscator, framing, stream`. НЕ объявлять
`handshake`. Реэкспорт `pub use stream::FakeTlsStream;`.

### B. НОВЫЙ `grammers-mtproto/src/tls/framing.rs` — TLS-record framing (AsyncRead/AsyncWrite обёртка)

Это обёртка над `S: AsyncRead+AsyncWrite`, которая **всегда** framing'ует и чтение, и запись:

- `pub struct FakeTlsFraming<S> { inner: S, read_state, write_state }`.
- **Write:** каждый `poll_write(buf)` упаковывает `buf[..min(buf.len(), MAX_TLS_PLAINTEXT_SIZE)]`
  в TLS-Application-record: `TlsRecordHeader::new(TLS_RECORD_APPLICATION, len).to_bytes()` + payload,
  и пишет в `inner`. Корректная буферизация через state-machine (как в старом writer.rs `WritingRecord`),
  данные НЕ терять при `Poll::Pending`.
- **Read:** `poll_read` читает 5-байтный TLS-record-заголовок, валидирует ContentType
  (`TLS_RECORD_APPLICATION` → отдаём payload; `TLS_RECORD_CHANGE_CIPHER` → пропускаем/игнор;
  остальные → ошибка), читает `length` байт payload, отдаёт в `buf`. Поддерживать partial reads
  и reassembly (несколько records могут прийти). Использовать внутренний буфер, т.к. record payload
  может быть больше `buf` вызывающего.
- Framing **никогда не отключается**. Это ключевое отличие от старой реализации.

### C. НОВЫЙ `grammers-mtproto/src/tls/stream.rs` — слоистая модель gotd

```text
FakeTlsStream<S>:
  framing: FakeTlsFraming<S>            // слой framing (читает/пишет TLS-records ВСЕГДА)
  obfs2_send: Aes256Ctr                 // AES-CTR шифрует исходящее
  obfs2_recv: Aes256Ctr                 // AES-CTR расшифровывает входящее
```
Реализуй `AsyncRead`/`AsyncWrite`:
- `poll_write(buf)`: скопировать buf, `obfs2_send.apply_keystream(&mut copy)`,
  `Pin::new(&mut framing).poll_write(cx, &copy)`.
- `poll_read(buf)`: `n = framing.poll_read(buf)`, если `n>0` — `obfs2_recv.apply_keystream(&mut buf[..n])`.

`async fn new(stream: S, secret: &[u8;16], dc_id: i16, hostname: &str) -> io::Result<Self>` —
выполняет handshake по модели gotd `obfuscator.go` + `faketls.go`:
1. Сгенерировать `client_hello = build_client_hello(secret, hostname)`. Запомнить `client_random`
   (32 байта по смещению `TLS_DIGEST_POS=11`). Отправить ClientHello **как TLS-Handshake-record**
   (`TLS_RECORD_HANDSHAKE`): framing тут ещё не создан, поэтому запиши руками
   `[0x16,0x03,0x03,len_be] + client_hello` в stream через `AsyncWriteExt::write_all`.
   (ClientHello из build_client_hello УЖЕ содержит внешний record-заголовок `0x16 0x03 0x01 0x02 0x00 ...` —
   ПРОВЕРЬ: если содержит, не дублируй заголовок, пиши client_hello как есть.)
2. Прочитать ответ сервера до полной проверки: `validate_server_hello(response, &client_random, secret)`.
   Прочитать нужно ServerHello + CCS + Application-шум целиком (валидатор вернёт смещение конца).
3. Получить `(frame, send_cipher, recv_cipher) = obfuscator::client_handshake(secret, dc_id)`.
4. Отправить `frame` (64 байта) **как TLS-Application-record** (`TLS_RECORD_APPLICATION`):
   `[0x17,0x03,0x03,0x00,0x40] + frame` через `write_all`.
5. **CCS-quirk (gotd faketls.go:62-75):** перед самым первым пользовательским пакетом нужно
   послать ChangeCipherSpec-record `[0x14,0x03,0x03,0x00,0x01,0x01]`. Реализуй флаг `first_packet: bool`
   в `poll_write`: при первом вызове сначала записать CCS-record в framing, потом сам payload.
6. Создать `FakeTlsStream { framing: FakeTlsFraming::new(stream), obfs2_send: send_cipher,
   obfs2_recv: recv_cipher }`.

> ВАЖНО: send_cipher из `client_handshake` уже применён к 64-байтному фрейму и сдвинут на 64 байта.
> Его состояние СИНХРОНИЗИРОВАНО с сервером — НЕ переинициализируй. recv_cipher — свежий.

## D. Режим секрета по ДЛИНЕ (не по префиксу)

В `grammers-mtproto/src/transport/mtproxy.rs` (или новом `tls/secret.rs`) определи:
```rust
pub enum ProxySecret { Simple([u8;16]), Secured([u8;16]), Faketls{ key:[u8;16], domain:String } }
pub fn parse_secret(hex_or_b64: &str) -> Result<ProxySecret, Error> {
    // 1) strip optional "ee"/"dd" PREFIX STRING, then hex/base64-decode → bytes
    // 2) match on bytes.len():
    //    16  → Simple
    //    17  → Secured (byte[0] tag)
    //    >17 → Faketls { key: bytes[1..17], domain: hex::decode(bytes[17..]) as utf8 }
}
```
Для тестового ee-ключа: hex-decode → 32 байта → tag=0xee, key=bytes[1..17], domain=`argeiphontes.ru`.

## E. Подключение в net-слой

`grammers-mtsender/src/net/tcp.rs`: в `connect_mtproxy_stream` (или в `connect`) — когда секрет
парсится как `Faketls{domain}`, выполнить `FakeTlsStream::new(tcp, &key, dc_id as i16, &domain).await`
и вернуть новый вариант `NetStream::MtProxyFakeTls(FakeTlsStream<TcpStream>)`. Добавь этот вариант
в `enum NetStream` и в `split()` (FakeTlsStream сам реализует AsyncRead+AsyncWrite — split через
`tokio::io::split` или прямой poll). Transport поверх — обычный `Intermediate`.

Для Simple/Secured — текущий путь (`NetStream::MtProxy(TcpStream)`) без изменений (регрессия!).

## Тесты (новые, в tls/ и net/)

- `parse_secret`: ee-ключ → Faketls с domain=`argeiphontes.ru`; 16-байт → Simple; dd+16 → Secured.
- `obfuscator::client_handshake`: длина frame==64; send_cipher после применения к тестовому буферу — детерминирован.
- TLS-record framing: round-trip нескольких records через `FakeTlsFraming` над `io::duplex`.

## Критерий готовности (DeepSeek, НЕ отмечай [x] сам — Claude проверит)

- `cargo test -p grammers-mtproto` зелёный.
- `cargo build -p grammers-mtsender --features mtproxy` компилируется.
- Никаких `switch_to_raw_mode`, никакой `handshake.rs` (ClientKeyExchange/Finished) в коде.
- В `docs/faketls/02-root-cause-analysis.md` описано, что raw-mode — корень зла. НЕ повторяй его.

## Feature-gate (ВАЖНО — расхождение master vs f534d9a)

На **master** у `grammers-mtproto` **НЕТ** фичи `mtproxy` (секция `[features]` в
`grammers-mtproto/Cargo.toml` отсутствует); `transport/mtproxy.rs` подключён безусловным `mod`.
Фича `mtproxy` живёт только в `grammers-mtsender`. Но восстановленные из `f534d9a` файлы
(`obfuscator.rs`) усыпаны `#[cfg(feature = "mtproxy")]`.

**Реши так (выбери одно и примени единообразно):**
- ВАРИАНТ A (рекомендуется): добавь в `grammers-mtproto/Cargo.toml` секцию
  `[features]\nmtproxy = ["subtle","aes","ctr"]` и deps `subtle/aes/ctr` с `optional=true`
  (как в f534d9a — см. секцию deps выше). Тогда `#[cfg(feature="mtproxy")]` из f534d9a заработает.
- ВАРИАНТ B: убери все `#[cfg(feature = "mtproxy")]` из восстановленных файлов и сделай deps
  `aes`/`ctr`/`subtle` обязательными. Проще, но тяжелее дефолтная сборка mtproto.

В `grammers-mtsender` фича `mtproxy` уже есть — она должна активировать mtproto-фичу:
`mtproxy = ["grammers-mtproto/mtproxy"]` (если выбрал вариант A).

## Ограничения

- Не меняй публичный API client без необходимости. Режим определяется по секрету — поле «тип прокси»
  в API НЕ добавлять (требование заказчика).
- НЕ делай `git commit` и не запускай живое подключение к 13371 — это сделает Claude.
