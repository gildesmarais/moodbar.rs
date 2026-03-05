#!/usr/bin/env node

import fs from "node:fs";
import { parseArgs } from "./args.mjs";
import { readWorkspaceVersion } from "./workspace-version.mjs";

function computeNextVersion(currentVersion, bump) {
  const match = currentVersion.match(/^(\d+)\.(\d+)\.(\d+)$/);
  if (!match) {
    throw new Error(
      `Unsupported version format "${currentVersion}". Expected plain X.Y.Z.`,
    );
  }

  const major = Number.parseInt(match[1], 10);
  const minor = Number.parseInt(match[2], 10);
  const patch = Number.parseInt(match[3], 10);

  if (bump === "major") {
    return `${major + 1}.0.0`;
  }
  if (bump === "minor") {
    return `${major}.${minor + 1}.0`;
  }
  if (bump === "patch") {
    return `${major}.${minor}.${patch + 1}`;
  }

  throw new Error(`Unsupported bump type: ${bump}`);
}

function writeWorkspaceVersion(cargoTomlPath, nextVersion) {
  const lines = fs.readFileSync(cargoTomlPath, "utf8").split(/\r?\n/);
  const sectionHeader = "[workspace.package]";
  const start = lines.findIndex((line) => line.trim() === sectionHeader);
  if (start < 0) {
    throw new Error(`Could not find [workspace.package] in ${cargoTomlPath}`);
  }

  let replaced = false;
  for (let i = start + 1; i < lines.length; i += 1) {
    if (/^\s*\[[^\]]+\]\s*$/.test(lines[i])) {
      break;
    }
    if (/^\s*version\s*=/.test(lines[i])) {
      lines[i] = `version = "${nextVersion}"`;
      replaced = true;
      break;
    }
  }

  if (!replaced) {
    throw new Error(
      `Could not find workspace.package.version in ${cargoTomlPath}`,
    );
  }

  fs.writeFileSync(cargoTomlPath, `${lines.join("\n")}\n`, "utf8");
}

function main() {
  const args = parseArgs(process.argv.slice(2));
  const cargoTomlPath = args.cargo ?? "Cargo.toml";
  const bump = args.bump;
  const write = args.write === "true" || args.write === "1";

  if (!bump) {
    throw new Error(
      "Usage: node scripts/bump-version.mjs --bump <patch|minor|major> [--cargo Cargo.toml] [--write true|false]",
    );
  }

  const current = readWorkspaceVersion(cargoTomlPath);
  const next = computeNextVersion(current, bump);

  if (write) {
    writeWorkspaceVersion(cargoTomlPath, next);
  }

  process.stdout.write(
    `${JSON.stringify({ current, next, bump, wrote: write })}\n`,
  );
}

main();
