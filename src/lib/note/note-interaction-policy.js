/**
 * Whether a sticky note window lets the cursor through to what is below it. Must match
 * `resolve_note_ignore_cursor` in `src-tauri/src/desktop/sticky/layer.rs`.
 *
 * Global operation and topmost notes take the cursor. Wallpaper-layer notes always let it
 * through: on Windows and macOS they sit under the desktop icons, where clicks never
 * arrive. Desktop-layer notes let it through unless the `desktopStickiesSelectable`
 * preference asks for selectable text.
 *
 * @param {{
 *   globalControlDisabled: boolean;
 *   isAlwaysOnTop: boolean;
 *   isWallpaper: boolean;
 *   desktopStickiesSelectable: boolean;
 * }} input
 */
export function resolveNoteIgnoreCursor(input) {
  if (!input.globalControlDisabled) return false;
  if (input.isAlwaysOnTop) return false;
  if (input.isWallpaper) return true;
  return !input.desktopStickiesSelectable;
}
