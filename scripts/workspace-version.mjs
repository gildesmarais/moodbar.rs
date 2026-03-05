#!/usr/bin/env node

import fs from "node:fs";

export function readWorkspaceVersion(cargoTomlPath = "Cargo.toml") {
  const cargoToml = fs.readFileSync(cargoTomlPath, "utf8");
  const sectionMatch = cargoToml.match(/\[workspace\.package\]([\s\S]*?)(\n\[|$)/);
  if (!sectionMatch) {
    throw new Error(`Could not find [workspace.package] in ${cargoTomlPath}`);
  }
  const versionMatch = sectionMatch[1].match(/\bversion\s*=\s*"([^"]+)"/);
  if (!versionMatch) {
    throw new Error(`Could not find workspace.package.version in ${cargoTomlPath}`);
  }
  return versionMatch[1];
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const cargoTomlPath = process.argv[2] ?? "Cargo.toml";
  process.stdout.write(readWorkspaceVersion(cargoTomlPath));
}
