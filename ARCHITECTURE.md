# Tuister Architecture

## Component Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        TUI Application                          │
│                        (src/main.rs)                            │
│                                                                 │
│  ┌───────────────────────────────────────────────────────────┐ │
│  │                    UI Layer (src/ui.rs)                   │ │
│  │  - Renders chat interface                                 │ │
│  │  - Handles model selection display                        │ │
│  │  - Input/output rendering                                 │ │
│  │  - Scroll management                                      │ │
│  └───────────────────────────────────────────────────────────┘ │
│                              │                                  │
│                              ▼                                  │
└──────────────────────────────┼──────────────────────────────────┘
                               │
┌──────────────────────────────┼──────────────────────────────────┐
│                              ▼                                  │
│                    Tuister Library (src/lib.rs)                │
│                                                                 │
│  ┌──────────────────┐  ┌──────────────────┐  ┌──────────────┐ │
│  │  ChatSession     │  │  OpenRouter      │  │   Models     │ │
│  │  (src/chat.rs)   │◄─│  Client          │  │  (src/       │ │
│  │                  │  │  (src/client.rs) │  │   models.rs) │ │
│  │ - Message mgmt   │  │                  │  │              │ │
│  │ - Model tracking │  │ - HTTP requests  │  │ - ChatMsg    │ │
│  │ - Send to 1-3    │  │ - Auth headers   │  │ - Model      │ │
│  │   models         │  │ - Response parse │  │ - Role       │ │
│  └──────────────────┘  └──────────────────┘  └──────────────┘ │
│                               │                                 │
│                               │                                 │
│  ┌────────────────────────────┼──────────────────────────────┐ │
│  │              Error Handling (src/error.rs)                │ │
│  │  - TuisterError enum                                      │ │
│  │  - Result type alias                                      │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────┼───────────────────────────────────┘
                              │
                              ▼
                    ┌───────────────────┐
                    │   OpenRouter API  │
                    │ (External Service)│
                    └───────────────────┘
```

## Data Flow

1. **User Input** → TUI captures keystrokes
2. **Message Creation** → UI creates ChatMessage
3. **Session Update** → ChatSession adds message to history
4. **API Call** → OpenRouterClient sends to selected models
5. **Response** → Parse and display in UI
6. **Cycle** → Repeat for next message

## Key Design Decisions

### Library-First Architecture
- Core logic in `src/lib.rs` and modules
- Allows building different UIs (web, GUI, etc.) later
- Business logic separate from presentation

### Async/Await Pattern
- Non-blocking API calls with Tokio
- UI remains responsive during LLM queries
- Can send to multiple models concurrently

### Error Handling
- Custom `TuisterError` enum with `thiserror`
- Descriptive error variants
- Proper error propagation with `Result<T>`

### Model Management
- Support 1-3 simultaneous models
- Tab key cycles through configurations
- Each model gets same message history

## Future Extension Points

The library architecture enables:
- **Web UI**: Build with Axum/Actix
- **Desktop GUI**: Use egui or Tauri
- **CLI tools**: Direct library usage
- **Plugins**: Custom model providers
- **Persistence**: Add database layer
