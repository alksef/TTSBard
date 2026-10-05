import { beforeEach, afterEach, describe, expect, it, vi } from 'vitest';
import { createRenderer, h, nextTick } from 'vue';
import { useInterfaceFontSync } from './useInterfaceFontSync';

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(),
  callbacks: new Map<string, () => void>(),
  unlisteners: [] as ReturnType<typeof vi.fn>[],
}));
vi.mock('@tauri-apps/api/core', () => ({ invoke: mocks.invoke }));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (name: string, callback: () => void) => {
    mocks.callbacks.set(name, callback);
    const unlisten = vi.fn(() => mocks.callbacks.delete(name));
    mocks.unlisteners.push(unlisten);
    return unlisten;
  }),
}));
vi.mock('../utils/debug', () => ({ debugLog: vi.fn(), debugWarn: vi.fn(), debugError: vi.fn() }));

describe('separate-window interface font lifecycle', () => {
  const renderer = createRenderer({
    createElement: () => ({}), createText: () => ({}), createComment: () => ({}),
    insert: () => {}, remove: () => {}, setText: () => {}, setElementText: () => {},
    parentNode: () => null, nextSibling: () => null, patchProp: () => {},
  });
  let family: string;
  let size: number;
  let props: Map<string, string>;
  let unmount: (() => void) | undefined;

  beforeEach(() => {
    family = 'Georgia';
    size = 20;
    props = new Map();
    mocks.callbacks.clear();
    mocks.unlisteners.length = 0;
    mocks.invoke.mockReset();
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === 'is_backend_ready') return true;
      if (command === 'get_all_app_settings') return {
        general: { ui_font_family: family, ui_font_size_px: size },
        tts: { provider: 'silero' }, webview: { enabled: false },
        audio: { speaker_enabled: false },
      };
      throw new Error(`Unexpected command: ${command}`);
    });
    vi.stubGlobal('document', {
      documentElement: { style: {
        setProperty: (key: string, value: string) => props.set(key, value),
        removeProperty: (key: string) => props.delete(key),
      } },
    });
  });
  afterEach(() => { unmount?.(); unmount = undefined; vi.unstubAllGlobals(); });

  async function mountWindow() {
    const errors: unknown[] = [];
    const app = renderer.createApp({ setup() { useInterfaceFontSync(); return () => h('div'); } });
    app.config.errorHandler = error => errors.push(error);
    app.mount({});
    unmount = () => app.unmount();
    await vi.waitFor(() => expect(props.get('--ui-font-size')).toBe('20px'));
    await vi.waitFor(() => expect(mocks.callbacks.has('settings-changed')).toBe(true));
    expect(errors).toEqual([]);
    expect(mocks.invoke).toHaveBeenCalledWith('get_all_app_settings', { consumeStartupNotifications: false });
  }

  it('mounts a root without a provider and restores its saved font', async () => {
    await mountWindow();
    expect(props.get('--ui-font-family')).toBe("'Georgia', var(--font-sans)");
  });

  it('updates an already-open window and resets to stylesheet defaults', async () => {
    await mountWindow();
    family = 'Segoe UI'; size = 18;
    mocks.callbacks.get('settings-changed')?.();
    await vi.waitFor(() => expect(props.get('--ui-font-size')).toBe('18px'));
    expect(props.get('--ui-font-family')).toContain('Segoe UI');
    family = 'default'; size = 16;
    mocks.callbacks.get('settings-changed')?.();
    await vi.waitFor(() => expect(props.has('--ui-font-size')).toBe(false));
    expect(props.has('--ui-font-family')).toBe(false);
  });

  it('disposes listeners and stops font updates on unmount', async () => {
    await mountWindow();
    await vi.waitFor(() => expect(mocks.unlisteners.length).toBe(4));
    const oldCallback = mocks.callbacks.get('settings-changed');
    unmount?.(); unmount = undefined;
    for (const stop of mocks.unlisteners) expect(stop).toHaveBeenCalledOnce();
    family = 'Segoe UI'; size = 18;
    oldCallback?.();
    await Promise.resolve(); await Promise.resolve(); await nextTick();
    expect(props.get('--ui-font-size')).toBe('20px');
  });
});
