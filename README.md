# Tuister

A Terminal User Interface (TUI) application for chatting with 1 to 3 Large Language Models (LLMs) simultaneously using OpenRouter as the backend.

## Features

- **Multi-Model Chat**: Chat with 1, 2, or 3 LLMs at the same time
- **TUI Interface**: Clean, terminal-based interface built with Ratatui
- **Library Architecture**: Core functionality separated into a library for future UI implementations
- **OpenRouter Integration**: Uses OpenRouter API to access multiple LLM providers

## Installation

### Prerequisites

- Rust 1.70 or later
- An OpenRouter API key ([Get one here](https://openrouter.ai/))

### Building from Source

```bash
git clone https://github.com/lhahne/tuister.git
cd tuister
cargo build --release
```

The compiled binary will be available at `target/release/tuister`.

## Usage

### Setting up API Key

Set your OpenRouter API key as an environment variable:

```bash
export OPENROUTER_API_KEY=your_api_key_here
```

### Running the Application

```bash
cargo run
```

Or if you've built the release binary:

```bash
./target/release/tuister
```

### Controls

#### Chat Mode
- **Type** to enter your message
- **Enter** to send message to all active models
- **Tab** to cycle between 1, 2, or 3 active models
- **↑/↓** to scroll through chat history
- **Ctrl+M** to open model selection
- **Ctrl+C** or **q** to quit

#### Model Selection Mode
- **↑/↓** to navigate through available models
- **Space** or **Enter** to toggle model selection
- **Ctrl+M** to return to chat
- **Ctrl+C** or **q** to quit

## Available Models

The application comes with 10 pre-configured models you can choose from:
1. GPT-3.5 Turbo (OpenAI)
2. GPT-4 (OpenAI)
3. GPT-4 Turbo (OpenAI)
4. Claude 3 Haiku (Anthropic)
5. Claude 3 Sonnet (Anthropic)
6. Claude 3 Opus (Anthropic)
7. Gemini Flash 1.5 (Google)
8. Gemini Pro 1.5 (Google)
9. Llama 3 70B (Meta)
10. Mistral 7B (Mistral AI)

By default, the first 3 models are selected when you start the app. Use **Ctrl+M** to open the model selection screen and choose which models you want to chat with.

## Architecture

The project is split into two main parts:

### Library (`src/lib.rs`)

The core library provides reusable components:
- `client`: OpenRouter API client implementation
- `models`: Data structures for messages, models, and API responses
- `chat`: Chat session management
- `error`: Error types and handling

### Binary (`src/main.rs`)

The TUI application built with:
- **Ratatui**: Terminal UI framework
- **Crossterm**: Terminal manipulation
- **Tokio**: Async runtime for API calls

This separation allows other UIs (web, GUI, etc.) to be built on top of the same core library.

## Project Structure

```
tuister/
├── src/
│   ├── lib.rs          # Library entry point
│   ├── main.rs         # TUI application
│   ├── client.rs       # OpenRouter API client
│   ├── models.rs       # Data models
│   ├── chat.rs         # Chat session logic
│   ├── error.rs        # Error types
│   └── ui.rs           # TUI rendering
├── Cargo.toml          # Dependencies
└── README.md           # This file
```

## Dependencies

- **tokio**: Async runtime
- **reqwest**: HTTP client for API calls
- **serde**: Serialization/deserialization
- **ratatui**: Terminal UI framework
- **crossterm**: Terminal manipulation
- **anyhow**: Error handling in main
- **thiserror**: Error type definitions

## Future Enhancements

- [ ] Configuration file support
- [ ] Custom model selection
- [ ] Chat history persistence
- [ ] Multiple chat sessions
- [ ] Syntax highlighting for code blocks
- [ ] Additional UI implementations (web, desktop GUI)

## License

See [LICENSE](LICENSE) file for details.
