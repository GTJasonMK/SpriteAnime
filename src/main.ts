import { GeneratorPage } from "./features/image/image-page";
import { SpritePage } from "./features/sprite/sprite-page";
import { VideoSpritePage } from "./features/video/video-page";
import { SettingsController } from "./settings/controller";
import appShellHtml from "./ui/app-shell.html?raw";
import generatorHtml from "./ui/generator.html?raw";
import settingsHtml from "./ui/settings.html?raw";
import spriteHtml from "./ui/sprite.html?raw";
import videoSpriteHtml from "./ui/video-sprite.html?raw";
import { TaskCoordinator } from "./workflows/task-coordinator";
import { WorkspaceSession } from "./workspace/session";

function mountApplication(): void {
  const root = document.getElementById("app");
  if (!root) {
    throw new Error("应用挂载节点 #app 不存在");
  }
  root.innerHTML = [appShellHtml, settingsHtml].join("");
  const host = document.getElementById("workspace-surface-host");
  if (!host) {
    throw new Error("工作台挂载节点 #workspace-surface-host 不存在");
  }
  host.innerHTML = [generatorHtml, videoSpriteHtml, spriteHtml].join("");
}

document.addEventListener("DOMContentLoaded", async () => {
  mountApplication();

  const settings = new SettingsController();
  await settings.init();
  const generator = new GeneratorPage(settings);
  const videoSprite = new VideoSpritePage();
  const sprite = new SpritePage();

  await generator.init();
  await videoSprite.init(settings);
  sprite.init();
  const coordinator = new TaskCoordinator(settings, generator, videoSprite, sprite);
  coordinator.init();

  const workspace = new WorkspaceSession(coordinator, generator, videoSprite, sprite);
  coordinator.setWorkspaceResetLifecycle(
    () => workspace.prepareForReset(),
    () => workspace.start()
  );
  try {
    const restored = await workspace.restore();
    if (!restored) coordinator.consumePendingTask();
  } catch (error) {
    console.error("[workspace] 恢复失败:", error);
    coordinator.showRecovery(error);
    return;
  }
  workspace.start();
  await workspace.bindWindowClose();
});
