export const TASK_ROUTE_CHANGE_EVENT = "spriteanime:task-route-change";
export const TASK_STATE_CHANGE_EVENT = "spriteanime:task-state-change";

export function notifyTaskStateChanged(): void {
  document.dispatchEvent(new CustomEvent(TASK_STATE_CHANGE_EVENT));
}
