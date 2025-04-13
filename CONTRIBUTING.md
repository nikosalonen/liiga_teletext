# Contributing Guidelines

Thank you for your interest in contributing to this project! This document provides guidelines and steps for contributing.

## Development Workflow

1. Fork the repository
2. Create a feature branch from the main branch
   ```bash
   git checkout -b feature/your-feature-name
   ```
3. Make your changes
4. Run tests to ensure everything works
   ```bash
   cargo test --all-features
   ```
5. Commit your changes (see Commit Guidelines below)
6. Push to your fork and submit a Pull Request

## Commit Guidelines

We follow conventional commits for clear communication and automated versioning. Each commit message should be structured as follows:

```
<type>: <description>

[optional body]
[optional footer(s)]
```

Types:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation changes
- `style`: Code style changes (formatting, missing semi-colons, etc)
- `refactor`: Code refactoring
- `test`: Adding missing tests
- `chore`: Maintenance tasks

Examples:

```
feat: add support for X
fix: resolve issue with Y
docs: update installation instructions
```

## Code Style

- Follow Rust standard practices and idioms
- Use `cargo fmt` to format your code
- Run `cargo clippy` to catch common mistakes and improve code quality
- Write documentation for public APIs
- Include tests for new functionality

## Questions or Problems?

If you have questions or encounter any problems, please open an issue in the repository. We're here to help!
