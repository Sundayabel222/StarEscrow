# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | ✅ Yes    |
| < 0.1.0 | ❌ No     |

Only the latest release receives security fixes. Please upgrade before reporting an issue.

## Reporting a Vulnerability

**Do not open a public GitHub issue for security vulnerabilities.**

Report vulnerabilities via GitHub's private [Security Advisories](https://github.com/The-Pantseller/StarEscrow/security/advisories/new) feature.

Please include:
- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof-of-concept
- Affected versions

You can expect an acknowledgement within **72 hours** and a status update within **7 days**.

## Disclosure Policy

- Vulnerabilities are kept confidential until a fix is released.
- Credit will be given to reporters in the release notes unless anonymity is requested.
- We follow [coordinated disclosure](https://en.wikipedia.org/wiki/Coordinated_vulnerability_disclosure): please allow reasonable time for a fix before public disclosure.

## Scope

This policy covers the `contracts/escrow` Soroban smart contract and the `clients/cli` package. For the threat model and known attack vectors, see [docs/SECURITY.md](docs/SECURITY.md).
