# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build and Development Commands

```bash
# Build
cargo build
cargo build --release

# Run
cargo run

# Test
cargo test                    # Run all tests
cargo test <test_name>        # Run specific test
cargo test --lib              # Run library tests only

# Check/lint
cargo check
cargo clippy
```

## Architecture

Tuister is a terminal-based multi-model LLM chat application that uses OpenRouter as its backend. The codebase is split into a reusable library and a TUI binary.

### Library (`src/lib.rs`)
The library exposes core functionality for building LLM chat interfaces:

- **`OpenRouterClient`** (`src/client.rs`): HTTP client for OpenRouter API
  - `fetch_models()`: Gets available models from `/api/v1/models`
  - `send_message()`: Non-streaming chat completion
  - `send_message_streaming()`: SSE streaming via tokio mpsc channels

- **`ChatSession`** (`src/chat.rs`): Manages conversation state
  - Holds the client, selected models, and message history
  - `send_to_model_streaming()`: Streams response from a single model
  - `send_to_all_models()`: Broadcasts to all selected models

- **`models`** (`src/models.rs`): Data structures for API requests/responses
  - `ChatMessage`, `Role`, `Model` for chat data
  - `StreamResponse`, `StreamDelta` for SSE parsing

### Binary (`src/main.rs` + `src/ui.rs`)
The TUI application uses Ratatui/Crossterm with two modes:

- **Chat Mode**: Side-by-side panes showing responses from 1-3 models
- **Model Selection Mode**: List interface to choose which models to use

The `App` struct in `ui.rs` owns the `ChatSession` and handles all UI state (input buffer, scroll position, active models, loading state).

### Data Flow
1. User types message → stored in `App.input`
2. Enter pressed → `submit_message()` adds to `ChatSession.messages`
3. For each active model: spawns streaming task via `send_to_model_streaming()`
4. Chunks received via mpsc channel → accumulated and displayed in model panes

## Configuration

API key is read from `OPENROUTER_API_KEY` environment variable. The app auto-loads `.env` files via dotenvy.
