# Contributing to MXP Protocol

Thank you for your interest in contributing to MXP (Mesh eXchange Protocol). This document provides guidelines for contributing to the protocol specification and reference implementation.

## Types of Contributions

### Protocol Enhancements
- New message types or features
- Performance improvements
- Security enhancements
- Protocol extensions

### Implementation Contributions
- Bug fixes in the reference implementation
- Performance optimizations
- New language implementations
- Documentation improvements
- Test coverage improvements

### Documentation
- Technical documentation
- Examples and tutorials
- API documentation
- Translation to other languages

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally
3. **Create a branch** for your changes
4. **Make your changes** following our guidelines
5. **Test thoroughly**
6. **Submit a pull request**

## Development Setup

### Prerequisites
- Rust 1.85 or later
- Cargo
- Git

### Build and Test

```bash
# Clone the repository
git clone https://github.com/yafatek/mxp-protocol
cd mxp-protocol

# Build
cargo build

# Run tests
cargo test --lib

# Run clippy
cargo clippy --all-targets -- -D warnings

# Format code
cargo fmt --all

# Run benchmarks (optional)
cargo bench
```

## Code Standards

### Rust Code
- Follow idiomatic Rust conventions
- Use `cargo fmt` before committing
- Ensure `cargo clippy` passes with no warnings
- Write doc comments for all public APIs
- Include examples in documentation
- Minimum 80% test coverage for new code

### Performance Requirements
- No regressions in existing benchmarks
- New code in hot paths must be benchmarked
- Zero-copy operations where possible
- Minimal allocations

### Testing
- Write unit tests for all new functionality
- Include integration tests for E2E flows
- Add property-based tests for protocol validation
- Ensure all tests pass before submitting PR

## Protocol Changes

Protocol changes require additional scrutiny:

1. **Backward Compatibility**: New features must not break existing implementations
2. **Specification Update**: Update SPEC.md with complete details
3. **Reference Implementation**: Implement in Rust reference implementation
4. **Tests**: Comprehensive test coverage including edge cases
5. **Documentation**: Update all relevant documentation
6. **Discussion**: Open an issue for discussion before starting work

### Reserved Fields
Use reserved fields in the header for extensions. Do not change existing field meanings.

### Versioning
Protocol version changes follow semantic versioning:
- **Major**: Breaking changes to wire format
- **Minor**: New features, backward compatible
- **Patch**: Bug fixes, clarifications

## Pull Request Process

1. **Create an Issue**: Discuss significant changes before coding
2. **Branch Naming**: Use descriptive names (e.g., `feature/stream-compression`, `fix/checksum-validation`)
3. **Commit Messages**: 
   - Use imperative mood ("Add feature" not "Added feature")
   - First line: Brief summary (50 chars or less)
   - Body: Detailed explanation if needed
4. **Documentation**: Update relevant documentation
5. **Tests**: Include tests for new functionality
6. **PR Description**:
   - What: Describe the changes
   - Why: Explain the motivation
   - Testing: How you tested the changes
   - Breaking: Note any breaking changes

### PR Checklist

- [ ] Code follows project style guidelines
- [ ] All tests pass (`cargo test`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Documentation is updated
- [ ] Examples are included if applicable
- [ ] CHANGELOG.md is updated (for releases)

## Code Review

All submissions require review:
- At least one maintainer approval required
- Address all review comments
- Keep PR focused on single concern
- Be responsive to feedback

## Communication

- **GitHub Issues**: Bug reports, feature requests
- **GitHub Discussions**: Questions, ideas, general discussion
- **Email**: protocol@getmxp.xyz for private matters

## Code of Conduct

This project follows the [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you agree to uphold this code.

## License

By contributing, you agree that your contributions will be licensed under:
- **Protocol Specification**: CC0 (Public Domain)
- **Reference Implementation**: MIT OR Apache-2.0

You confirm that:
- You have the right to submit the contribution
- You grant the project rights to use your contribution under these licenses
- Your contribution does not violate any third-party rights

## Recognition

Contributors are recognized in:
- GitHub contributors page
- CHANGELOG.md for significant contributions
- Release notes

## Questions?

- Open a [GitHub Discussion](https://github.com/yafatek/mxp-protocol/discussions)
- Email: protocol@getmxp.xyz

Thank you for contributing to MXP!

