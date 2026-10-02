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

/**
 * Whether a note has a sticky window on the desktop. `use-window-sync.js` opens windows for
 * exactly these notes (while desktop stickies are shown).
 *
 * @param {{ isPinned?: boolean; isArchived?: boolean; isDeleted?: boolean }} note
 */
export function hasStickyWindow(note) {
  return !!note.isPinned && !note.isArchived && !note.isDeleted;
}

/**
 * Why the `desktopStickiesSelectable` switch has no effect right now, or `null` when it
 * applies. Hidden stickies and global operation take precedence over it, and it only
 * changes stickies left on the desktop layer: wallpaper-layer stickies always let clicks
 * through and topmost stickies always take them (see `resolveNoteIgnoreCursor`).
 *
 * @param {{
 *   stickiesVisible: boolean;
 *   globalControlDisabled: boolean;
 *   notes: Array<{
 *     isPinned?: boolean;
 *     isArchived?: boolean;
 *     isDeleted?: boolean;
 *     isAlwaysOnTop?: boolean;
 *     isWallpaper?: boolean;
 *   }>;
 * }} input
 * @returns {"stickiesHidden" | "globalOperation" | "noDesktopStickies" | null}
 */
export function resolveDesktopSelectableOverride(input) {
  if (!input.stickiesVisible) return "stickiesHidden";
  if (!input.globalControlDisabled) return "globalOperation";
  const hasDesktopLayerSticky = input.notes.some(
    (note) => hasStickyWindow(note) && !note.isAlwaysOnTop && !note.isWallpaper,
  );
  return hasDesktopLayerSticky ? null : "noDesktopStickies";
}
