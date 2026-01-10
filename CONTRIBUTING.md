# Contributing to Tuister

Thank you for your interest in contributing to Tuister!

## Development Setup

### Prerequisites
- Rust 1.70 or later
- An OpenRouter API key for testing

### Getting Started

1. Clone the repository:
```bash
git clone https://github.com/lhahne/tuister.git
cd tuister
```

2. Set up your environment:
```bash
cp .env.example .env
# Edit .env and add your OpenRouter API key
```

3. Build and test:
```bash
cargo build
cargo test
cargo clippy
```

## Project Structure

- `src/lib.rs` - Library entry point
- `src/main.rs` - TUI application entry point
- `src/client.rs` - OpenRouter API client
- `src/models.rs` - Data models and types
- `src/chat.rs` - Chat session management
- `src/error.rs` - Error types
- `src/ui.rs` - Terminal UI rendering

## Coding Standards

### Style
- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common mistakes

### Testing
- Add unit tests for new functionality
- Ensure all tests pass before submitting PR
- Aim for meaningful test coverage

### Documentation
- Add doc comments for public APIs
- Update README.md for user-facing changes
- Update ARCHITECTURE.md for design changes

## Making Changes

1. Create a new branch for your feature:
```bash
git checkout -b feature/your-feature-name
```

2. Make your changes following the coding standards

3. Test thoroughly:
```bash
cargo test
cargo clippy
cargo build --release
```

4. Commit with clear messages:
```bash
git commit -m "Add feature: clear description"
```

5. Push and create a pull request

## Areas for Contribution

### High Priority
- [ ] Configuration file support (TOML/YAML)
- [ ] Custom model selection in UI
- [ ] Chat history persistence
- [ ] Better error messages in UI

### Medium Priority
- [ ] Syntax highlighting for code in responses
- [ ] Multiple chat sessions
- [ ] Export chat to file
- [ ] Model response streaming

### Future Ideas
- [ ] Web UI using Axum
- [ ] Desktop GUI with egui/Tauri
- [ ] Plugin system for custom models
- [ ] Response caching

## Questions?

Feel free to open an issue for:
- Bug reports
- Feature requests
- Questions about the code
- Suggestions for improvements

## License

By contributing, you agree that your contributions will be licensed under the same license as the project.
