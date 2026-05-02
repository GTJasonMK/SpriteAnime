import assert from "node:assert/strict";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const outDir = path.join(tmpdir(), "spriteanime-workflow-tests");

function compileModule(sourceRelativePath) {
  const sourcePath = path.join(root, sourceRelativePath);
  const compiledPath = path.join(outDir, `${sourceRelativePath.replace(/[\\/]/g, "__")}.js`);
  const transpiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
    compilerOptions: {
      module: ts.ModuleKind.ES2022,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: sourcePath,
  });
  writeFileSync(compiledPath, transpiled.outputText);
  return pathToFileURL(compiledPath).href;
}

rmSync(outDir, { recursive: true, force: true });
mkdirSync(outDir, { recursive: true });

const generatorWorkflow = await import(compileModule("src/pages/generator-workflow.ts"));
const spriteWorkflow = await import(compileModule("src/pages/sprite/workflow-state.ts"));

function testGeneratorWorkflow() {
  const empty = generatorWorkflow.getGeneratorWorkflowPermissions("empty", {
    hasRecords: false,
    hasSelection: false,
    hasMattingCanvas: false,
    mattingDirty: false,
    hasMattingUndo: false,
    hasMattingRedo: false,
  });
  assert.equal(empty.generate, true);
  assert.equal(empty.addRecord, true);
  assert.equal(empty.enterMatting, false);
  assert.equal(empty.deleteRecord, false);

  const ready = generatorWorkflow.getGeneratorWorkflowPermissions("ready", {
    hasRecords: true,
    hasSelection: true,
    hasMattingCanvas: false,
    mattingDirty: false,
    hasMattingUndo: false,
    hasMattingRedo: false,
  });
  assert.equal(ready.generate, true);
  assert.equal(ready.enterMatting, true);
  assert.equal(ready.sendToSprite, true);
  assert.equal(ready.clearRecords, true);
  assert.equal(ready.saveMatting, false);

  const generating = generatorWorkflow.getGeneratorWorkflowPermissions("generating", {
    hasRecords: true,
    hasSelection: true,
    hasMattingCanvas: true,
    mattingDirty: true,
    hasMattingUndo: true,
    hasMattingRedo: true,
  });
  assert.equal(generating.generate, false);
  assert.equal(generating.openSettings, false);
  assert.equal(generating.clearRecords, false);

  const matting = generatorWorkflow.getGeneratorWorkflowPermissions("matting", {
    hasRecords: true,
    hasSelection: true,
    hasMattingCanvas: true,
    mattingDirty: true,
    hasMattingUndo: true,
    hasMattingRedo: true,
  });
  assert.equal(matting.generate, false);
  assert.equal(matting.exitMatting, true);
  assert.equal(matting.runAutoMatting, true);
  assert.equal(matting.eraseMatting, true);
  assert.equal(matting.undoMatting, true);
  assert.equal(matting.redoMatting, true);
  assert.equal(matting.saveMatting, true);
  assert.equal(matting.deleteRecord, false);

  const mattingWithoutCanvas = generatorWorkflow.getGeneratorWorkflowPermissions("matting", {
    hasRecords: true,
    hasSelection: true,
    hasMattingCanvas: false,
    mattingDirty: true,
    hasMattingUndo: true,
    hasMattingRedo: true,
  });
  assert.equal(mattingWithoutCanvas.runAutoMatting, false);
  assert.equal(mattingWithoutCanvas.eraseMatting, false);
  assert.equal(mattingWithoutCanvas.saveMatting, false);

  const mattingProcessing = generatorWorkflow.getGeneratorWorkflowPermissions("mattingProcessing", {
    hasRecords: true,
    hasSelection: true,
    hasMattingCanvas: true,
    mattingDirty: true,
    hasMattingUndo: true,
    hasMattingRedo: true,
  });
  assert.equal(mattingProcessing.exitMatting, false);
  assert.equal(mattingProcessing.eraseMatting, false);
  assert.equal(mattingProcessing.redoMatting, false);
  assert.equal(mattingProcessing.saveMatting, false);
}

function testSpriteWorkflow() {
  const empty = spriteWorkflow.getSpriteWorkflowPermissions("empty", {
    hasImage: false,
    hasFrames: false,
  });
  assert.equal(empty.pickImage, true);
  assert.equal(empty.previewGrid, true);
  assert.equal(empty.splitFrames, false);
  assert.equal(empty.exportFrames, false);

  const editing = spriteWorkflow.getSpriteWorkflowPermissions("editingGrid", {
    hasImage: true,
    hasFrames: false,
  });
  assert.equal(editing.editGrid, true);
  assert.equal(editing.editRegion, true);
  assert.equal(editing.detectBounds, true);
  assert.equal(editing.splitFrames, true);
  assert.equal(editing.exportFrames, false);

  const detecting = spriteWorkflow.getSpriteWorkflowPermissions("detectingBounds", {
    hasImage: true,
    hasFrames: false,
  });
  assert.equal(detecting.editGrid, false);
  assert.equal(detecting.detectBounds, false);
  assert.equal(detecting.splitFrames, false);

  const split = spriteWorkflow.getSpriteWorkflowPermissions("previewingFrames", {
    hasImage: true,
    hasFrames: true,
  });
  assert.equal(split.returnToGrid, true);
  assert.equal(split.playFrames, true);
  assert.equal(split.exportFrames, true);
  assert.equal(split.editGrid, false);
  assert.equal(split.pickImage, false);
}

testGeneratorWorkflow();
testSpriteWorkflow();

rmSync(outDir, { recursive: true, force: true });
console.log("Workflow state tests passed.");
