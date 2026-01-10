mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tuister::{ChatSession, Model, OpenRouterClient};
use ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Get API key from environment
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .expect("OPENROUTER_API_KEY environment variable must be set");
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    
    // Create client
    let client = OpenRouterClient::new(api_key)?;
    
    // Available models - users can choose from these
    let available_models = vec![
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
    
    // Start with first 3 models selected by default
    let default_models = available_models.iter().take(3).cloned().collect();
    
    let session = ChatSession::new(client, default_models);
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

async fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui::ui(f, app))?;
        
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        return Ok(());
                    }
                    KeyCode::Char('q') => {
                        return Ok(());
                    }
                    KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.toggle_model_selection();
                    }
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

