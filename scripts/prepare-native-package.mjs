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
  const readmePath = args.readme;
  const cargoTomlPath = args["workspace-cargo"] ?? "Cargo.toml";

  if (!packageDir || !readmePath) {
    throw new Error(
      "Usage: node scripts/prepare-native-package.mjs --package-dir <dir> --readme <md> [--workspace-cargo Cargo.toml]",
    );
  }

  const packageJsonPath = path.join(packageDir, "package.json");
  if (!fs.existsSync(packageJsonPath)) {
    throw new Error(`Missing package.json at ${packageJsonPath}`);
  }

  const version = readWorkspaceVersion(cargoTomlPath);
  const pkg = JSON.parse(fs.readFileSync(packageJsonPath, "utf8"));
  pkg.version = version;

  fs.writeFileSync(packageJsonPath, `${JSON.stringify(pkg, null, 2)}\n`, "utf8");

  copyRequiredFile(readmePath, path.join(packageDir, "README.md"));
  copyRequiredFile("LICENSE-MIT", path.join(packageDir, "LICENSE-MIT"));
  copyRequiredFile("LICENSE-APACHE", path.join(packageDir, "LICENSE-APACHE"));
}

main();
