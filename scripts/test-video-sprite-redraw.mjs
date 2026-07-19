import assert from "node:assert/strict";
import { createRequire } from "node:module";
import {
  cleanupTempDir,
  compileCommonJsModule,
  getRepoRoot,
  resetTempDir,
  runTests,
} from "./ts-test-helpers.mjs";

const root = getRepoRoot(import.meta.url);
const outDir = resetTempDir("spriteanime-video-redraw-tests");
const require = createRequire(import.meta.url);
const redraw = require(
  compileCommonJsModule(root, outDir, "src/features/video/redraw.ts")
);

function testEvenFourByFourPlan() {
  const plan = redraw.buildRedrawPlan(16, 4, 2, 2);
  assert.equal(plan.finalRows, 4);
  assert.equal(plan.batches.length, 4);
  assert.equal(plan.batches[3].paddingCount, 0);
  assert.deepEqual(plan.batches[3].sourceIndices, [12, 13, 14, 15]);
}

function testPartialBatchDuplicatesLastFrame() {
  const plan = redraw.buildRedrawPlan(10, 4, 2, 2);
  assert.equal(plan.finalRows, 3);
  assert.equal(plan.finalRows * plan.finalCols - plan.totalFrames, 2);
  assert.equal(plan.batches.length, 3);
  assert.equal(plan.batches[2].paddingCount, 2);
  assert.deepEqual(plan.batches[2].sourceIndices, [8, 9, 9, 9]);
  assert.equal(plan.batches[2].validCount, 2);
}

function testLargeAllowedGroupWarns() {
  const plan = redraw.buildRedrawPlan(18, 6, 3, 3);
  assert.equal(plan.groupRows * plan.groupCols, 9);
}

function testInvalidGroupsFailExplicitly() {
  assert.throws(() => redraw.buildRedrawPlan(8, 4, 4, 4), /每组最多 9 帧/);
  assert.throws(() => redraw.buildRedrawPlan(4, 4, 3, 2), /不能大于总帧数/);
  assert.throws(() => redraw.buildRedrawPlan(1, 1, 1, 1), /总帧数必须/);
}

function testProjectedOutputGuardsCanvasLimits() {
  const normal = redraw.buildRedrawPlan(16, 4, 2, 2);
  assert.deepEqual(redraw.getProjectedRedrawOutput(normal, "1K"), {
    width: 2048,
    height: 2048,
    pixels: 4194304,
  });
  const tooTall = redraw.buildRedrawPlan(64, 1, 1, 1);
  assert.throws(
    () => redraw.validateProjectedRedrawOutput(tooTall, "1K"),
    /超过最长边/
  );
}

runTests([
  testEvenFourByFourPlan,
  testPartialBatchDuplicatesLastFrame,
  testLargeAllowedGroupWarns,
  testInvalidGroupsFailExplicitly,
  testProjectedOutputGuardsCanvasLimits,
]);

cleanupTempDir(outDir);
console.log("Video sprite redraw tests passed.");
