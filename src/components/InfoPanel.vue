<script setup lang="ts">
import { Globe, Zap, Music, Lightbulb, ClipboardPenLine, Keyboard } from 'lucide-vue-next';
// Info panel - static content with hotkeys and presets documentation
</script>

<template>
  <div class="info-panel">
    <!-- Глобальные горячие клавиши -->
    <section class="info-section">
      <h2><Globe :size="18" class="section-icon" /> Глобальные горячие клавиши</h2>

      <h3>Фиксированные</h3>
      <div class="hotkey-list">
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F1</kbd>
          <span>Включить режим перехвата текста / Показать плавающее окно</span>
        </div>
      </div>

      <h3>Настраиваемые (значения по умолчанию)</h3>
      <div class="hotkey-list">
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F2</kbd>
          <span>Показать звуковую панель</span>
        </div>
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F3</kbd>
          <span>Показать главное окно (поверх всех, для фокуса)</span>
        </div>
      </div>

      <p class="hint">
        <Keyboard :size="14" /> Настройка горячих клавиш доступна в разделе «Горячие клавиши»
      </p>
    </section>

    <!-- Обработка текста -->
    <section class="info-section">
      <h2><ClipboardPenLine :size="18" class="section-icon" /> Обработка текста</h2>

      <h3>Управление отправкой</h3>
      <p class="section-description">
        Используйте префиксы для контроля маршрутизации текста:
      </p>
      <table class="features-table">
        <thead>
          <tr>
            <th>Префикс</th>
            <th>Описание</th>
            <th>Twitch</th>
            <th>WebView</th>
          </tr>
        </thead>
        <tbody>
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
        </tbody>
      </table>

      <h3>Быстрая вставка</h3>
      <p class="section-description">
        <strong>Список замен</strong> — используйте <code>\ключ</code> для замены фраз<br>
        <strong>Список юзернеймов</strong> — используйте <code>%юзернейм</code> для замены имён
      </p>

      <h3>Конвертация чисел</h3>
      <p class="section-description">
        Числа автоматически преобразуются в текст с согласованием рода:
      </p>

      <div class="example-block">
        <div class="example-item"><code>1 книга</code> → <code>одна книга</code></div>
        <div class="example-item"><code>2 книги</code> → <code>две книги</code></div>
        <div class="example-item"><code>5 книг</code> → <code>пять книг</code></div>
      </div>
    </section>

    <!-- Перехват текста -->
    <section class="info-section">
      <h2><Zap :size="18" class="section-icon" /> Режим перехвата текста</h2>
      <p class="section-description">
        Режим перехвата позволяет перехватывать ввод с клавиатуры в реальном времени
        и отправлять текст в TTS. Активируется нажатием <code>Ctrl+Shift+F1</code>.
      </p>

      <h3>Горячие клавиши в режиме перехвата</h3>
      <div class="hotkey-list">
        <div class="hotkey-item">
          <kbd>F8</kbd>
          <span>Переключить раскладку клавиатуры (EN ↔ RU)</span>
        </div>
        <div class="hotkey-item">
          <kbd>F6</kbd>
          <span>Переключить режим: Enter не закрывает окно / Enter закрывает окно</span>
        </div>
        <div class="hotkey-item">
          <kbd>Enter</kbd>
          <span>Отправить текст в TTS (закрыть окно, если режим F6 выключен)</span>
        </div>
        <div class="hotkey-item">
          <kbd>Escape</kbd>
          <span>Очистить текст, отключить перехват, скрыть окно</span>
        </div>
        <div class="hotkey-item">
          <kbd>Backspace</kbd>
          <span>Удалить последний символ</span>
        </div>
        <div class="hotkey-item">
          <kbd>Space</kbd>
          <span>Добавить пробел</span>
        </div>
      </div>
    </section>

    <!-- Звуковая панель -->
    <section class="info-section">
      <h2><Music :size="18" class="section-icon" /> Звуковая панель (Саунпад)</h2>
      <p class="section-description">
        Позволяет привязать аудиофайлы к клавишам для мгновенного воспроизведения.
        Поддерживается до 26 привязок (по одной на каждую букву A-Z).
      </p>

      <h3>Горячие клавиши</h3>
      <div class="hotkey-list">
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F2</kbd>
          <span>Показать/скрыть звуковую панель (по умолчанию, настраивается)</span>
        </div>
      </div>

      <h3>Привязанные клавиши</h3>
      <p class="subsection-description">
        Когда звуковая панель видна, нажатие клавиш <kbd>A</kbd>—<kbd>Z</kbd>
        воспроизводит соответствующие звуки.
      </p>

      <div class="soundpad-grid">
        <div v-for="i in 26" :key="i" class="key-badge">
          {{ String.fromCharCode(64 + i) }}
        </div>
      </div>

      <h3>Поддерживаемые форматы</h3>
      <div class="format-list">
        <span class="format-badge">MP3</span>
        <span class="format-badge">WAV</span>
        <span class="format-badge">OGG</span>
        <span class="format-badge">FLAC</span>
      </div>

      <p class="hint">
        <Lightbulb :size="14" /> Звуковая панель настраивается в разделе «Звуковая панель»
      </p>
    </section>

    <!-- Дополнительно -->
    <section class="info-section">
      <h2><Lightbulb :size="18" class="section-icon" /> Дополнительная информация</h2>
      <p class="section-description">
        Все настройки сохраняются автоматически между запусками
      </p>
    </section>
  </div>
