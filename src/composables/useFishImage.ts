import { invoke } from '@tauri-apps/api/core';

const imageCache = new Map<string, string>();

export async function fetchFishImage(imageUrl: string | null | undefined): Promise<string | undefined> {
  if (!imageUrl) return undefined;

  // Check cache first
  if (imageCache.has(imageUrl)) {
    return imageCache.get(imageUrl)!;
  }

  try {
    const dataUrl = await invoke<string>('fetch_fish_audio_image', { imageUrl });
    imageCache.set(imageUrl, dataUrl);
    return dataUrl;
  } catch (error) {
    console.error('Failed to fetch image:', error);
    return undefined;
  }
}

export function clearFishImageCache() {
  imageCache.clear();
}
