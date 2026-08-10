<script setup lang="ts">
import { ref, watch } from 'vue';
import { FileAudio, Upload, Play, AudioLines, Square, FolderOpen, X } from 'lucide-vue-next';

interface FileInfo {
  path: string;
  name: string;
  size: number;
}

const props = defineProps<{
  selectedFile: FileInfo | null;
  fileFormat: string;
  isPreviewPlaying: boolean;
  previewError: string;
}>();

const emit = defineEmits<{
  (e: 'pickFile'): void;
  (e: 'replaceFile'): void;
  (e: 'clearFile'): void;
  (e: 'playPreview', mode: 'original' | 'effects'): void;
  (e: 'stopPreview'): void;
}>();

const playingMode = ref<'original' | 'effects' | null>(null);

function onPlay(mode: 'original' | 'effects') {
  playingMode.value = mode;
  emit('playPreview', mode);
}

function onStop() {
  playingMode.value = null;
  emit('stopPreview');
}

watch(() => props.isPreviewPlaying, (val) => {
  if (!val) playingMode.value = null;
});
</script>

<template>
  <div class="preview-bar" :class="{ 'has-error': !!previewError }">
    <div class="bar-main" :class="{ 'bar-main--empty': !selectedFile }">
      <template v-if="!selectedFile">
        <div class="info-group">
          <FileAudio class="bar-icon" :size="18" />
          <span class="bar-title">Проверить эффекты</span>
        </div>
        <button @click="emit('pickFile')" class="pick-btn">
          <Upload :size="16" />
          <span>Выбрать файл</span>
        </button>
      </template>

      <template v-else>
        <div class="info-group">
          <span class="file-name" :title="selectedFile.name">{{ selectedFile.name }}</span>
          <span class="format-badge">{{ fileFormat }}</span>
        </div>
        <div class="controls-group">
          <button
            @click="onPlay('original')"
            :disabled="isPreviewPlaying"
            :class="{ 'playing-original': playingMode === 'original' && isPreviewPlaying }"
            class="play-btn"
            title="Воспроизвести оригинал без эффектов"
            aria-label="Воспроизвести оригинал"
          >
            <Play :size="16" />
            <span class="btn-label">Оригинал</span>
          </button>
          <button
            @click="onPlay('effects')"
            :disabled="isPreviewPlaying"
            :class="{ 'playing-effects': playingMode === 'effects' && isPreviewPlaying }"
            class="play-btn"
            title="Воспроизвести со всеми эффектами и DSP"
            aria-label="Воспроизвести со всеми эффектами"
          >
            <AudioLines :size="16" />
            <span class="btn-label">С эффектами</span>
          </button>
          <button
            @click="onStop"
            :disabled="!isPreviewPlaying"
            class="icon-btn stop-btn"
            title="Остановить воспроизведение"
            aria-label="Остановить воспроизведение"
          >
            <Square :size="16" />
          </button>
          <button
            @click="emit('replaceFile')"
            class="icon-btn"
            title="Заменить файл"
            aria-label="Заменить файл"
          >
            <FolderOpen :size="16" />
          </button>
          <button
            @click="emit('clearFile')"
            class="icon-btn"
            title="Очистить выбранный файл"
            aria-label="Очистить выбранный файл"
          >
            <X :size="16" />
          </button>
        </div>
      </template>
    </div>

    <div class="error-line" :title="previewError || undefined">
      <span class="error-text">{{ previewError }}</span>
    </div>
  </div>
</template>

<style scoped>
.preview-bar {
  padding: 0;
}

.bar-main {
  display: flex;
  align-items: center;
  gap: 8px;
  min-height: 36px;
}

.bar-main--empty {
  display: grid;
  grid-template-columns: 1fr auto 1fr;
}

.bar-main--empty .info-group {
  grid-column: 2;
  transform: translateX(-12px);
}

.bar-main--empty .pick-btn {
  grid-column: 3;
  justify-self: end;
}

.info-group {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
  flex: 1;
}

.bar-icon {
  flex-shrink: 0;
  color: var(--color-text-secondary);
}

.bar-title {
  font-size: 14px;
  color: var(--color-text-secondary);
  white-space: nowrap;
}

.pick-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex-shrink: 0;
  padding: 6px 14px;
  background: var(--btn-accent-bg);
  border: 1px solid var(--color-accent);
  color: var(--color-text-primary);
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  font-family: inherit;
  transition: all 0.15s;
}

.pick-btn:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.pick-btn:focus-visible,
.play-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}

.file-name {
  margin-left: 8px;
  font-size: 14px;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.format-badge {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--color-text-muted);
  background: var(--color-bg-field-hover);
  padding: 1px 6px;
  border-radius: 4px;
  font-family: var(--font-mono);
  line-height: 1.6;
}

.controls-group {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
}

.play-btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
  padding: 5px 12px;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-primary);
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  font-family: inherit;
  transition: all 0.15s;
}

.play-btn:hover:not(:disabled) {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
}

.play-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.play-btn.playing-original:disabled,
.play-btn.playing-effects:disabled {
  opacity: 1;
  cursor: default;
}

.play-btn.playing-original {
  color: var(--color-accent);
  border-color: var(--color-accent);
  background: var(--btn-accent-bg);
}

.play-btn.playing-effects {
  color: var(--color-accent);
  border-color: var(--color-accent);
  background: var(--btn-accent-bg);
}

.icon-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  padding: 0;
  border: 1px solid var(--color-border);
  background: var(--color-bg-field);
  color: var(--color-text-secondary);
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.icon-btn:hover {
  background: var(--color-bg-field-hover);
  border-color: var(--color-border-strong);
  color: var(--color-text-primary);
}

.icon-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: 1px;
}

.icon-btn.stop-btn {
  color: var(--color-danger);
  border-color: var(--danger-border);
}

.icon-btn.stop-btn:hover:not(:disabled) {
  background: var(--danger-bg-weak);
}

.icon-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.error-line {
  height: 4px;
  line-height: 20px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  color: var(--color-danger);
}

.preview-bar.has-error .error-line {
  height: 20px;
}

.error-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

@media (max-width: 499px) {
  .bar-main {
    flex-wrap: wrap;
    gap: 4px 8px;
  }

  .pick-btn {
    width: 100%;
    justify-content: center;
  }

  .info-group {
    min-width: 0;
    flex: 1 1 auto;
  }

  .controls-group {
    width: 100%;
    justify-content: flex-start;
    flex-wrap: wrap;
  }

  .play-btn {
    padding: 4px 8px;
    font-size: 12px;
    gap: 4px;
  }
}
</style>
