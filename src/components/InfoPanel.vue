<script setup lang="ts">
// Info panel - static content with hotkeys and presets documentation
</script>

<template>
  <div class="info-panel">
    <!-- Глобальные горячие клавиши -->
    <section class="info-section">
      <h2>🌐 Глобальные горячие клавиши</h2>
      <div class="hotkey-list">
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F1</kbd>
          <span>Включить режим перехвата текста / Показать плавающее окно</span>
        </div>
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F2</kbd>
          <span>Показать звуковую панель</span>
        </div>
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F3</kbd>
          <span>Показать главное окно (поверх всех, для фокуса)</span>
        </div>
      </div>
    </section>

    <!-- Перехват текста -->
    <section class="info-section">
      <h2>⚡ Режим перехвата текста</h2>
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
          <span>Добавить пробел (с автозаменой пресетов)</span>
        </div>
      </div>

      <h3>Пресеты препроцессора</h3>
      <p class="subsection-description">
        Пресеты позволяют автоматически заменять короткие коды на полный текст при вводе.
        Замена происходит мгновенно после нажатия пробела.
      </p>

      <div class="preset-list">
        <div class="preset-item">
          <code>\ключ</code>
          <span>Замена текста по коду</span>
        </div>
        <div class="preset-example">
          Пример: <code>\name</code> + пробел → <em>Александр</em>
        </div>

        <div class="preset-item">
          <code>%юзернейм</code>
          <span>Замена имени пользователя</span>
        </div>
        <div class="preset-example">
          Пример: <code>%admin</code> + пробел → <em>Администратор</em>
        </div>
      </div>

      <p class="hint">
        💡 Пресеты настраиваются в разделе «🔧 Препроцессор»
      </p>
    </section>

    <!-- Звуковая панель -->
    <section class="info-section">
      <h2>🎵 Звуковая панель (Саунпад)</h2>
      <p class="section-description">
        Позволяет привязать аудиофайлы к клавишам для мгновенного воспроизведения.
        Поддерживается до 26 привязок (по одной на каждую букву A-Z).
      </p>

      <h3>Горячие клавиши</h3>
      <div class="hotkey-list">
        <div class="hotkey-item">
          <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>F2</kbd>
          <span>Показать/скрыть звуковую панель</span>
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
        💡 Звуковая панель настраивается в разделе «🎵 Звуковая панель»
      </p>
    </section>

    <!-- Дополнительно -->
    <section class="info-section">
      <h2>💡 Дополнительная информация</h2>
      <ul class="info-list">
        <li>Все настройки сохраняются автоматически между запусками</li>
        <li>Плавающее окно можно сделать прозрачным и пропускающим клики</li>
        <li>Главное окно при вызове через <code>Ctrl+Shift+F3</code> становится поверх всех только для фокуса</li>
        <li>После потери фокуса главное окно возвращается в нормальное состояние</li>
        <li>TTS провайдеры: OpenAI, Silero Bot, TTSVoiceWizard</li>
        <li>Аудиоустройства можно настраивать independently (вывод и виртуальный микрофон)</li>
      </ul>
    </section>
  </div>
</template>

<style scoped>
.info-panel {
  max-width: 900px;
  margin: 0 auto;
}

.info-section {
  margin-bottom: 2rem;
  padding: 1.5rem;
  background: rgba(255, 255, 255, 0.03);
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.info-section h2 {
  margin-top: 0;
  margin-bottom: 1rem;
  font-size: 1.5rem;
  color: var(--color-text-primary);
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.info-section h3 {
  margin-top: 1.5rem;
  margin-bottom: 0.75rem;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

.section-description {
  color: var(--color-text-secondary);
  line-height: 1.6;
  margin-bottom: 1rem;
}

.section-description code {
  background: rgba(29, 140, 255, 0.15);
  color: #70d6ff;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-size: 0.9rem;
  border: 1px solid rgba(29, 140, 255, 0.3);
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
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-bottom: 2px solid rgba(255, 255, 255, 0.2);
  border-radius: 6px;
  padding: 0.3rem 0.6rem;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.85rem;
  color: var(--color-text-primary);
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.2);
}

.hotkey-item span {
  color: var(--color-text-secondary);
  flex: 1;
}

.preset-list {
  background: rgba(0, 0, 0, 0.25);
  padding: 1rem;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.08);
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
  background: rgba(29, 140, 255, 0.15);
  color: #70d6ff;
  padding: 0.3rem 0.6rem;
  border-radius: 4px;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.9rem;
  border: 1px solid rgba(29, 140, 255, 0.3);
}

.preset-item span {
  color: var(--color-text-secondary);
}

.preset-example {
  margin-left: 1.5rem;
  font-size: 0.9rem;
  color: var(--color-text-secondary);
  padding: 0.6rem;
  background: rgba(255, 255, 255, 0.03);
  border-radius: 6px;
  margin-top: 0.5rem;
  border: 1px solid rgba(255, 255, 255, 0.06);
}

.preset-example code {
  background: rgba(29, 140, 255, 0.12);
  color: #70d6ff;
  padding: 0.15rem 0.35rem;
  border-radius: 3px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  border: 1px solid rgba(29, 140, 255, 0.25);
}

.preset-example em {
  color: #4ade80;
  font-style: normal;
  font-weight: 500;
}

.hint {
  margin-top: 1rem;
  padding: 0.75rem;
  background: rgba(255, 152, 0, 0.1);
  border-left: 4px solid #ff9800;
  border-radius: 6px;
  color: #ffb74d;
  font-size: 0.9rem;
}

.soundpad-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(40px, 1fr));
  gap: 0.5rem;
  margin-top: 0.75rem;
}

.key-badge {
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
  border-bottom: 2px solid rgba(255, 255, 255, 0.2);
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
  background: rgba(74, 222, 128, 0.12);
  color: #4ade80;
  padding: 0.4rem 0.8rem;
  border-radius: 6px;
  font-family: var(--font-mono);
  font-weight: 600;
  font-size: 0.9rem;
  border: 1px solid rgba(74, 222, 128, 0.25);
}

.info-list {
  margin: 0;
  padding-left: 1.5rem;
  color: var(--color-text-secondary);
  line-height: 1.8;
}

.info-list li {
  margin-bottom: 0.5rem;
}

.info-list code {
  background: rgba(29, 140, 255, 0.12);
  color: #70d6ff;
  padding: 0.15rem 0.35rem;
  border-radius: 3px;
  font-family: var(--font-mono);
  font-size: 0.85rem;
  border: 1px solid rgba(29, 140, 255, 0.25);
}
</style>
