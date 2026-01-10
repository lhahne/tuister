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
    
    // Default models - users can configure these
    let models = vec![
        Model::new("openai/gpt-3.5-turbo", "GPT-3.5 Turbo"),
        Model::new("anthropic/claude-3-haiku", "Claude 3 Haiku"),
        Model::new("google/gemini-flash-1.5", "Gemini Flash 1.5"),
    ];
    
    let session = ChatSession::new(client, models);
    let mut app = App::new(session);
    
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
                    KeyCode::Char(c) => {
                        app.input_char(c);
                    }
                    KeyCode::Backspace => {
                        app.delete_char();
                    }
                    KeyCode::Enter => {
                        app.submit_message().await?;
                    }
                    KeyCode::Up => {
                        app.scroll_up();
                    }
                    KeyCode::Down => {
                        app.scroll_down();
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

