# Security Policy

## Supported versions

GameScape has not had its first public release yet. Once v1.0 ships, this table will list supported versions.

| Version | Supported |
|---|---|
| Pre-release / development builds | ❌ Not supported |

## Reporting a vulnerability

**Please do not report security vulnerabilities in public issues.**

Use GitHub's **private vulnerability reporting**:

1. Go to the repository's **Security** tab.
2. Select **Report a vulnerability**.
3. Provide the details below.

If you cannot use GitHub, contact details for a private channel will be listed here before the first public release.

### What to include

- A description of the vulnerability and its impact.
- Steps to reproduce (or a proof of concept) — safely, without harming other users.
- The GameScape version/build and Windows version (for app issues).
- Any suggested fix or mitigation, if you have one.

### What not to do

- Do not test against other users, their machines, or their data.
- Do not open public issues, pull requests, or discussions about the vulnerability before it is fixed.
- Do not access, modify, or exfiltrate data that is not yours.

## Our commitments

- **Acknowledge:** within 72 hours (best effort — this is a small maintainer team).
- **Triage:** initial assessment within 7 days.
- **Fix:** critical issues as quickly as possible with an out-of-band patch; high-severity issues targeted for the next patch release; others scheduled. Timelines are best-effort targets, stated honestly.
- **Disclosure:** coordinated. We aim to publish a fix and a security advisory before details become public, with a default 90-day disclosure window.
- **Credit:** we are happy to credit reporters in advisories (opt-in). There is no bug bounty program.

## Safe harbor

We consider good-faith security research conducted within the rules above to be authorised. We will not pursue legal action against researchers who act in good faith, follow this policy, and avoid privacy violations, data destruction, and service disruption.

## Scope

**In scope:** the GameScape desktop application, the public website and catalogue pipeline, the build/release automation in this repository.

**Out of scope:** third-party services (GitHub, Microsoft Edge/WebView2, game launchers), social engineering, physical attacks, denial-of-service against GitHub-hosted infrastructure.
