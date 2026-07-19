import { getCurrentWindow } from "@tauri-apps/api/window";
import { resetWorkspace, revealWorkspaceSnapshot } from "../api/commands";
import { bindModalFocusTrap } from "../utils/dialog";
import { getById } from "../utils/dom";
import { getErrorMessage } from "../utils/errors";

export function showWorkspaceRecovery(
  error: unknown,
  beforeReset: (() => Promise<boolean>) | null,
  resume: (() => void) | null
): void {
  const modal = getById("workspace-recovery-modal");
  const message = getById("workspace-recovery-message");
  const reset = getById<HTMLButtonElement>("btn-reset-workspace");
  message.textContent = getErrorMessage(error);
  modal.hidden = false;
  getById<HTMLButtonElement>("btn-reveal-workspace").onclick = () => void revealWorkspaceSnapshot();
  getById<HTMLButtonElement>("btn-exit-workspace-recovery").onclick = () => void getCurrentWindow().destroy();
  reset.onclick = async () => {
    reset.disabled = true;
    let shouldResume = false;
    try {
      shouldResume = await beforeReset?.() ?? false;
      await resetWorkspace();
      window.location.reload();
    } catch (resetError) {
      if (shouldResume) resume?.();
      reset.disabled = false;
      message.textContent = `重置失败：${getErrorMessage(resetError)}`;
    }
  };
  bindModalFocusTrap(modal, () => reset.focus());
  reset.focus();
}
