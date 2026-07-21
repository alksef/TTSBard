# CSS Development Skill

Используйте для Vue styles, тем, токенов и визуальных исправлений.

## Правила

1. Сначала искать существующий semantic token в `src/styles/variables.css`.
2. Цвета и повторяющиеся размеры выражать через проектные tokens; локальное
   значение допустимо только когда оно не несёт theme/semantic смысла.
3. Новый цветовой token определить для обеих тем и назвать по назначению, а не
   по оттенку.
4. Проверить light/dark theme, узкую ширину, focus/disabled/error states и
   отсутствие горизонтального overflow.
5. Icon-only control должен иметь `title` и `aria-label`.

```css
/* semantic usage */
color: rgb(var(--rgb-accent));
background: var(--color-bg-primary);
```

Решение проекта: [DECISION-012 — CSS tokens](../../docs/decisions/012-css-tokens.md).
Для интерактивной проверки используйте `npm run dev`; typecheck и production
frontend build выполняет `npm run build`.
