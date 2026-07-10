# Задача DeepSeek: FakeTLS — Round 8 (ДИАГНОСТИЧЕСКИЕ тесты-дампы, НЕ менять логику)

**Рабочая директория:** `D:/RustProjects/grammers`. Round 1-7: handshake работает, framing работает,
но Telegram не отвечает на пересланные данные (mtg расшифровывает и пересылает 176 байт, Telegram молчит).
Статическая сверка с gotd `obfuscated2/keys.go` показывает, что derive_cipher / revert / CTR-mode СОВПАДАЮТ.
Нужен ЭМПИРИЧЕСКИЙ побайтовый дамп, чтобы найти расхождение.

## Цель

Добавить **только диагностические тесты** (НЕ менять рабочую логику obfuscator.rs/stream.rs),
которые печатают на stdout конкретные байты для фиксированных входных данных. Claude сравнит
их с эталоном gotd. Никаких правок алгоритма — только `#[test]` с `println!`/`dbg!`/`assert_eq`-hex.

## Что добавить (все в `grammers-mtproto/src/tls/obfuscator.rs`, модуль `tests`)

Сделай функции тестируемыми: если `generate_frame`/`derive_cipher`/`client_handshake` сейчас под
`#[cfg(feature="mtproxy")]` — тесты тоже под той же gate.

### Тест 1: детерминированный frame + дамп header и keystream

Сделай `generate_frame` РАЗРЕШЁННЫМ для теста с фиксированным frame (не random). Добавь helper
(только в tests), который строит frame из фиксированных байт:

```rust
#[cfg(feature = "mtproxy")]
#[test]
fn dump_client_handshake_vectors() {
    // Фиксированный секрет (16 байт) — тот же, что в gotd-эталоне.
    let secret: [u8; 16] = [0x42u8; 16];

    // Фиксированный "init"-frame (64 байт), валидный по is_frame_valid.
    // Заполни детерминированно: [0..8]=заголовок (не 0xef, не forbidden),
    // [8..56]=key+iv (ненулевые), [56..60]=connection_type, [60..62]=dc=2, [62..64]=padding.
    let mut frame = [0u8; 64];
    for i in 0..64 { frame[i] = ((i as u8).wrapping_mul(7)).wrapping_add(1); }
    // обеспечить валидность: frame[0] != 0xef; frame[4..8] != 0; first4 not forbidden
    frame[0] = 0x11; // не 0xef
    // connection type + dc как в generate_frame:
    frame[56..60].copy_from_slice(&CONNECTION_TYPE);
    frame[60..62].copy_from_slice(&(2i16).to_le_bytes());

    // Воспроизвести client_handshake НА ЭТОМ frame (вынеси логику в тестируемую ф-ю если надо):
    let mut send_cipher = derive_cipher(&frame, &secret);
    let mut frame_reverted = frame;
    revert_key_iv(&mut frame_reverted);
    let recv_cipher = derive_cipher(&frame_reverted, &secret);

    let mut encrypted = frame;
    send_cipher.apply_keystream(&mut encrypted);
    encrypted[0..56].copy_from_slice(&frame[0..56]); // restore plaintext (round 5)

    // ДАМП (Claude сравнит с gotd):
    eprintln!("DUMP secret = {:02x?}", secret);
    eprintln!("DUMP frame_plain = {}", hex::encode(frame));
    eprintln!("DUMP header_sent = {}", hex::encode(&encrypted)); // 64 байта на провод
    eprintln!("DUMP header[0:8]  = {} (plain)", hex::encode(&encrypted[0..8]));
    eprintln!("DUMP header[8:56] = {} (plain)", hex::encode(&encrypted[8..56]));
    eprintln!("DUMP header[56:64]= {} (encrypted)", hex::encode(&encrypted[56..64]));

    // send_cipher keystream: применить к нулям → сам keystream (cipher уже сдвинут на 64).
    let mut ks_send = [0u8; 32];
    let mut tmp_send = Aes256Ctr::new(/*re-derive чтобы получить свежий? НЕТ*/ ); // см ниже
    // ВАЖНО: send_cipher уже сдвинут на 64 (применён к frame). Продемонстрируй keystream ПОСЛЕ init:
    let mut probe = [0u8; 16];
    send_cipher.apply_keystream(&mut probe);
    eprintln!("DUMP send_keystream_after_init[64:80] = {}", hex::encode(probe));

    // recv_cipher — свежий (counter=0). Его keystream:
    let mut recv_probe = [0u8; 16];
    // recv_cipher уже создан выше; продемонстрируй его keystream с 0:
    // (если recv_cipher нельзя применить к probe без mutable borrow — сделай копию логики)
    eprintln!("DUMP recv_keystream[0:16] = <см gotd decrypt-cipher с этим же init>");
}
```

КРИТИЧНО: тест должен **компилироваться и печатать**. Если mutable-borrow проблемы — реорганизуй
(создай recv_cipher заново для probe). Главное — детерминированный вывод.

### Тест 2: дамп SHA256(key‖secret) для фиксированных key/secret

```rust
#[test]
fn dump_derive_key() {
    let key = [0xAAu8; 32];
    let secret = [0x42u8; 16];
    let mut h = Sha256::new();
    h.update(&key);
    h.update(&secret);
    eprintln!("DUMP SHA256(key=0xAA*32 ‖ secret=0x42*16) = {}", hex::encode(h.finalize()));
}
```

## Запуск (Claude)

```bash
cargo test -p grammers-mtproto --features mtproxy -- dump_client_handshake_vectors dump_derive_key --nocapture
```
Claude возьмёт отпечатки DUMP и сравнит с gotd-эталоном (Claude сам напишет gotd-тест с теми же
фиксированными secret=0x42*16 и frame).

## Не трогать
- Логику obfuscator.rs (derive_cipher/revert/client_handshake/Aes256Ctr) — НЕ менять.
- stream.rs, server_hello.rs, framing.rs — НЕ менять.
- Только ДОБАВИТЬ диагностические тесты.

Если в ходе написания теста найдёшь, что какая-то функция недоступна из tests (private) — вынеси
нужный helper в `pub(crate)` или `#[cfg(test)]`-доступ, но логику не меняй.

## Критерий
Тесты компилируются, печатают DUMP-строки. `cargo test` остаётся зелёным.
НЕ коммить, живые тесты не запускать.
