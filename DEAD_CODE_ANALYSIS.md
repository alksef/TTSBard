# Dead Code Analysis

**Дата:** 2026-04-12
**Коммит удаления:** `cfe0b10 feat: remove floating window (text interception)`
**Статус:** ✅ УДАЛЕНИЕ ЗАВЕРШЕНО

---

## История

Коммит `cfe0b10` удалил:
- `src-floating/` (UI floating window)
- `src-tauri/src/hook.rs` (backend хуки для клавиатуры)
- `src-tauri/src/floating.rs` (управление floating окном)

Все методы ниже **использовались только в удалённом `hook.rs`**.

---

## Удалённый код

### `src-tauri/src/state.rs`

✅ **Удалены поля:**
- `current_text` - Текст из floating окна
- `current_layout` - Раскладка EN/RU

✅ **Удалены методы:**
| Метод | Описание |
|-------|----------|
| `get_current_text()` | Чтение текста |
| `set_current_text()` | Установка текста |
| `append_text()` | Добавление символа |
| `remove_last_char()` | Удаление символа |
| `clear_text()` | Очистка текста |
| `get_current_layout()` | Текущая раскладка |
| `toggle_layout()` | Переключение EN/RU |
| `get_active_window()` | Активное окно |
| `is_soundpanel_active()` | Проверка soundpanel |
| `get_openai_voice()` | Получение голоса OpenAI |
| `get_openai_proxy_url()` | Получение proxy URL |
| `refresh_devices()` | Обновление списка устройств |

✅ **Удалены неиспользуемые импорты:**
- `InputLayout` из `crate::events`
- `AppHandle`, `Manager` из `tauri`
- `HostTrait`, `DeviceTrait` из `cpal::traits`

### `src-tauri/src/preprocessor/replacer.rs`

✅ **Удалены методы:**
| Метод | Описание |
|-------|----------|
| `save_usernames_to_file()` | Сохранение usernames в файл |
| `check_and_replace_end()` | Live replacement при вводе |
| `reload()` | Перезагрузка replacements |

✅ **Удалены неиспользуемые regex паттерны:**
- `REPLACEMENT_PATTERN_END` - для `check_and_replace_end`
- `USERNAME_PATTERN_END` - для `check_and_replace_end`

✅ **Удалены тесты:**
- `test_replacement_end_pattern`
- `test_username_end_pattern`
- `test_check_and_replace_end`
- `test_check_and_replace_end_no_match`

✅ **Исправлено:**
- `save_to_file()` помечен как `#[cfg(test)]` (только для тестов)
- Убран `#[allow(dead_code)]` из `get_replacements_map()` (используется)

---

## Результат

**Всего удалено:**
- 2 поля
- 13 методов в `state.rs`
- 3 метода в `replacer.rs`
- 2 regex паттерна
- 4 теста
- 4 неиспользуемых импорта

**Компиляция:** ✅ Clean, no warnings
**Тесты:** ✅ Все проходят (4 passed)

---

## Дата завершения
2026-04-12
