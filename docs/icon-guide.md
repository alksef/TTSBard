# TTSBard Icon Creation Guide

## Overview

This guide explains how to create and generate icons for TTSBard application.

## Required Icons

All icons should be placed in `src-tauri/icons/` directory.

| File | Size | Purpose |
|------|------|---------|
| `32x32.png` | 32×32px | Small icon |
| `128x128.png` | 128×128px | Standard icon |
| `128x128@2x.png` | 256×256px | HiDPI/Retina |
| `icon.icns` | — | macOS application icon |
| `icon.ico` | — | Windows application icon |
| `icon.png` | 512×512px | Source + System Tray |
| `icon-active.png` | 512×512px | System Tray (active state) |

## Method 1: Tauri CLI (Recommended)

Easiest way - generate all icons from a single source image.

```bash
cd D:/RustProjects/app-tts-v2

# Create source icon (1024x1024px) and save as ttsbard.png in project root
# Then run:
npx @tauri-apps/cli icon src-tauri/icons/ttsbard.png
```

All required icons will be automatically generated.

## Method 2: Online Tools

### Step 1: Create Source Icon
- Size: 1024×1024px
- Format: PNG with transparency
- Design: Simple, recognizable at small sizes

### Step 2: Convert to .ICO (Windows)
Use: https://icoconvert.com/
1. Upload your 1024×1024 PNG
2. Select sizes: 256, 128, 96, 64, 48, 32, 16
3. Download `icon.ico`
4. Place in `src-tauri/icons/`

### Step 3: Convert to .ICNS (macOS)
Use: https://cloudconvert.com/png-to-icns
1. Upload your 1024×1024 PNG
2. Download `icon.icns`
3. Place in `src-tauri/icons/`

### Step 4: Resize for PNG icons
Use: https://squoosh.app/ or ImageMagick:
```bash
# Install ImageMagick, then:
convert icon.png -resize 32x32 32x32.png
convert icon.png -resize 128x128 128x128.png
convert icon.png -resize 256x256 128x128@2x.png
```

## Method 3: ImageMagick (CLI)

```bash
# Create all variants from source icon.png
convert icon.png -resize 32x32 32x32.png
convert icon.png -resize 128x128 128x128.png
convert icon.png -resize 256x256 128x128@2x.png

# Windows .ico (multiple sizes in one file)
convert icon.png -define icon:auto-resize=256,128,96,64,48,32,16 icon.ico

# macOS .icns (requires iconutil on macOS)
# Or use online converter
```

## Design Recommendations for TTSBard

### Concept
- **Symbol**: 🔊 speaker, 📜 scroll, or 🎭 theater mask
- **Style**: Minimalist, flat design
- **Colors**: Match app theme (dark background + accent)

### Tips for Good Icons
1. **Simplicity** - Should be recognizable at 32×32px
2. **Contrast** - Good contrast for visibility
3. **Consistency** - Match your app's visual style
4. **Unique** - Stand out from other TTS apps

### Suggested Color Palette
```
Background: #2c2c2c (app dark theme)
Accent: #4CAF50 (green from app)
Text/Symbol: White or light gray
```

### Example Ideas

1. **Speaker + Book**: 📢 + 📖 = Knowledge spoken
2. **Play Button in Speech Bubble**: ▶️ inside bubble
3. **Sound Waves with Wand**: 🎵 + ✨ = TTS magic
4. **Simple "TTS" text**: Clean typography

## Verification

After generating icons, verify they appear correctly:

```bash
# Run dev build
npm run tauri dev

# Check:
# - Windows taskbar icon
# - System tray icon
# - Window title bar icon
```

## Tools Summary

| Tool | Purpose | Platform |
|------|---------|----------|
| Tauri CLI | Auto-generate all formats | Cross-platform |
| Figma | Design source icon | Web/App |
| Inkscape | Vector design | Cross-platform |
| GIMP | Raster editing | Cross-platform |
| Squoosh | Image compression | Web |
| ICO Convert | .ICO generation | Web |
| CloudConvert | Format conversion | Web |

## File Structure

```
src-tauri/
└── icons/
    ├── 32x32.png
    ├── 128x128.png
    ├── 128x128@2x.png
    ├── icon.icns
    ├── icon.ico
    ├── icon.png          # Main + Tray
    └── icon-active.png   # Tray (active)
```

## Notes

- Always keep a high-resolution source (1024×1024 or SVG)
- Test icons on both light and dark backgrounds
- System tray icons should be simple (high contrast)
- Consider accessibility (color blindness friendly)
