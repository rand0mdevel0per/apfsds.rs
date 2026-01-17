# Contributing to APFSDS

Thank you for your interest in contributing to APFSDS! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Workflow](#development-workflow)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)

---

## Code of Conduct

We are committed to providing a welcoming and inclusive community. Please be respectful and constructive in all interactions.

---

## Getting Started

### Prerequisites

- Rust 1.85+ (nightly, 2024 edition)
- Git
- PostgreSQL 14+ (optional, for integration tests)
- ClickHouse 23+ (optional, for analytics tests)

### Setting Up Development Environment

```bash
# Clone the repository
git clone https://github.com/rand0mdevel0per/apfsds.rs.git
cd apfsds

# Install Rust nightly
rustup install nightly
rustup default nightly

# Build
cargo build

# Run tests
cargo test --workspace
```

### Repository Structure

```
apfsds/
├── crates/          # Core libraries
│   ├── protocol/    # Wire protocol
│   ├── crypto/      # Cryptography
│   ├── transport/   # Network layer
│   ├── obfuscation/ # Traffic masking
│   ├── storage/     # MVCC engine
│   └── raft/        # Consensus
├── daemon/          # Server binary
├── client/          # Client binary
├── cli/             # Management CLI
├── tests/           # Integration tests
├── docs/            # Documentation
└── helm-chart/      # Kubernetes
```

---

## Development Workflow

### Branching Strategy

- `master` - Stable release branch
- `develop` - Integration branch for features
- `feature/*` - Feature branches
- `fix/*` - Bug fix branches
- `release/*` - Release preparation

### Creating a Feature Branch

```bash
git checkout develop
git pull origin develop
git checkout -b feature/my-amazing-feature
```

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

Types:
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Adding tests
- `chore`: Maintenance

Examples:
```
feat(transport): add QUIC connection pooling
fix(crypto): handle invalid key length gracefully
docs(readme): update installation instructions
```

---

## Coding Standards

### Rust Style

We follow the official [Rust Style Guide](https://doc.rust-lang.org/nightly/style-guide/).

```bash
# Format code
cargo fmt

# Check lints
cargo clippy --workspace -- -D warnings
```

### Error Handling

- Use `thiserror` for library errors
- Use `anyhow` for application errors
- Propagate errors with `?`; avoid `.unwrap()` in production code

```rust
// Good
fn process_data(data: &[u8]) -> Result<Output, ProcessError> {
    let parsed = parse(data)?;
    Ok(transform(parsed))
}

// Avoid
fn process_data(data: &[u8]) -> Output {
    let parsed = parse(data).unwrap();
    transform(parsed)
}
```

### Documentation

- Add doc comments (`///`) to all public items
- Include examples where helpful
- Document panics and safety concerns

```rust
/// Encrypts the given plaintext using AES-256-GCM.
///
/// # Arguments
///
/// * `plaintext` - Data to encrypt
///
/// # Returns
///
/// Encrypted ciphertext with 12-byte nonce prefix
///
/// # Example
///
/// ```
/// let ciphertext = cipher.encrypt(b"hello world")?;
/// ```
pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, CryptoError> {
    // ...
}
```

---

## Testing

### Running Tests

```bash
# All unit tests
cargo test --workspace

# Specific crate
cargo test -p apfsds-protocol

# Integration tests (requires running services)
cargo test -p apfsds-tests --test handshake -- --ignored

# With logging
RUST_LOG=debug cargo test
```

### Writing Tests

- Place unit tests in `#[cfg(test)] mod tests` at file bottom
- Place integration tests in `tests/` directory
- Use descriptive test names

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let cipher = AesGcmCipher::new(&[0u8; 32]);
        let plaintext = b"hello world";
        
        let ciphertext = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&ciphertext).unwrap();
        
        assert_eq!(plaintext, decrypted.as_slice());
    }

    #[test]
    fn test_decrypt_invalid_data_returns_error() {
        let cipher = AesGcmCipher::new(&[0u8; 32]);
        
        let result = cipher.decrypt(b"invalid");
        
        assert!(result.is_err());
    }
}
```

### Test Coverage

We aim for >80% test coverage on critical paths:
- `crypto/` - 95%+
- `protocol/` - 90%+
- `storage/` - 85%+

---

## Pull Request Process

### Before Submitting

1. **Ensure tests pass**: `cargo test --workspace`
2. **Format code**: `cargo fmt`
3. **Check lints**: `cargo clippy --workspace`
4. **Update documentation** if needed
5. **Add tests** for new functionality

### Submitting a PR

1. Push your branch to your fork
2. Open a Pull Request against `develop`
3. Fill out the PR template
4. Request review from maintainers

### PR Template

```markdown
## Description
Brief description of changes

## Type of Change
- [ ] Bug fix
- [ ] New feature
- [ ] Breaking change
- [ ] Documentation update

## Testing
Describe testing performed

## Checklist
- [ ] Tests pass locally
- [ ] Code formatted with `cargo fmt`
- [ ] No clippy warnings
- [ ] Documentation updated
- [ ] Changelog updated (if applicable)
```

### Review Process

1. At least one maintainer approval required
2. CI checks must pass
3. Merge conflicts resolved
4. Squash and merge for clean history

---

## Release Process

Releases are managed by maintainers:

1. Version bump in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create release branch: `release/v0.2.0`
4. PR to `master`
5. Tag and GitHub release
6. Publish to crates.io

---

## Getting Help

- **Issues**: For bugs and feature requests
- **Discussions**: For questions and ideas
- **Discord**: [Coming soon]

---

Thank you for contributing! 🎉
