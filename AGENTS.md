# Agent Guidelines for Tuister

This document provides instructions and context for AI agents working on the Tuister codebase.

## Build and Development Commands

Use these commands for development and verification:

```bash
# Build
cargo build
cargo build --release

# Run
cargo run

# Test
cargo test                    # Run all tests
cargo test <test_name>        # Run specific test (e.g. cargo test test_client_creation)
cargo test --lib              # Run library tests only
cargo test --bin tuister      # Run binary tests only

# Check/lint
cargo check                   # Fast compilation check
cargo clippy                  # Run linter
cargo fmt                     # Format code
```

## Architecture Overview

Tuister is split into a library and a binary to ensure core logic is reusable.

- **Library (`src/lib.rs` + modules)**: Contains the core API client, session management, and data models.
  - `OpenRouterClient`: Handles communication with OpenRouter API.
  - `ChatSession`: Manages conversation state and history.
  - `models`: Serialized data structures for API requests/responses.
  - `error`: Custom error types using `thiserror`.
- **Binary (`src/main.rs`, `src/ui.rs`)**: The TUI application.
  - `main.rs`: Entry point and main event loop.
  - `ui.rs`: Rendering logic using `ratatui`.

## Code Style Guidelines

### 1. Formatting & Naming
- Follow standard Rust naming conventions: `PascalCase` for types, `snake_case` for functions and variables.
- Always run `cargo fmt` before committing.
- Prefer descriptive variable names over short abbreviations (e.g., `transmitter` instead of `tx` in public APIs, though `tx`/`rx` is acceptable for local channels).

### 2. Imports
- Group imports: standard library first, then external crates, then crate modules.
- Use `crate::` for internal library imports.
- Example:
  ```rust
  use std::collections::HashMap;
  
  use reqwest::Client;
  use serde::Serialize;
  
  use crate::error::{Result, TuisterError};
  ```

### 3. Error Handling
- **Library**: Use `thiserror` to define `TuisterError` in `src/error.rs`. Return `crate::error::Result<T>`.
- **Binary**: Use `anyhow` for top-level error handling in `main.rs`.
- Use the `?` operator for propagating errors.
- Avoid `unwrap()` or `expect()` in library code unless it's genuinely unreachable or in tests.

### 4. Async/Await
- The project uses `tokio` as the async runtime.
- Prefer `tokio::sync::mpsc` for internal communication.
- Use `async-trait` for traits that require async methods.

### 5. Testing
- Place unit tests in a `mod tests` block at the bottom of the file with `#[cfg(test)]`.
- Use `insta` for snapshot testing where appropriate (configured in `dev-dependencies`).
- Ensure all new features have corresponding tests.

### 6. Documentation
- Use `///` for doc comments on public items.
- Maintain `CLAUDE.md` with updated architecture details if significant changes are made.

## Configuration
- API keys should never be hardcoded.
- Use `dotenvy` to load `.env` files.
- Configuration is managed in `src/config.rs`.

## Cursor & Copilot Rules
(No project-specific Cursor or Copilot rules found in `.cursorrules` or `.github/copilot-instructions.md`)

## Development Workflow

1.  **Exploration**: Use `grep` and `glob` to find relevant components.
2.  **Implementation**: Follow the library-first approach. Implement logic in the library modules before updating the TUI.
3.  **Verification**:
    - Run `cargo check` for fast feedback.
    - Run `cargo test` to ensure no regressions.
    - Run `cargo clippy` and `cargo fmt` to maintain code quality.
    - Manually test the TUI by running `cargo run` if possible (though terminal input may be limited for automated agents).

## Architecture Details

### Data Models (`src/models.rs`)
- `ChatMessage`: The core message structure with `role` and `content`.
- `Model`: Represents an LLM available on OpenRouter.
- `StreamResponse`: Parsed from SSE chunks during streaming.

### Error Handling (`src/error.rs`)
```rust
#[derive(Error, Debug)]
pub enum TuisterError {
    #[error("API error: {0}")]
    ApiError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    // ... other variants
}
```

### API Client (`src/client.rs`)
- Uses `reqwest::Client`.
- Implements streaming via `send_message_streaming` which returns an `mpsc` receiver.

## Design Principles
- **Separation of Concerns**: Keep TUI rendering logic (`ui.rs`) separate from business logic (`chat.rs`).
- **Streaming First**: Prioritize streaming implementations for better user experience.
- **Robustness**: Handle API errors gracefully, providing fallbacks where possible (see `models.rs` for curated fallback models).
- **Tool Support**: The project supports tool calling (function calling). Check `src/tools.rs` for implementation details.
