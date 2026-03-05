#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "./args.mjs";

function main() {
  const args = parseArgs(process.argv.slice(2));
  const packageDir = args["package-dir"];
  const expectedName = args["expected-name"];
  const expectedRepositoryUrl = args["expected-repository-url"];
  const requiredFilesArg = args["required-files"];
  const jsonOutput = args.json === "true" || args.json === "1";

  function emit(result) {
    if (jsonOutput) {
      console.log(JSON.stringify(result));
    } else if (result.ok) {
      console.log(`verify-npm-package: OK (${result.name}@${result.version})`);
    } else {
      console.error(`verify-npm-package: ${result.error}`);
    }
  }

  function failWith(message, extra = {}) {
    emit({ ok: false, error: message, ...extra });
    process.exit(1);
  }

  if (!packageDir || !expectedName) {
    throw new Error(
      "Usage: node scripts/verify-npm-package.mjs --package-dir <dir> --expected-name <name> [--expected-repository-url <url>] [--required-files file1,file2] [--json true|false]",
    );
  }

  const packageJsonPath = path.join(packageDir, "package.json");
  if (!fs.existsSync(packageJsonPath)) {
    failWith(`missing package.json at ${packageJsonPath}`);
  }

  const pkg = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  if (pkg.name !== expectedName) {
    failWith(`expected name ${expectedName}, got ${pkg.name ?? "<missing>"}`, {
      expectedName,
      actualName: pkg.name ?? null,
    });
  }
  if (!pkg.version) {
    failWith("missing version");
  }
  if (!pkg.repository || !pkg.repository.url) {
    failWith("missing repository.url");
  }
  if (expectedRepositoryUrl && pkg.repository.url !== expectedRepositoryUrl) {
    failWith(
      `expected repository.url ${expectedRepositoryUrl}, got ${pkg.repository.url ?? "<missing>"}`,
      {
        expectedRepositoryUrl,
        actualRepositoryUrl: pkg.repository?.url ?? null,
      },
    );
  }

  const requiredFiles = requiredFilesArg
    ? requiredFilesArg
        .split(",")
        .map((file) => file.trim())
        .filter(Boolean)
    : ["README.md", "LICENSE-MIT", "LICENSE-APACHE", "package.json"];

  const missingFiles = requiredFiles.filter(
    (file) => !fs.existsSync(path.join(packageDir, file)),
  );
  if (missingFiles.length > 0) {
    failWith(`missing required files: ${missingFiles.join(", ")}`, {
      missingFiles,
    });
  }

  emit({
    ok: true,
    name: expectedName,
    version: pkg.version,
    packageDir,
    requiredFilesChecked: requiredFiles,
  });
}

main();
