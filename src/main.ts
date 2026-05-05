import { GeneratorPage } from "./pages/generator";
import { SpritePage } from "./pages/sprite";
import { VideoSpritePage } from "./pages/video-sprite";

document.addEventListener("DOMContentLoaded", async () => {
  console.log("[main] SpriteAnimte 启动中...");

  const generator = new GeneratorPage();
  const videoSprite = new VideoSpritePage();
  const sprite = new SpritePage();

  // 标签页切换
  const tabButtons = document.querySelectorAll<HTMLButtonElement>(".tab-button");
  const pages = document.querySelectorAll<HTMLElement>(".page");

  tabButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      const tabName = btn.dataset.tab;
      console.log("[main] 切换到标签页:", tabName);

      // 切换激活标签
      tabButtons.forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");

      // 切换页面
      pages.forEach((p) => p.classList.remove("active"));
      const targetPage = document.getElementById(`page-${tabName}`);
      if (targetPage) {
        targetPage.classList.add("active");
      }
    });
  });

  // 初始化两个页面
  try {
    await generator.init();
    videoSprite.init(generator);
    await sprite.init(generator);
    console.log("[main] SpriteAnimte 启动完成");
  } catch (err) {
    console.error("[main] 启动失败:", err);
  }
});
