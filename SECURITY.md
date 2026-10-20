# Security Policy

## Reporting a vulnerability

Please report vulnerabilities privately through [GitHub's vulnerability reporting](https://github.com/blopker/dynimg/security/advisories/new), not in a public issue.

Only the latest release gets fixes.

## Scope

Most useful are reports that break the sandbox:

- Reading local files without an assets directory, or outside it
- Making network requests when network access is off

The README lists [known risks for untrusted templates](README.md#untrusted-templates): crashes from deep nesting, high CPU and memory use, requests to internal addresses when network access is on, and decoder exposure through `data:` URIs. Reports about those are still welcome if you've found a worse impact than the README describes, such as code execution.
