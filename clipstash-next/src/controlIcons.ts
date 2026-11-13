// Shared geometry keeps React dialogs and the standalone reader aligned at every font scale.
export const CONTROL_ICON_PATHS = {
  close: "M6 6L18 18M18 6L6 18",
  previous: "M16 5L8 12L16 19",
  next: "M8 5L16 12L8 19",
} as const;

export function controlIconSvg(name: keyof typeof CONTROL_ICON_PATHS) {
  return `<svg class="control-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true" focusable="false"><path d="${CONTROL_ICON_PATHS[name]}"/></svg>`;
}
