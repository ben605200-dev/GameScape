#!/usr/bin/env node
/**
 * GameScape community-standards file check.
 *
 * Verifies the repository keeps the files GitHub's community profile
 * checklist expects, with the metadata keys GitHub requires for
 * issue templates:
 *   - issue forms (.yml):  name + description  (validated here)
 *   - issue templates (.md): name + about      (none used yet)
 *
 * IMPORTANT: this check supplements but never replaces manual verification
 * of GitHub's own "Insights -> Community" page, which is the authoritative
 * source of truth (supported locations, recognized standard texts, etc.).
 *
 * Usage: node scripts/community-standards-check.mjs
 */

import { readFileSync, existsSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");

const errors = [];
const warnings = [];

function read(rel) {
  return readFileSync(path.join(ROOT, rel), "utf8");
}

function exists(rel) {
  try {
    return statSync(path.join(ROOT, rel)).isFile();
  } catch {
    return false;
  }
}

// ---------------------------------------------------------------------------
// Required files
// ---------------------------------------------------------------------------

const REQUIRED_FILES = [
  "README.md",
  "LICENSE",
  "CODE_OF_CONDUCT.md",
  "CONTRIBUTING.md",
  "SECURITY.md",
  "SUPPORT.md",
  ".github/PULL_REQUEST_TEMPLATE.md",
  ".github/ISSUE_TEMPLATE/config.yml",
];

const ISSUE_FORMS = [
  ".github/ISSUE_TEMPLATE/bug_report.yml",
  ".github/ISSUE_TEMPLATE/feature_request.yml",
  ".github/ISSUE_TEMPLATE/content_report.yml",
  ".github/ISSUE_TEMPLATE/copyright_takedown.yml",
];

for (const rel of [...REQUIRED_FILES, ...ISSUE_FORMS]) {
  if (!exists(rel)) {
    errors.push(`Missing required file: ${rel}`);
  } else if (read(rel).trim().length === 0) {
    errors.push(`File is empty: ${rel}`);
  }
}

// ---------------------------------------------------------------------------
// Issue form metadata (keys GitHub recognizes)
// ---------------------------------------------------------------------------

for (const rel of ISSUE_FORMS) {
  if (!exists(rel)) continue;
  const content = read(rel);
  if (!/^name:\s*\S+/m.test(content)) {
    errors.push(`Issue form missing top-level \`name:\` key: ${rel}`);
  }
  if (!/^description:\s*\S+/m.test(content)) {
    errors.push(`Issue form missing top-level \`description:\` key: ${rel}`);
  }
  if (!/^body:/m.test(content)) {
    warnings.push(`Issue form has no \`body:\` (GitHub may render it as an empty form): ${rel}`);
  }
}

// ---------------------------------------------------------------------------
// Required sections / content expectations
// ---------------------------------------------------------------------------

const contentChecks = [
  { rel: "README.md", test: /##\s/m, message: "README should contain section headings (##)" },
  { rel: "README.md", test: /license/i, message: "README should mention the license" },
  { rel: "README.md", test: /install/i, message: "README should include installation information" },
  { rel: "README.md", test: /support/i, message: "README should link to support" },
  { rel: "SECURITY.md", test: /report/i, message: "SECURITY.md must explain how to report vulnerabilities" },
  { rel: "SECURITY.md", test: /vulnerab/i, message: "SECURITY.md should explicitly discuss vulnerabilities" },
  { rel: "CONTRIBUTING.md", test: /pull request/i, message: "CONTRIBUTING should describe the pull request process" },
  { rel: "CODE_OF_CONDUCT.md", test: /Contributor Covenant/, message: "CODE_OF_CONDUCT should be the Contributor Covenant text" },
  { rel: "CONTRIBUTING.md", test: /Signed-off-by|DCO/i, message: "CONTRIBUTING should describe the DCO sign-off requirement" },
];

for (const { rel, test, message } of contentChecks) {
  if (exists(rel) && !test.test(read(rel))) {
    errors.push(`${message} (${rel})`);
  }
}

// ---------------------------------------------------------------------------
// Placeholder warnings (not failures — must be resolved before public launch)
// ---------------------------------------------------------------------------

try {
  const files = execFileSync("git", ["ls-files", "-z"], { cwd: ROOT, encoding: "utf8" })
    .split("\0")
    .filter(Boolean);
  const placeholderHits = [];
  for (const rel of files) {
    if (rel === "scripts/community-standards-check.mjs") continue;
    if (!/\.(md|yml|yaml|json|mjs|ts|tsx|rs)$/.test(rel)) continue;
    const content = readFileSync(path.join(ROOT, rel), "utf8");
    if (/@OWNER|OWNER\/gamescape/.test(content)) {
      placeholderHits.push(rel);
    }
  }
  if (placeholderHits.length > 0) {
    warnings.push(
      `Placeholder "OWNER" still present in: ${placeholderHits.join(", ")} — resolve before the repository goes public.`,
    );
  }
} catch {
  // git unavailable; skip placeholder scan
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

console.log("GameScape community standards check");
console.log(`Required files checked: ${REQUIRED_FILES.length + ISSUE_FORMS.length}`);

if (warnings.length > 0) {
  console.log("");
  for (const w of warnings) console.log(`\u26a0\ufe0f  ${w}`);
}

if (errors.length > 0) {
  console.error("");
  for (const e of errors) console.error(`\u274c ${e}`);
  console.error(
    "\nNote: this check supplements manual verification of GitHub's" +
      " Insights -> Community page, which remains the source of truth.",
  );
  process.exit(1);
}

console.log("\u2705 Community files look good.");
console.log(
  "Reminder: verify the repository's Insights -> Community page before any public release.",
);
