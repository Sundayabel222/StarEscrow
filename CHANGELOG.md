# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- GitHub issue templates for bug reports and feature requests
- Pull request template with comprehensive checklist
- CLI installation documentation in README
- CHANGELOG.md following Keep a Changelog format

### Changed
- Enhanced README with detailed CLI build and installation instructions

### Deprecated
- N/A

### Removed
- N/A

### Fixed
- N/A

### Security
- N/A

## [0.1.0] - 2024-01-01

### Added
- Initial release of StarEscrow protocol
- Soroban smart contract for escrow management
- CLI client for contract interaction
- Support for yield protocol integration
- Configurable fee structure (basis points)
- State machine implementation (Active, WorkSubmitted, Completed, Cancelled, Expired)
- Event emission for all state transitions
- Comprehensive test suite with snapshot testing
- Documentation:
  - Protocol specification
  - Deployment guide
  - Security and threat model
- CI/CD pipeline with GitHub Actions
- Code of Conduct and Contributing guidelines

### Features
- Create escrow with optional deadline and yield protocol
- Submit work (freelancer)
- Approve work and release payment (payer)
- Cancel escrow (payer, before submission)
- Expire escrow (payer, after deadline)
- Fee deduction and collection
- Yield accrual and withdrawal

### Security
- Reentrancy protection
- Integer overflow/underflow checks
- Access control for all sensitive operations
- Time-based expiration logic
- Secure token transfer patterns

## [0.0.1] - 2023-12-01

### Added
- Initial project structure
- Basic contract skeleton
- README with project overview
- MIT License

---

## Version History

- **[Unreleased]** - GitHub templates, documentation improvements
- **[0.1.0]** - First stable release with complete escrow functionality
- **[0.0.1]** - Initial project setup

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct and the process for submitting pull requests.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
