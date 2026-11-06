#!/usr/bin/env node
/**
 * GameScape repository privacy scan.
 *
 * Fails (exit 1) if tracked files look like they contain:
 *   - secrets, private keys, credentials files
 *   - environment files (`.env`), local databases, logs/dumps
 *   - personal data patterns (user paths, non-allowlisted emails)
 *   - media files that must never live in the repository
 *   - images outside the approved location allowlist
 *
 * This is a fast, deterministic gate. It supplements (never replaces)
 * GitHub secret scanning + push protection and gitleaks.
 *
 * Usage: node scripts/privacy-scan.mjs
 */

import { execFileSync } from "node:child_process";
import { readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const SELF = "scripts/privacy-scan.mjs";

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

const SKIP_TEXT_SCAN = new Set([SELF, "pnpm-lock.yaml", "Cargo.lock"]);

const ALLOWED_USERS = new Set([
  "example",
  "user",
  "username",
  "yourname",
  "name",
  "public",
  "runner",
  "runneradmin",
]);

const ALLOWED_EMAIL_DOMAINS = new Set([
  "example.com",
  "example.org",
  "example.net",
  "users.noreply.github.com",
  "noreply.github.com",
  "contributor-covenant.org",
  "apache.org",
]);

const IMAGE_EXT = /\.(png|jpe?g|webp|avif|ico|svg)$/i;
const MEDIA_EXT = /\.(mp4|mov|mkv|avi|webm|gif)$/i;

const IMAGE_ALLOWED_PREFIXES = [
  "assets/branding/",
  "apps/website/src/assets/",
  "apps/website/public/",
  "packages/catalogue/thumbnails/",
  "packages/ui/src/assets/",
  "apps/desktop/src-tauri/icons/",
  "docs/assets/",
  ".github/",
];

const NAME_RULES = [
  {
    rule: "environment-file",
    re: /(^|\/)\.env(\.|$)/,
    allow: (p) => p.endsWith(".env.example"),
  },
  {
    rule: "private-key-material",
    re: /\.(pem|key|p12|pfx|jks|keystore|mobileprovision)$/i,
    allow: (p) => p.includes("test-fixtures/"),
  },
  {
    rule: "ssh-private-key",
    re: /(^|\/)(id_rsa|id_ed25519)(\.|$)/,
  },
  {
    rule: "local-database",
    re: /\.(db|sqlite|sqlite3|db3)([-.].*)?$/i,
  },
  {
    rule: "signature-file",
    re: /\.sig$/i,
    allow: (p) => p.includes("test-fixtures/"),
  },
  {
    rule: "credentials-file",
    re: /(^|\/)credentials[^/]*\.json$/i,
    allow: (p) => p.includes("test-fixtures/"),
  },
  {
    rule: "log-or-dump",
    re: /\.(log|dmp|trace|etl)$/i,
  },
  {
    rule: "media-file",
    re: MEDIA_EXT,
  },
];

const TEXT_RULES = [
  {
    rule: "windows-user-path",
    re: /[A-Za-z]:\\Users\\([^\\/\s"'<>]+)/g,
    check: (m) => !ALLOWED_USERS.has(m[1].toLowerCase()),
  },
  {
    rule: "unix-home-path",
    re: /\/(?:home|Users)\/([A-Za-z0-9._-]+)/g,
    check: (m) => !ALLOWED_USERS.has(m[1].toLowerCase()),
  },
  { rule: "private-key-block", re: /-----BEGIN [A-Z ]*PRIVATE KEY-----/g },
  { rule: "token-github", re: /\bgh[opusr]_[A-Za-z0-9]{20,}\b/g },
  { rule: "token-github-pat", re: /\bgithub_pat_[A-Za-z0-9_]{30,}\b/g },
  { rule: "token-aws", re: /\bAKIA[0-9A-Z]{12,}\b/g },
  { rule: "token-slack", re: /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/g },
  { rule: "token-google", re: /\bAIza[0-9A-Za-z_-]{30,}\b/g },
  { rule: "token-stripe", re: /\bsk_(live|test)_[A-Za-z0-9]{10,}\b/g },
  { rule: "token-npm", re: /\bnpm_[A-Za-z0-9]{30,}\b/g },
  {
    rule: "email",
    // Requires an alphabetic TLD (avoids false positives like `pkg@1.2.3`).
    re: /[\w.+-]+@[\w-]+(?:\.[\w-]+)*\.[A-Za-z]{2,}\b/g,
    check: (m) => !ALLOWED_EMAIL_DOMAINS.has(m[0].split("@")[1].toLowerCase()),
  },
];

// ---------------------------------------------------------------------------
// File listing
// ---------------------------------------------------------------------------

function listTrackedFiles() {
  const out = execFileSync("git", ["ls-files", "-z"], {
    cwd: ROOT,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
  });
  return out
    .split("\0")
    .filter(Boolean)
    .map((p) => p.replaceAll("\\", "/"));
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function readFileIfPossible(rel) {
  const abs = path.join(ROOT, rel);
  let st;
  try {
    st = statSync(abs);
  } catch {
    return null;
  }
  if (!st.isFile()) return null;
  const maxBytes = 4 * 1024 * 1024;
  const buf = st.size > maxBytes ? readFileSync(abs).subarray(0, maxBytes) : readFileSync(abs);
  return { buf, truncated: st.size > maxBytes, size: st.size };
}

function isBinary(buf) {
  const len = Math.min(buf.length, 8192);
  for (let i = 0; i < len; i += 1) if (buf[i] === 0) return true;
  return false;
}

// ---------------------------------------------------------------------------
// Scan
// ---------------------------------------------------------------------------

const findings = [];
const files = listTrackedFiles();

for (const rel of files) {
  // 1) Filename rules
  for (const { rule, re, allow } of NAME_RULES) {
    if (re.test(rel) && !(allow && allow(rel))) {
      findings.push({ rule, file: rel, detail: rel });
      break;
    }
  }

  // 2) Image location allowlist
  if (IMAGE_EXT.test(rel)) {
    const allowed = IMAGE_ALLOWED_PREFIXES.some((p) => rel.startsWith(p) && rel.length > p.length);
    if (!allowed) {
      findings.push({ rule: "image-outside-allowlist", file: rel, detail: rel });
    }
  }

  // 3) Content
  if (SKIP_TEXT_SCAN.has(rel)) continue;
  const data = readFileIfPossible(rel);
  if (!data) continue;

  // SQLite database magic
  if (data.buf.subarray(0, 16).toString("latin1").startsWith("SQLite format 3\0")) {
    findings.push({ rule: "sqlite-database-content", file: rel, detail: "SQLite header detected" });
    continue;
  }

  if (isBinary(data.buf)) continue;

  const text = data.buf.toString("utf8");
  const lines = text.split(/\r?\n/);

  for (const { rule, re, check } of TEXT_RULES) {
    re.lastIndex = 0;
    for (let ln = 0; ln < lines.length; ln += 1) {
      const re2 = new RegExp(re.source, re.flags);
      let m;
      while ((m = re2.exec(lines[ln])) !== null) {
        if (!check || check(m)) {
          findings.push({
            rule,
            file: rel,
            detail: `line ${ln + 1}: ${m[0].slice(0, 120)}`,
          });
        }
        if (m.index === re2.lastIndex) re2.lastIndex += 1;
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

console.log("GameScape privacy scan");
console.log(`Scanned ${files.length} tracked files.`);

if (findings.length > 0) {
  console.error(`\n\u274c ${findings.length} finding(s):\n`);
  for (const f of findings) {
    console.error(`  - [${f.rule}] ${f.file} (${f.detail})`);
  }
  console.error(
    "\nFix the findings above. If a finding is a deliberate test fixture," +
      " place it under a `test-fixtures/` directory where the rule allows it," +
      " or refine scripts/privacy-scan.mjs (with review).",
  );
  process.exit(1);
}

console.log("\u2705 No privacy findings.");
