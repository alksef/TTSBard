# Error Handling Visual Preview

## Silero Card Error State

### Normal State
```
┌─────────────────────────────────────────────────────────┐
│ ○ Silero Bot                                 ▶         │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  ● Не подключено                                        │
│     Авторизуйтесь для использования Silero TTS          │
│                                                          │
│  [Подключить Telegram]                                  │
│                                                          │
│  Информация:                                            │
│  • Для работы Silero TTS необходима авторизация...      │
└─────────────────────────────────────────────────────────┘
```

### Error State
```
┌─────────────────────────────────────────────────────────┐
│ ○ Silero Bot                                 ▼         │  ← Red border #ef4444
├─────────────────────────────────────────────────────────┤  ← Light red background #fef2f2
│                                                          │
│  ┌──────────────────────────────────────────────────┐  │
│  │ ⚠ Ошибка подключения Telegram             [Исправить] │  │  ← Error banner
│  │   Не удалось подключиться к Telegram              │  │
│  └──────────────────────────────────────────────────┘  │
│                                                          │
│  ● Не подключено                                        │
│     Авторизуйтесь для использования Silero TTS          │
│                                                          │
│  [Подключить Telegram]                                  │
│                                                          │
│  Информация:                                            │
│  • Для работы Silero TTS необходима авторизация...      │
└─────────────────────────────────────────────────────────┘
```

## Telegram Auth Modal - Error State

### Error State Layout
```
┌─────────────────────────────────────────┐
│  Подключение Telegram              [×]  │
├─────────────────────────────────────────┤
│                                          │
│          ⚠                               │  ← Red circle icon
│     Ошибка подключения                   │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │ Не удалось подключиться к Telegram.  │ │  ← Error message
│  │ Проверьте credentials и повторите.   │ │
│  └────────────────────────────────────┘ │
│                                          │
│  ┌────────────────────────────────────┐ │
│  │ Произошла ошибка при подключении    │ │
│  │ к Telegram. Попробуйте снова или    │ │
│  │ отключите интеграцию.               │ │
│  └────────────────────────────────────┘ │
│                                          │
│  [Попробовать снова] [Отключить]         │  ← Action buttons
│                                          │
└─────────────────────────────────────────┘
```

## Color Scheme

### Error Colors
- **Background**: `#fef2f2` (Light red)
- **Border**: `#ef4444` (Red)
- **Error Banner Background**: `#fee` (Very light red)
- **Error Banner Border**: `#fcc` (Medium red)
- **Error Banner Accent**: `#f44` (Strong red)
- **Error Text**: `#c33` (Dark red)
- **Error Details**: `#933` (Very dark red)

### Button Colors
- **Fix Button**: `#dc2626` (Red) → hover: `#b91c1c` (Darker red)
- **Retry Button**: `#2563eb` (Blue) → hover: `#1d4ed8` (Darker blue)
- **Disable Button**: `#6b7280` (Gray) → hover: `#4b5563` (Darker gray)

## CSS Classes Reference

### TtsPanel.vue
```css
.provider-card.error-state      /* Red border and background */
.silero-error-banner            /* Error banner container */
.error-banner-content           /* Banner content layout */
.error-icon                     /* Warning icon */
.error-text                     /* Text container */
.error-title                    /* Title style */
.error-message                  /* Message style */
.fix-button                     /* Fix button style */
```

### TelegramAuthModal.vue
```css
.error-state                    /* Modal error container */
.error-icon-modal               /* Large error icon */
.error-message-modal            /* Error message box */
.error-info                     /* Info box with red accent */
.retry-button                   /* Retry action button */
.disable-button                 /* Disable action button */
```

## Responsive Behavior

### Desktop (> 768px)
- Error banner: Full width with horizontal layout
- Buttons: Side by side with equal width
- Icon: 64px diameter

### Mobile (< 768px)
- Error banner: Stacked vertical layout
- Buttons: Full width stacked
- Icon: 48px diameter
- Padding reduced to 12px

## Animation States

### Error Appearance
```css
.provider-card {
  transition: all 0.2s ease;
}

/* When error appears */
.provider-card.error-state {
  border-color: #ef4444;
  background: #fef2f2;
  animation: error-pulse 0.3s ease;
}

@keyframes error-pulse {
  0% { transform: scale(1); }
  50% { transform: scale(1.02); }
  100% { transform: scale(1); }
}
```

### Button Hover
```css
.fix-button {
  transition: background 0.2s;
}

.fix-button:hover {
  background: #b91c1c;
  transform: translateY(-1px);
  box-shadow: 0 2px 8px rgba(220, 38, 38, 0.3);
}
```

## Accessibility

### ARIA Labels
```vue
<div
  v-if="sileroError"
  class="silero-error-banner"
  role="alert"
  aria-live="assertive"
>
  <button
    class="fix-button"
    aria-label="Исправить ошибку подключения Telegram"
  >
    Исправить
  </button>
</div>
```

### Keyboard Navigation
- **Tab**: Navigate between buttons
- **Enter**: Activate focused button
- **Escape**: Close modal (if open)
- **Focus management**: Modal traps focus when open

### Screen Reader Support
- Error announcements: `role="alert"` + `aria-live="assertive"`
- Button labels: Clear `aria-label` attributes
- State changes: Announced to screen readers
- Error messages: Read automatically when appear

## Browser Compatibility

✅ Chrome/Edge: Full support
✅ Firefox: Full support
✅ Safari: Full support
✅ Opera: Full support

Minimum versions:
- Chrome 90+
- Firefox 88+
- Safari 14+
- Edge 90+
