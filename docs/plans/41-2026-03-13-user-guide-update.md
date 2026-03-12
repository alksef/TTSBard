# План #41: Обновление документации — Автозамена текста

## Обзор
1. **Добавить в `docs/README.md`** информацию о префиксах `!` и `!!`
2. **Добавить в `InfoPanel.vue`** блок об обработке текста с описанием всех функций

## Текущее состояние
- ❌ `docs/README.md` не содержит информации о префиксах `!`, `!!`
- ❌ `InfoPanel.vue` не описывает функции обработки текста

## Что добавить

### 1. В docs/README.md — новый раздел после "Горячие клавиши":

```markdown
## Автозамена текста

### Управление отправкой

Используйте префиксы в начале текста для управления отправкой в Twitch и WebView:

| Префикс | Описание | Пример | Twitch | WebView | TTS |
|---------|----------|--------|--------|---------|-----|
| Без префикса | Отправить везде | `Привет всем` | ✅ | ✅ | ✅ |
| `!` | Только WebView | `!Привет` | ❌ | ✅ | ✅ |
| `!!` | Только TTS | `!!Привет` | ❌ | ❌ | ✅ |

**Примеры использования:**
```
!Только для OBS — озвучить, но не в Twitch
!!Локальное сообщение — только для TTS
Обычный текст — отправить везде
```

### Быстрая вставка

- **Список замен** — используйте `\ключ` для замены фраз (настраивается в панели "Автозамена")
- **Список юзернеймов** — используйте `%юзернейм` для замены имён пользователей

### Конвертация чисел

Числа автоматически преобразуются в текст с согласованием рода следующего слова:

| Ввод | Результат |
|------|-----------|
| `У меня 5 яблок` | `У меня пять яблок` |
| `1 книга` | `одна книга` |
| `2 книги` | `две книги` |
| `5 книг` | `пять книг` |
| `123` | `сто двадцать три` |
| `-10` | `минус десять` |

**Правила:**
- Числа `1` и `2-4` согласуются с родом следующего слова
- Числа `5+` используют множественное число
```

### 2. В InfoPanel.vue — новый блок "Обработка текста":

Добавить после существующего контента, перед закрывающим тегом:

```vue
<!-- В script setup нужно добавить импорт иконки -->
import { ClipboardPenLine } from 'lucide-vue-next'

<section class="info-section preprocessor-section">
  <h2><ClipboardPenLine :size="18" /> Обработка текста</h2>

  <div class="info-block">
    <h3>Управление отправкой</h3>
    <p>Используйте префиксы для контроля маршрутизации текста:</p>
    <table class="features-table">
      <tr>
        <th>Префикс</th>
        <th>Описание</th>
        <th>Twitch</th>
        <th>WebView</th>
      </tr>
      <tr>
        <td>Нет</td>
        <td>Отправить везде</td>
        <td>✅</td>
        <td>✅</td>
      </tr>
      <tr>
        <td><code>!</code></td>
        <td>Только WebView</td>
        <td>❌</td>
        <td>✅</td>
      </tr>
      <tr>
        <td><code>!!</code></td>
        <td>Только TTS</td>
        <td>❌</td>
        <td>❌</td>
      </tr>
    </table>
  </div>

  <div class="info-block">
    <h3>Быстрая вставка</h3>
    <p><strong>Список замен</strong> — используйте <code>\ключ</code> для замены фраз</p>
    <p><strong>Список юзернеймов</strong> — используйте <code>%юзернейм</code> для замены имён</p>
  </div>

  <div class="info-block">
    <h3>Конвертация чисел</h3>
    <p>Числа автоматически преобразуются в текст с согласованием рода:</p>
    <ul>
      <li><code>1 книга</code> → <code>одна книга</code></li>
      <li><code>2 книги</code> → <code>две книги</code></li>
      <li><code>5 книг</code> → <code>пять книг</code></li>
    </ul>
  </div>
</section>
```

### Стили для InfoPanel.vue (добавить к существующим):

```css
.preprocessor-section {
  margin-top: 2rem;
}

.preprocessor-section h2 {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.preprocessor-section h2 svg {
  flex-shrink: 0;
}

.info-block {
  margin-bottom: 1.5rem;
}

.info-block:last-child {
  margin-bottom: 0;
}

.info-block h3 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--color-text-primary);
  margin-bottom: 0.75rem;
}

.info-block p {
  margin-bottom: 0.5rem;
}

.features-table {
  width: 100%;
  border-collapse: collapse;
  margin: 1rem 0;
}

.features-table th,
.features-table td {
  padding: 0.5rem 0.75rem;
  text-align: left;
  border-bottom: 1px solid rgba(255, 255, 255, 0.08);
}

.features-table th {
  font-weight: 600;
  color: var(--color-text-primary);
}

.features-table td {
  color: var(--color-text-secondary);
}

.info-block code {
  background: rgba(29, 140, 255, 0.15);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
  color: var(--color-info);
  border: 1px solid rgba(29, 140, 255, 0.28);
}

.info-block ul {
  margin: 0;
  padding-left: 1.25rem;
}

.info-block li {
  margin-bottom: 0.4rem;
  color: var(--color-text-secondary);
}
```

## Сводка файлов

| Файл | Действие |
|------|----------|
| `docs/README.md` | Добавить раздел "Автозамена текста" |
| `src/components/InfoPanel.vue` | Добавить блок "Обработка текста" |
| `docs/plans/counter.txt` | Инкремент на 41 |

## Задачи

- [ ] Добавить раздел "Автозамена текста" в docs/README.md
  - [ ] Управление отправкой (префиксы `!`, `!!`)
  - [ ] Быстрая вставка (`\ключ`, `%юзернейм`)
  - [ ] Конвертация чисел (примеры)
- [ ] Добавить блок "Обработка текста" в InfoPanel.vue
  - [ ] Импорт иконки ClipboardPenLine
  - [ ] Заголовок с иконкой
  - [ ] Управление отправкой (таблица)
  - [ ] Быстрая вставка (описание)
  - [ ] Конвертация чисел (примеры)
  - [ ] Добавить стили для нового блока
- [ ] Обновить счётчик планов

## Примечания

1. В docs/README.md добавляем **три подраздела**: префиксы, быстрая вставка, конвертация чисел
2. В InfoPanel.vue добавляем **полный блок** со всей информацией об обработке текста
3. Используем иконку **ClipboardPenLine** (как в сайдбаре у "Автозамена")
4. Не меняем название панели "Препроцессор" в UI
5. Используем термин "Обработка текста" для пользователей

## Структура блока в InfoPanel.vue

```
Обработка текста (ClipboardPenLine icon)
├── Управление отправкой (таблица с ! и !!)
├── Быстрая вставка (\ключ и %юзернейм)
└── Конвертация чисел (примеры)
```
