import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";
import {
  cleanupTempDir,
  compileCommonJsModule,
  getRepoRoot,
  resetTempDir,
  runTests,
} from "./ts-test-helpers.mjs";

const root = getRepoRoot(import.meta.url);
const outDir = resetTempDir("spriteanime-task-workspace-tests");
const require = createRequire(import.meta.url);

const taskTypes = require(
  compileCommonJsModule(root, outDir, "src/workflows/task-types.ts")
);
const presentation = require(
  compileCommonJsModule(root, outDir, "src/workflows/task-presentation.ts")
);

const emptyContext = {
  hasImageSelection: false,
  mattingDirty: false,
  hasSpriteImage: false,
  hasSpriteFrames: false,
  spritePath: "",
  videoSourceMode: "local",
  hasVideo: false,
  videoName: "",
  videoOutputOrigin: "none",
  hasVideoOutput: false,
  hasRedrawRun: false,
};

function testTaskRoutesAreFinite() {
  assert.deepEqual(taskTypes.initialTaskRoute("image"), { kind: "image", stage: "source" });
  assert.deepEqual(taskTypes.initialTaskRoute("video"), { kind: "video", stage: "source" });
  assert.equal(taskTypes.isTaskRoute({ kind: "sprite", stage: "bounds" }), true);
  assert.equal(taskTypes.isTaskRoute({ kind: "video", stage: "grid" }), false);
  assert.equal(taskTypes.isTaskRoute({ kind: "unknown", stage: "source" }), false);
}

function testImageStagesUnlockFromActualOutputs() {
  const route = { kind: "image", stage: "source" };
  assert.equal(presentation.isTaskStepUnlocked(route, "source", emptyContext), true);
  assert.equal(presentation.isTaskStepUnlocked(route, "matting", emptyContext), false);
  const selected = { ...emptyContext, hasImageSelection: true };
  assert.equal(presentation.isTaskStepUnlocked(route, "matting", selected), true);
  assert.equal(presentation.isTaskStepUnlocked(route, "bounds", selected), false);
  const split = { ...selected, hasSpriteImage: true, hasSpriteFrames: true };
  assert.equal(presentation.isTaskStepUnlocked(route, "bounds", split), true);
  assert.equal(presentation.isTaskStepUnlocked(route, "preview", split), true);
}

function testVideoLabelsFollowSourceAndRunState() {
  const source = { kind: "video", stage: "source" };
  assert.equal(presentation.getPrimaryLabel(source, emptyContext), "选择本地视频");
  assert.equal(
    presentation.getPrimaryLabel(source, { ...emptyContext, videoSourceMode: "ai" }),
    "AI 生成视频"
  );
  const redraw = { kind: "video", stage: "redraw" };
  assert.equal(
    presentation.getPrimaryLabel(redraw, { ...emptyContext, hasRedrawRun: true }),
    "继续 / 重试"
  );
  assert.equal(presentation.getSecondaryLabel(redraw), "跳过重绘");
}

function testTaskSurfaceMapping() {
  assert.equal(presentation.getTaskSurfaceId({ kind: "image", stage: "source" }), "page-generator");
  assert.equal(presentation.getTaskSurfaceId({ kind: "image", stage: "grid" }), "page-sprite");
  assert.equal(presentation.getTaskSurfaceId({ kind: "video", stage: "preview" }), "page-video-sprite");
}

function testUnifiedShellHasOneNavigationModel() {
  const shell = readFileSync(`${root}/src/ui/app-shell.html`, "utf8");
  const surfaces = ["generator", "video-sprite", "sprite"].map((name) =>
    readFileSync(`${root}/src/ui/${name}.html`, "utf8")
  ).join("\n");
  assert.equal((shell.match(/data-start-task=/g) || []).length, 3);
  assert.match(shell, /id="workspace-step-list"/);
  assert.match(shell, /id="workspace-primary-action"/);
  assert.doesNotMatch(shell, /tab-button|data-page/);
  [
    "btn-generate",
    "btn-to-sprite",
    "btn-load-split",
    "btn-video-sprite-extract",
    "btn-video-redraw-start",
  ].forEach((id) => assert.doesNotMatch(surfaces, new RegExp(`id="${id}"`)));
}

runTests([
  testTaskRoutesAreFinite,
  testImageStagesUnlockFromActualOutputs,
  testVideoLabelsFollowSourceAndRunState,
  testTaskSurfaceMapping,
  testUnifiedShellHasOneNavigationModel,
]);

cleanupTempDir(outDir);
console.log("Task workspace tests passed.");
