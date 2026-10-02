export const NOTE_WINDOW_NON_DRAGGABLE_SELECTOR = [
  "button",
  "input",
  "select",
  "textarea",
  "a",
  "label",
  "summary",
  "[contenteditable=\"true\"]",
  ".command-popover",
  ".note-tag-editor",
  ".note-conflict-notice",
  ".color-popover",
  ".text-color-popover",
  ".opacity-popover",
  ".frost-popover",
].join(",");

/**
 * @param {{
 *   getCurrentWindow: () => {
 *     outerPosition: () => Promise<{ x: number; y: number }>;
 *     scaleFactor: () => Promise<number>;
 *   };
 *   moveWindow: (position: { x: number; y: number }) => Promise<void>;
 *   getWindowPosition?: () => Promise<{ x: number; y: number; surfaceRelativePointer?: boolean }>;
 *   now?: () => number;
 *   getCanInteract: () => boolean;
 *   getIsEditing: () => boolean;
 *   getIsAlwaysOnTop?: () => boolean;
 *   dismissFloatingPanels: (target: HTMLElement | null) => void;
 *   onDraggingChange?: (dragging: boolean) => void;
 *   onPositionPersist?: (position: { x: number; y: number }) => Promise<void> | void;
 * }} input
 */
export function createNoteWindowDragController(input) {
  let dragWindowX = 0;
  let dragWindowY = 0;
  let lastDragScreenX = 0;
  let lastDragScreenY = 0;
  let dragPointerId = -1;
  let dragging = false;
  // Wayland reports pointer "screen" coordinates relative to the window, which moves
  // under the pointer while dragging. The grab point then stays fixed and each event's
  // offset from it is how far the window still has to move. An event is relative to
  // wherever the compositor had the window when it sent it, and a move takes a frame or
  // two to land, so events are dropped while a move is in flight or settling; otherwise
  // the same motion would be counted twice and the window would overshoot the pointer.
  let surfaceRelativePointer = false;
  let surfaceMoveInFlight = false;
  let surfaceMoveSettledAt = 0;
  let lastAppliedScreenX = NaN;
  let lastAppliedScreenY = NaN;
  const SURFACE_MOVE_SETTLE_MS = 32;
  const now = input.now ?? (() => performance.now());
  let pendingPointerId = -1;
  let pendingStartScreenX = 0;
  let pendingStartScreenY = 0;
  /** @type {HTMLDivElement | null} */
  let pendingSurface = null;

  const DRAG_START_THRESHOLD_PX = 4;

  /** @param {boolean} next */
  function setDragging(next) {
    dragging = next;
    input.onDraggingChange?.(next);
  }

  function endManualWindowDrag() {
    setDragging(false);
    dragPointerId = -1;
    clearPendingDragIntent();
  }

  function clearPendingDragIntent() {
    pendingPointerId = -1;
    pendingStartScreenX = 0;
    pendingStartScreenY = 0;
    pendingSurface = null;
  }

  function persistCurrentPosition() {
    if (!dragging) return;
    void Promise.resolve(
      input.onPositionPersist?.({
        x: dragWindowX,
        y: dragWindowY,
      }),
    ).catch((err) => {
      console.error("persistCurrentPosition failed", err);
    });
  }

  /**
   * @param {PointerEvent} event
   * @param {HTMLDivElement} dragSurface
   */
  async function startManualWindowDrag(event, dragSurface) {
    const grabScreenX = pendingStartScreenX;
    const grabScreenY = pendingStartScreenY;
    if (input.getWindowPosition) {
      const position = await input.getWindowPosition();
      dragWindowX = position.x;
      dragWindowY = position.y;
      surfaceRelativePointer = !!position.surfaceRelativePointer;
    } else {
      const win = input.getCurrentWindow();
      const [position, scaleFactor] = await Promise.all([win.outerPosition(), win.scaleFactor()]);
      dragWindowX = position.x / scaleFactor;
      dragWindowY = position.y / scaleFactor;
      surfaceRelativePointer = false;
    }
    lastDragScreenX = surfaceRelativePointer ? grabScreenX : event.screenX;
    lastDragScreenY = surfaceRelativePointer ? grabScreenY : event.screenY;
    surfaceMoveInFlight = false;
    surfaceMoveSettledAt = 0;
    lastAppliedScreenX = NaN;
    lastAppliedScreenY = NaN;
    dragPointerId = event.pointerId;
    setDragging(true);
    dragSurface.setPointerCapture(event.pointerId);
    clearPendingDragIntent();
  }

  /** @param {PointerEvent} event */
  function applyManualWindowDragPosition(event) {
    if (!dragging) return;
    if (event.pointerId !== dragPointerId) return;
    if (event.buttons !== 1) {
      persistCurrentPosition();
      endManualWindowDrag();
      return;
    }
    if (surfaceRelativePointer) {
      moveSurfaceTowardPointer(event);
      return;
    }
    const deltaX = event.screenX - lastDragScreenX;
    const deltaY = event.screenY - lastDragScreenY;
    lastDragScreenX = event.screenX;
    lastDragScreenY = event.screenY;
    dragWindowX += deltaX;
    dragWindowY += deltaY;
    input
      .moveWindow({
        x: dragWindowX,
        y: dragWindowY,
      })
      .catch((err) => {
        console.error("moveWindow failed", err);
        endManualWindowDrag();
      });
  }

  /**
   * @param {PointerEvent} event
   * @param {{ ignoreSettle?: boolean }} [options]
   */
  function moveSurfaceTowardPointer(event, options = {}) {
    if (surfaceMoveInFlight) return;
    if (!options.ignoreSettle && now() < surfaceMoveSettledAt) return;
    if (!Number.isFinite(event.screenX) || !Number.isFinite(event.screenY)) return;
    // A release reports the last motion position, which may already have been applied.
    if (event.screenX === lastAppliedScreenX && event.screenY === lastAppliedScreenY) return;
    const deltaX = event.screenX - lastDragScreenX;
    const deltaY = event.screenY - lastDragScreenY;
    if (deltaX === 0 && deltaY === 0) return;
    lastAppliedScreenX = event.screenX;
    lastAppliedScreenY = event.screenY;
    dragWindowX += deltaX;
    dragWindowY += deltaY;
    surfaceMoveInFlight = true;
    input
      .moveWindow({
        x: dragWindowX,
        y: dragWindowY,
      })
      .then(() => {
        surfaceMoveSettledAt = now() + SURFACE_MOVE_SETTLE_MS;
      })
      .catch((err) => {
        console.error("moveWindow failed", err);
        endManualWindowDrag();
      })
      .finally(() => {
        surfaceMoveInFlight = false;
      });
  }

  /** @param {PointerEvent} event */
  function onDragPointerMove(event) {
    if (!dragging && event.pointerId === pendingPointerId) {
      const deltaX = Math.abs(event.screenX - pendingStartScreenX);
      const deltaY = Math.abs(event.screenY - pendingStartScreenY);
      if (deltaX < DRAG_START_THRESHOLD_PX && deltaY < DRAG_START_THRESHOLD_PX) {
        return;
      }
      event.preventDefault();
      const surface = pendingSurface;
      if (!surface) {
        clearPendingDragIntent();
        return;
      }
      void startManualWindowDrag(event, surface).catch((err) => {
        console.error("startManualWindowDrag failed", err);
        clearPendingDragIntent();
      });
      return;
    }
    applyManualWindowDragPosition(event);
  }

  /** @param {PointerEvent} event */
  function onDragPointerUp(event) {
    if (!dragging && event.pointerId === pendingPointerId) {
      clearPendingDragIntent();
      return;
    }
    if (event.pointerId !== dragPointerId) return;
    const surface = /** @type {HTMLDivElement | null} */ (event.currentTarget);
    if (surface?.hasPointerCapture(event.pointerId)) {
      surface.releasePointerCapture(event.pointerId);
    }
    // Catch up with motion dropped while the last move settled.
    if (surfaceRelativePointer) moveSurfaceTowardPointer(event, { ignoreSettle: true });
    persistCurrentPosition();
    endManualWindowDrag();
    clearPendingDragIntent();
  }

  /** @param {PointerEvent} event */
  async function handleDragPointerDown(event) {
    if (event.button !== 0) return;
    if (!input.getCanInteract()) return;
    const target = /** @type {HTMLElement | null} */ (event.target);
    input.dismissFloatingPanels(target);
    if (target?.closest(NOTE_WINDOW_NON_DRAGGABLE_SELECTOR)) {
      return;
    }
    const isAlwaysOnTop = input.getIsAlwaysOnTop?.() ?? false;
    const inShell = !!target?.closest(".note-shell");
    const inWindow = !!target?.closest(".note-window");
    const inToolbar = !!target?.closest(".toolbar");
    const inPreview = !!target?.closest(".preview-text, .note-block-surface");
    if (input.getIsEditing()) {
      if (!isAlwaysOnTop && !inToolbar) return;
      if (isAlwaysOnTop && !inWindow && !inToolbar && !inShell) return;
    } else if (isAlwaysOnTop) {
      if (!inWindow && !inToolbar && !inShell) return;
    } else if (!isAlwaysOnTop && !inPreview && !inToolbar) {
      return;
    }
    const surface = /** @type {HTMLDivElement} */ (event.currentTarget);
    pendingPointerId = event.pointerId;
    pendingStartScreenX = event.screenX;
    pendingStartScreenY = event.screenY;
    pendingSurface = surface;
  }

  return {
    endManualWindowDrag,
    handleDragPointerDown,
    onDragPointerMove,
    onDragPointerUp,
  };
}
