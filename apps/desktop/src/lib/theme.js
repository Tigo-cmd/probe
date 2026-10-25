// The colour scheme: follow the system, or force light or dark. The choice
// is a per-computer convenience kept in the webview's storage; if storage is
// unavailable the app simply follows the system.

const KEY = 'probe.theme';
export const THEMES = ['system', 'light', 'dark'];

export function getTheme() {
  try {
    const t = localStorage.getItem(KEY);
    return THEMES.includes(t) ? t : 'system';
  } catch {
    return 'system';
  }
}

export function applyTheme(theme) {
  const root = document.documentElement;
  if (theme === 'light' || theme === 'dark') root.dataset.theme = theme;
  else delete root.dataset.theme;
}

export function setTheme(theme) {
  applyTheme(theme);
  // Match the window frame too, inside the desktop app.
  if (typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window) {
    import('@tauri-apps/api/window')
      .then(({ getCurrentWindow }) => getCurrentWindow().setTheme(theme === 'system' ? null : theme))
      .catch(() => {});
  }
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // Not remembered; it still applies for this session.
  }
}
