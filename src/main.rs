mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
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
            eprintln!("Warning: Failed to fetch models from OpenRouter: {}. Using fallback models.", e);
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
    
    // Start with first 3 models selected by default
    let default_models = available_models.iter().take(3).cloned().collect();
    
    let session = ChatSession::new(chat_client, default_models);
    let mut app = App::new(session, available_models);
    
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
    vec![
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
    ]
}

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, app))?;
        
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Handle Ctrl+C to quit from any mode
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }
                
                // Handle Escape to toggle model selection from any mode
                if key.code == KeyCode::Esc {
                    app.toggle_model_selection();
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
                        app.handle_enter().await?;
                    }
                    KeyCode::Up => {
                        app.handle_up();
                    }
                    KeyCode::Down => {
                        app.handle_down();
                    }
                    KeyCode::Tab => {
                        app.cycle_model_selection();
                    }
                    _ => {}
                }
            }
        }
    }
}

