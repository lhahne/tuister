mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tuister::config::Config;
use tuister::{ChatSession, OpenRouterClient};
use ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file if it exists (silently ignore if not found)
    let _ = dotenvy::dotenv();

    // Get API key from environment
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable must be set");

    // Create client
    let client = OpenRouterClient::new(api_key.clone())?;

    // Fetch available models from OpenRouter API
    let available_models = match client.fetch_models().await {
        Ok(models) => {
            if models.is_empty() {
                eprintln!("Warning: No models fetched from OpenRouter. Using fallback models.");
                get_fallback_models()
            } else {
                models
            }
        }
        Err(e) => {
            eprintln!(
                "Warning: Failed to fetch models from OpenRouter: {}. Using fallback models.",
                e
            );
            get_fallback_models()
        }
    };

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create client for chat (reuse API key)
    let chat_client = OpenRouterClient::new(api_key)?;

    // Load saved config (for model selection persistence)
    let config = Config::load();
    let saved_model_ids = if config.selected_model_ids.is_empty() {
        None
    } else {
        Some(config.selected_model_ids)
    };

    // Start with saved models or first 3 by default
    let default_models = if let Some(ref ids) = saved_model_ids {
        available_models
            .iter()
            .filter(|m| ids.contains(&m.id))
            .cloned()
            .collect::<Vec<_>>()
    } else {
        available_models.iter().take(3).cloned().collect()
    };

    // Ensure at least one model is selected
    let default_models = if default_models.is_empty() {
        available_models.iter().take(1).cloned().collect()
    } else {
        default_models
    };

    let session = ChatSession::new(chat_client, default_models);
    let mut app = App::new(session, available_models, saved_model_ids);

    // Run the app
    let res = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {:?}", err);
    }

    Ok(())
}

fn get_fallback_models() -> Vec<tuister::Model> {
    use tuister::Model;
    let mut models = vec![
        Model::new("openai/gpt-3.5-turbo", "GPT-3.5 Turbo"),
        Model::new("openai/gpt-4", "GPT-4"),
        Model::new("openai/gpt-4-turbo", "GPT-4 Turbo"),
        Model::new("anthropic/claude-3-haiku", "Claude 3 Haiku"),
        Model::new("anthropic/claude-3-sonnet", "Claude 3 Sonnet"),
        Model::new("anthropic/claude-3-opus", "Claude 3 Opus"),
        Model::new("google/gemini-flash-1.5", "Gemini Flash 1.5"),
        Model::new("google/gemini-pro-1.5", "Gemini Pro 1.5"),
        Model::new("meta-llama/llama-3-70b-instruct", "Llama 3 70B"),
        Model::new("mistralai/mistral-7b-instruct", "Mistral 7B"),
    ];

    // Sort models alphabetically by name
    models.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    models
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    // Track whether we're waiting for a second Ctrl+C to quit
    let mut pending_quit = false;

    loop {
        terminal.draw(|f| ui::ui(f, app))?;

        // Poll for streaming updates (non-blocking)
        app.poll_streaming();

        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle Ctrl+C: first cancels streaming, second quits
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    if app.is_streaming() {
                        // First Ctrl+C while streaming: cancel the streaming
                        app.cancel_streaming();
                        pending_quit = false;
                        continue;
                    } else if pending_quit {
                        // Second Ctrl+C: quit the app
                        return Ok(());
                    } else {
                        // First Ctrl+C with no streaming: set pending quit
                        pending_quit = true;
                        continue;
                    }
                }

                // Any other key resets the pending quit state
                pending_quit = false;

                // Handle Ctrl+n to clear chat
                if key.code == KeyCode::Char('n') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.clear_chat();
                    continue;
                }

                // Handle Ctrl+h for about screen
                if key.code == KeyCode::Char('h') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    app.toggle_about();
                    continue;
                }

                // Handle Escape to toggle model selection from any mode
                if key.code == KeyCode::Esc {
                    if app.mode == ui::AppMode::About {
                        app.toggle_about();
                    } else {
                        app.toggle_model_selection();
                    }
                    continue;
                }

                // Handle other keys based on current mode
                match key.code {
                    KeyCode::Char(' ') => {
                        app.toggle_current_model();
                    }
                    KeyCode::Char(c) => {
                        app.input_char(c);
                    }
                    KeyCode::Backspace => {
                        app.delete_char();
                    }
                    KeyCode::Enter => {
                        app.handle_enter();
                    }
                    KeyCode::Up => {
                        app.handle_up();
                    }
                    KeyCode::Down => {
                        app.handle_down();
                    }
                    KeyCode::PageUp => {
                        app.handle_page_up();
                    }
                    KeyCode::PageDown => {
                        app.handle_page_down();
                    }
                    KeyCode::Tab => {
                        app.cycle_model_selection();
                    }
                    KeyCode::Left => {
                        app.cycle_panel_focus_left();
                    }
                    KeyCode::Right => {
                        app.cycle_panel_focus_right();
                    }
                    _ => {}
                }
            }
        }
    }
}
