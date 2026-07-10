# Plan 116: Атомарная и упорядоченная запись истории и вкладок (history.rs & tabs.rs)

**Дата:** 2026-07-11  
**Статус:** Запланировано к реализации  
**Источник:** review-001-2026-07-11 (MINOR)

---

## 1. Проблема
В `src-tauri/src/history.rs` и `src-tauri/src/tabs.rs` для сохранения файлов на диск используется паттерн `std::thread::spawn(move || { fs::write(...) })`.
Это порождает следующие риски:
1. **Гонка записи (Out-of-order writes):** Быстрый поток команд (например, быстрое переключение/редактирование вкладок или частый запуск TTS) порождает несколько фоновых потоков ОС. Старая запись может завершиться позже новой и затереть актуальные данные.
2. **Неатомарная запись:** `fs::write` пишет файл напрямую. Если во время записи произойдёт краш приложения или выключение ПК, файл окажется повреждённым/пустым.

---

## 2. Предлагаемое решение

Так как сохранение истории и вкладок происходит относительно редко (по действию пользователя или запуску фразы) и объём файлов невелик (JSON-файлы истории и вкладок редко превышают сотни КБ), мы переведём операции записи на **синхронные атомарные вызовы** под общей блокировкой `Mutex`.

Для этого:
1. Создадим/используем общий вспомогательный метод атомарной записи файла (через создание временного файла, `sync_all` и `fs::rename`) — такой же, какой используется в `settings.rs`.
2. Уберём `std::thread::spawn` из `history.rs` и `tabs.rs`.
3. Все операции записи файлов истории (`history.json`, `ngrams.json`, `phrases.json`) и вкладок (`tabs.json`) будут проходить синхронно в рамках вызовов методов менеджера под блокировкой мьютекса записи.

---

## 3. Детали изменений

### 3.1 Общий метод атомарной записи
Поскольку в `history.rs` и `tabs.rs` нет прямого доступа к функциям `settings.rs`, мы добавим локальные (или вынесем) функции атомарной записи файлов.
В `history.rs` определим `write_json_atomically` и мьютекс сериализации записи.
В `tabs.rs` сделаем то же самое.

### 3.2 Изменения в `history.rs`
1. Добавить static lock для сериализации записи на диск:
   ```rust
   use std::sync::OnceLock;
   use parking_lot::Mutex as ParkingMutex;
   static HISTORY_WRITE_LOCK: OnceLock<ParkingMutex<()>> = OnceLock::new();
   fn history_write_lock() -> &'static ParkingMutex<()> {
       HISTORY_WRITE_LOCK.get_or_init(|| ParkingMutex::new(()))
   }
   ```
2. Убрать `spawn_save` и `spawn_save_phrases`.
3. Реализовать `write_json_atomically(path: &Path, content: &str)` (создание `.tmp` файла, запись, `sync_all`, `fs::rename`).
4. В `HistoryManager` в методах `record_text`, `clear_history`, `delete_phrase` и др. вызывать `write_json_atomically` под блокировкой `history_write_lock().lock()`.

### 3.3 Изменения в `tabs.rs`
1. Добавить static lock:
   ```rust
   use std::sync::OnceLock;
   use parking_lot::Mutex as ParkingMutex;
   static TABS_WRITE_LOCK: OnceLock<ParkingMutex<()>> = OnceLock::new();
   fn tabs_write_lock() -> &'static ParkingMutex<()> {
       TABS_WRITE_LOCK.get_or_init(|| ParkingMutex::new(()))
   }
   ```
2. Убрать `spawn_save` и заменить на синхронную запись `write_json_atomically` под блокировкой `tabs_write_lock().lock()`.

---

## 4. Ожидаемый результат
* Исключаются гонки записи на стыке потоков ОС.
* Гарантируется целостность файлов (если запись не удалась, старый файл остаётся нетронутым).
* Тесты в `history.rs` и `tabs.rs` избавляются от необходимости искусственных `thread::sleep` при тестировании сохранения.
