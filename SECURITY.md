# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | Yes       |

## Reporting a Vulnerability

**Please do not report security vulnerabilities through public GitHub issues.**

If you believe you have found a security vulnerability in Saikuro, please
report it by opening a
[GitHub Security Advisory](https://github.com/Nisoku/Saikuro/security/advisories/new)
on this repository.

Include as much of the following information as possible to help us understand
and reproduce the issue:

- The type of vulnerability (e.g. buffer overflow, injection, privilege escalation)
- The affected component (crate name, adapter language, transport type)
- Full paths to the relevant source files
- Step-by-step instructions to reproduce the issue
- Proof-of-concept or exploit code (if available)
- Impact assessment: what an attacker could achieve

We aim to acknowledge receipt within **3 business days** and to provide a
remediation timeline within **7 business days**.

## Disclosure Policy

We follow a coordinated disclosure model. Once a fix is available we will:

1. Release a patched version.
2. Publish a GitHub Security Advisory crediting the reporter (unless anonymity
   is requested).
3. Add a note in the `CHANGELOG.md` under the patched version.
