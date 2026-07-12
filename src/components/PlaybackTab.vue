<script setup lang="ts">
import { Palette } from 'lucide-vue-next'
</script>

<template>
  <div class="playback-tab">
    <section class="info-section">
      <div class="placeholder">
        <Palette :size="28" class="placeholder-icon" />
        <h2>Настройки внешнего вида перемещены</h2>
        <p>
          Цвет и прозрачность окна управления воспроизведением теперь
          настраиваются в разделе
          <strong>Настройки&nbsp;→&nbsp;Интерфейс</strong>.
        </p>
        <p class="placeholder-hint">
          Там же можно выбрать, наследовать ли оформление от главного окна
          или задать собственное.
        </p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.playback-tab {
  max-width: 900px;
  margin: 0 auto;
}

.info-section {
  padding: 2rem 1.5rem;
  background: var(--color-bg-field);
  border: 1px solid var(--color-border);
  border-left: 4px solid var(--color-accent);
  border-radius: 12px;
  backdrop-filter: blur(8px);
}

.placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: 0.75rem;
  color: var(--color-text-secondary);
}

.placeholder-icon {
  color: var(--color-accent);
}

.placeholder h2 {
  margin: 0;
  font-size: 1.1rem;
  color: var(--color-text-primary);
}

.placeholder p {
  margin: 0;
  font-size: 0.95rem;
  line-height: 1.6;
  max-width: 520px;
}

.placeholder-hint {
  color: var(--color-text-muted);
  font-size: 0.88rem;
}
</style>