</template>

<style scoped>
.info-panel {
  max-width: 900px;
  margin: 0 auto;
}

.info-section {
  margin-bottom: 1.5rem;
  padding: 12px 16px;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.info-section h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.1rem;
  color: var(--color-text-primary);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.section-icon {
  color: var(--color-accent);
  flex-shrink: 0;
}

.info-section h3 {
  margin-top: 1rem;
  margin-bottom: 0.5rem;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

.info-section h2 + h3 {
  margin-top: 0.5rem;
}

.section-description {
  color: var(--color-text-secondary);
  line-height: 1.6;
  margin-bottom: 0.5rem;
}

.section-description code {
  background: var(--color-accent-glow);
  color: var(--color-info);
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.9rem;
  border: 1px solid var(--info-border);
}

.subsection-description {
  color: var(--color-text-secondary);
  line-height: 1.6;
  margin-bottom: 0.75rem;
}

.hotkey-list {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.hotkey-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.hotkey-item kbd {
  background: var(--kbd-bg);
  border: 1px solid var(--color-border-strong);
  border-bottom: 2px solid var(--color-border);
  border-radius: 6px;
  padding: 0.3rem 0.6rem;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--color-text-primary);
  box-shadow: 0 2px 4px var(--kbd-shadow);
}

.hotkey-item span {
  color: var(--color-text-secondary);
  flex: 1;
}

.preset-list {
  background: var(--output-bg-dark);
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid var(--color-border);
}

.preset-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
}

.preset-item:last-child {
  margin-bottom: 0;
}

.preset-item code {
  background: var(--color-accent-glow);
  color: var(--color-info);
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.9rem;
  border: 1px solid var(--info-border);
}

.preset-item span {
  color: var(--color-text-secondary);
}

.preset-example {
  margin-left: 1.5rem;
  font-size: 0.9rem;
  color: var(--color-text-secondary);
  padding: 0.6rem;
  background: var(--color-bg-field);
  border-radius: 6px;
  margin-top: 0.5rem;
  border: 1px solid var(--color-border-weak);
}

.preset-example code {
  background: var(--color-accent-glow);
  color: var(--color-info);
  padding: 0.15rem 0.35rem;
  border-radius: 3px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  border: 1px solid var(--info-border);
}

.preset-example em {
  color: var(--success-text-bright);
  font-style: normal;
  font-weight: 500;
}

.hint {
  margin-top: 1rem;
  padding: 0.75rem;
  background: var(--warning-bg-weak);
  border-left: 4px solid var(--warning-border-solid);
  border-radius: 6px;
  color: var(--color-warning);
  font-size: 0.9rem;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.soundpad-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(40px, 1fr));
  gap: 0.5rem;
  margin-top: 0.75rem;
}

.key-badge {
  background: var(--kbd-bg);
  border: 1px solid var(--color-border-strong);
  border-bottom: 2px solid var(--color-border);
  border-radius: 6px;
  padding: 0.5rem;
  text-align: center;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.9rem;
  color: var(--color-text-secondary);
}

.format-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.format-badge {
  background: var(--success-bg-weak);
  color: var(--success-text-bright);
  padding: 0.4rem 0.8rem;
  border-radius: 6px;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.9rem;
  border: 1px solid var(--success-shadow);
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
  border-bottom: 1px solid var(--color-border);
}

.features-table th {
  font-weight: 600;
  color: var(--color-text-primary);
}

.features-table td {
  color: var(--color-text-secondary);
}

.info-block code {
  background: var(--color-accent-glow);
  padding: 0.15rem 0.35rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.85em;
  color: var(--color-info);
  border: 1px solid var(--info-border);
}

.info-block ul {
  margin: 0;
  padding-left: 1.25rem;
}

.info-block li {
  margin-bottom: 0.4rem;
  color: var(--color-text-secondary);
}

.example-block {
  background: var(--output-bg-dark);
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid var(--color-border);
}

.example-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.75rem;
  font-size: 0.9rem;
  color: var(--color-text-secondary);
}

.example-item:last-child {
  margin-bottom: 0;
}

.example-item code {
  background: var(--color-accent-glow);
  color: var(--color-info);
  padding: 0.2rem 0.5rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.9rem;
  border: 1px solid var(--info-border);
}
</style>
