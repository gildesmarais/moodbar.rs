#!/usr/bin/env node

import fs from "node:fs";
import path from "node:path";
import { parseArgs } from "./args.mjs";
import { readWorkspaceVersion } from "./workspace-version.mjs";

function copyRequiredFile(src, dest) {
  if (!fs.existsSync(src)) {
    throw new Error(`Required file missing: ${src}`);
  }
  fs.copyFileSync(src, dest);
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const packageDir = args["package-dir"];
  const packageJsonSourcePath = args["package-json-source"];
  const readmeSourcePath = args["readme-source"];
  const cargoTomlPath = args["workspace-cargo"] ?? "Cargo.toml";

  if (!packageDir || !packageJsonSourcePath || !readmeSourcePath) {
    throw new Error(
      "Usage: node scripts/prepare-npm-package.mjs --package-dir <dir> --package-json-source <json> --readme-source <md> [--workspace-cargo Cargo.toml]",
    );
  }

  const packageJsonPath = path.join(packageDir, "package.json");
  if (!fs.existsSync(packageJsonPath)) {
    throw new Error(
      `Missing generated package at ${packageJsonPath}. Build package first.`,
    );
  }

  const version = readWorkspaceVersion(cargoTomlPath);
  const generatedPackage = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  const packageSource = JSON.parse(
    fs.readFileSync(packageJsonSourcePath, "utf8"),
  );
  const mergedPackage = {
    ...generatedPackage,
    ...packageSource,
    version,
  };

  fs.writeFileSync(
    packageJsonPath,
    `${JSON.stringify(mergedPackage, null, 2)}\n`,
    "utf8",
  );

  copyRequiredFile(readmeSourcePath, path.join(packageDir, "README.md"));
  copyRequiredFile("LICENSE-MIT", path.join(packageDir, "LICENSE-MIT"));
  copyRequiredFile("LICENSE-APACHE", path.join(packageDir, "LICENSE-APACHE"));
}

main();
