import "@testing-library/jest-dom/vitest";
import { vi } from "vitest";

// ── Stub DOM methods missing in jsdom ───────────────────────────────────
Element.prototype.scrollIntoView = vi.fn();

// ── Mock @tauri-apps/api modules ────────────────────────────────────────
// These are needed because any Svelte component that imports from Tauri
// will fail in jsdom without stubs.

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/window", () => {
  const setSize = vi.fn();
  const hide = vi.fn();
  const close = vi.fn();
  const show = vi.fn();
  const onFocusChanged = vi.fn().mockResolvedValue(() => {});
  const startDragging = vi.fn();
  return {
    getCurrentWindow: () => ({
      label: "main",
      setSize,
      hide,
      close,
      show,
      onFocusChanged,
      startDragging,
    }),
    LogicalSize: class LogicalSize {
      width: number;
      height: number;
      constructor(w: number, h: number) {
        this.width = w;
        this.height = h;
      }
    },
  };
});

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
  emit: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-global-shortcut", () => ({
  register: vi.fn().mockResolvedValue(undefined),
  unregister: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));
