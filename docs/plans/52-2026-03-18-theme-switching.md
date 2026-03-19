# План 52: Переключение темы (Тёмная/Светлая)

**Дата:** 2026-03-18
**Статус:** Черновик

## Обзор

Реализовать функцию переключения между тёмной и светлой цветовой темой. Предпочтение темы будет сохраняться в `settings.json` и применяться через CSS переменные с атрибутом `data-theme`.

## Требования

- Добавить выбор темы в Настройки в новый раздел "Внешний вид"
- Поддержка двух тем: Тёмная (по умолчанию) и Светлая
- Сохранение выбора темы в `settings.json`
- Использование иконок Lucide (Moon, Sun) для индикации темы
- Применение темы через атрибут `data-theme` на элементе `:root`

## Текущее состояние

- Проект использует `lucide-vue-next` v0.577.0 для иконок
- Весь стиль использует CSS переменные из `src/styles/variables.css`
- Настройки хранятся в `src-tauri/src/config/settings.rs` → `settings.json`
- UI настроек в `src/components/SettingsPanel.vue`

## Шаги реализации

### 1. Backend: Добавить поддержку темы

**Файл:** `src-tauri/src/config/settings.rs`

Добавить `Theme` enum и поле в `AppSettings`:

```rust
// Перед определением AppSettings

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

fn default_theme() -> Theme { Theme::Dark }

// Добавить в структуру AppSettings
pub struct AppSettings {
    // ... существующие поля ...
    #[serde(default = "default_theme")]
    pub theme: Theme,
    // ... остальные поля ...
}
```

### 2. Frontend Types: Добавить тип Theme

**Файл:** `src/types/settings.ts`

```typescript
export type Theme = 'dark' | 'light';

export interface GeneralSettingsDto {
    // ... существующие поля ...
    theme?: Theme;
}
```

### 3. CSS Переменные: Определить светлую тему

**Файл:** `src/styles/variables.css`

Добавить в конец файла:

```css
/* ==================== Светлая тема ==================== */

[data-theme="light"] {
    /* RGB Цвета */
    --rgb-bg: 250, 252, 255;
    --rgb-bg-elevated: 255, 255, 255;
    --rgb-text: 15, 23, 42;
    --rgb-text-secondary: 71, 85, 105;
    --rgb-text-muted: 148, 163, 184;
    --rgb-accent: 59, 130, 246;
    --rgb-accent-strong: 37, 99, 235;
    --rgb-success: 34, 197, 94;
    --rgb-warning: 251, 191, 36;
    --rgb-danger: 239, 68, 68;
    --rgb-info: 59, 130, 246;
    --rgb-border: 226, 232, 240;
    --rgb-border-strong: 203, 213, 225;
}
```

### 4. Панель настроек: Добавить селектор темы

**Файл:** `src/components/SettingsPanel.vue`

#### 4.1 Импортировать иконки
```typescript
import { Moon, Sun } from 'lucide-vue-next';
```

#### 4.2 Добавить новый раздел в шаблон
Добавить после существующих разделов (Общие, Редактор, Логирование):

```vue
<div class="settings-section">
    <h3 class="section-title">Внешний вид</h3>

    <div class="theme-selector">
        <label
            class="theme-option"
            :class="{ active: generalSettings.theme === 'dark' }"
        >
            <input
                type="radio"
                value="dark"
                :checked="generalSettings.theme === 'dark'"
                @change="setTheme('dark')"
            />
            <Moon :size="16" />
            <span>Тёмная</span>
        </label>

        <label
            class="theme-option"
            :class="{ active: generalSettings.theme === 'light' }"
        >
            <input
                type="radio"
                value="light"
                :checked="generalSettings.theme === 'light'"
                @change="setTheme('light')"
            />
            <Sun :size="16" />
            <span>Светлая</span>
        </label>
    </div>
</div>
```

#### 4.3 Добавить метод setTheme
```typescript
const setTheme = async (theme: Theme) => {
    await invoke('update_theme', { theme });
};
```

### 5. App.vue: Применить атрибут темы

Добавить watcher для применения атрибута `data-theme`:

```typescript
watch(() => settings.value.generalSettings.theme, (newTheme) => {
    const theme = newTheme || 'dark';
    document.documentElement.setAttribute('data-theme', theme);
}, { immediate: true });
```

### 6. Tauri команда (если требуется)

**Файл:** `src-tauri/src/commands/mod.rs` (или подходящий файл команд)

```rust
#[tauri::command]
pub async fn update_theme(
    manager: State<'_, SettingsManager>,
    theme: Theme,
) -> Result<()> {
    manager.update_field("/theme", &theme)
        .context("Failed to update theme")?;
    Ok(())
}
```

## Значения цветов

| Переменная | Тёмная (текущая) | Светлая (новая) |
|------------|------------------|-----------------|
| `--rgb-bg` | 9, 11, 15 | 250, 252, 255 |
| `--rgb-bg-elevated` | 16, 19, 26 | 255, 255, 255 |
| `--rgb-text` | 244, 242, 238 | 15, 23, 42 |
| `--rgb-text-secondary` | 163, 175, 190 | 71, 85, 105 |
| `--rgb-text-muted` | 115, 129, 145 | 148, 163, 184 |
| `--rgb-accent` | 29, 140, 255 | 59, 130, 246 |
| `--rgb-accent-strong` | 0, 109, 255 | 37, 99, 235 |
| `--rgb-success` | 74, 222, 128 | 34, 197, 94 |
| `--rgb-warning` | 255, 183, 77 | 251, 191, 36 |
| `--rgb-danger` | 255, 111, 105 | 239, 68, 68 |

## Файлы для изменения

| Файл | Изменение |
|------|-----------|
| `src-tauri/src/config/settings.rs` | Добавить Theme enum + поле |
| `src/types/settings.ts` | Добавить тип Theme |
| `src/styles/variables.css` | Добавить CSS светлой темы |
| `src/components/SettingsPanel.vue` | Добавить UI селектора темы |
| `src/App.vue` | Применить data-theme атрибут |
| `src-tauri/src/commands/*.rs` | Добавить команду update_theme |

## Структура settings.json после изменений

```json
{
  "audio": { ... },
  "tts": { ... },
  "hotkey_enabled": true,
  "quick_editor_enabled": false,
  "theme": "dark",
  "twitch": { ... },
  "webview": { ... },
  "logging": { ... }
}
```

## Чек-лист для тестирования

- [ ] Тёмная тема работает (по умолчанию)
- [ ] Светлая тема переключается корректно
- [ ] Тема сохраняется после перезапуска приложения
- [ ] Все компоненты отображаются корректно в светлой теме
- [ ] Иконки (Moon/Sun) отображаются правильно
- [ ] CSS переменные применяются корректно через data-theme

## Примечания

- Плавающие окна (`src-floating`, `src-soundpanel`) используют отдельную систему стилей с выбором цветов - они вне области видимости этого плана
- Градиенты используют `color-mix()` и должны адаптироваться автоматически через CSS переменные
