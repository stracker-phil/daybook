/**
 * Spread on an overlay part that moves the window: Tauri's drag script drags from anywhere inside it except
 * clickable elements (buttons, inputs, links, focusable or option/button roles).
 */
export const DRAG_REGION = { "data-tauri-drag-region": "deep" } as const;
