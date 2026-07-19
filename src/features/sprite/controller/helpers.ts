export function isEditableKeyboardTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) {
    return false;
  }
  if (target.isContentEditable || target.closest("[contenteditable='true']")) {
    return true;
  }
  return ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

const BOUNDARY_EDITOR_INPUT_IDS = new Set([
  "boundary-left-input",
  "boundary-top-input",
  "boundary-right-input",
  "boundary-bottom-input",
  "boundary-anchor-x-input",
]);

export function getBoundaryEditorInputTarget(target: EventTarget | null): HTMLInputElement | null {
  if (!(target instanceof HTMLInputElement)) {
    return null;
  }
  return BOUNDARY_EDITOR_INPUT_IDS.has(target.id) ? target : null;
}
