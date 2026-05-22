import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import ts from "typescript";

export { pathToFileURL };

export function getRepoRoot(importMetaUrl) {
  return path.resolve(path.dirname(fileURLToPath(importMetaUrl)), "..");
}

export function resetTempDir(name) {
  const outDir = path.join(tmpdir(), name);
  rmSync(outDir, { recursive: true, force: true });
  mkdirSync(outDir, { recursive: true });
  return outDir;
}

export function cleanupTempDir(outDir) {
  rmSync(outDir, { recursive: true, force: true });
}

export function runTests(tests) {
  for (const test of tests) {
    test();
  }
}

export function compileCommonJsModule(root, outDir, sourceRelativePath) {
  return compileModule(root, outDir, sourceRelativePath, ts.ModuleKind.CommonJS);
}

export function compileEsModule(root, outDir, sourceRelativePath, outputRelativePath) {
  return compileModule(
    root,
    outDir,
    sourceRelativePath,
    ts.ModuleKind.ES2022,
    outputRelativePath
  );
}

function compileModule(root, outDir, sourceRelativePath, moduleKind, outputRelativePath) {
  const sourcePath = path.join(root, sourceRelativePath);
  const compiledRelativePath = outputRelativePath || sourceRelativePath.replace(/\.ts$/, ".js");
  const compiledPath = path.join(outDir, compiledRelativePath);
  mkdirSync(path.dirname(compiledPath), { recursive: true });

  const transpiled = ts.transpileModule(readFileSync(sourcePath, "utf8"), {
    compilerOptions: {
      module: moduleKind,
      target: ts.ScriptTarget.ES2022,
      strict: true,
    },
    fileName: sourcePath,
  });
  writeFileSync(compiledPath, transpiled.outputText);
  return compiledPath;
}
