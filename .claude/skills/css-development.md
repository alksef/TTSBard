# CSS Development Skill

## When to Use
- Styling Vue components
- Adding new theme variables
- Modifying existing styles
- Fixing visual bugs

## Process

1. **Identify target**: Locate component and its styles
2. **Check variables**: Review `src/styles/variables.css` for existing variables
3. **Use semantic colors**: Never hardcode hex/rgb values
4. **Test themes**: Verify both light and dark modes

## Conventions

### Color Usage
```css
/* Correct */
color: rgb(var(--rgb-accent));
background: var(--color-bg-primary);

/* Wrong */
color: #8B5CF6;
background: rgba(30, 30, 40, 0.9);
```

### Adding Variables
```css
/* In variables.css */
--rgb-my-color: 100, 150, 200;
--color-my-feature: rgb(var(--rgb-my-color));
```

### Theme Support
```css
/* Dark (default) */
--rgb-bg-primary: 17, 17, 27;

/* Light override */
[data-theme="light"] {
  --rgb-bg-primary: 255, 255, 255;
}
```

## Commands
```bash
npm run dev    # Live preview with HMR
```
