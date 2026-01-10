use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use tuister::{ChatSession, Model, OpenRouterClient, Role};
use tokio::sync::mpsc;

#[derive(Debug, PartialEq)]
enum AppMode {
    Chat,
    ModelSelection,
}

pub struct App {
    session: ChatSession,
    input: String,
    messages: Vec<DisplayMessage>,
    scroll: usize,
    active_models: Vec<bool>,
    is_loading: bool,
    mode: AppMode,
    available_models: Vec<Model>,
    selected_model_index: usize,
}

#[derive(Clone)]
struct DisplayMessage {
    role: Role,
    content: String,
    model_name: Option<String>,
}

impl App {
    pub fn new(session: ChatSession, available_models: Vec<Model>) -> Self {
        let num_models = session.models().len();
        let active_models = vec![true; num_models.min(3)];
        
        Self {
            session,
            input: String::new(),
            messages: Vec::new(),
            scroll: 0,
            active_models,
            is_loading: false,
            mode: AppMode::Chat,
            available_models,
            selected_model_index: 0,
        }
    }
    
    pub fn toggle_model_selection(&mut self) {
        match self.mode {
            AppMode::Chat => {
                self.mode = AppMode::ModelSelection;
                self.selected_model_index = 0;
            }
            AppMode::ModelSelection => {
                self.mode = AppMode::Chat;
                self.update_session_models();
            }
        }
    }
    
    pub fn toggle_current_model(&mut self) {
        if self.mode == AppMode::ModelSelection {
            if self.selected_model_index < self.active_models.len() {
                self.active_models[self.selected_model_index] = 
                    !self.active_models[self.selected_model_index];
            }
        } else {
            // In chat mode, space is just a character for input
            self.input.push(' ');
        }
    }
    
    pub fn handle_up(&mut self) {
        match self.mode {
            AppMode::Chat => self.scroll_up(),
            AppMode::ModelSelection => {
                if self.selected_model_index > 0 {
                    self.selected_model_index -= 1;
                }
            }
        }
    }
    
    pub fn handle_down(&mut self) {
        match self.mode {
            AppMode::Chat => self.scroll_down(),
            AppMode::ModelSelection => {
                if self.selected_model_index < self.available_models.len() - 1 {
                    self.selected_model_index += 1;
                }
            }
        }
    }
    
    pub async fn handle_enter(&mut self) -> anyhow::Result<()> {
        match self.mode {
            AppMode::Chat => self.submit_message().await,
            AppMode::ModelSelection => {
                self.toggle_current_model();
                Ok(())
            }
        }
    }
    
    fn update_session_models(&mut self) {
        // Get the selected models
        let selected_models: Vec<Model> = self
            .available_models
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < self.active_models.len() && self.active_models[*i])
            .map(|(_, m)| m.clone())
            .collect();
        
        if selected_models.is_empty() {
            // Ensure at least one model is selected
            if !self.available_models.is_empty() {
                self.active_models[0] = true;
                return;
            }
        }
        
        // Create a new session with selected models
        // We need to preserve the API key from the old session
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .expect("OPENROUTER_API_KEY environment variable must be set");
        let client = OpenRouterClient::new(api_key).expect("Failed to create client");
        let messages = self.session.messages().to_vec();
        
        self.session = ChatSession::new(client, selected_models);
        
        // Restore message history
        for msg in messages {
            match msg.role {
                Role::System => self.session.add_system_message(msg.content),
                Role::User => self.session.add_user_message(msg.content),
                Role::Assistant => {} // Skip assistant messages as they're responses
            }
        }
    }
    
    pub fn input_char(&mut self, c: char) {
        if self.mode == AppMode::Chat {
            self.input.push(c);
        }
    }
    
    pub fn delete_char(&mut self) {
        if self.mode == AppMode::Chat {
            self.input.pop();
        }
    }
    
    pub fn scroll_up(&mut self) {
        if self.scroll > 0 {
            self.scroll -= 1;
        }
    }
    
    pub fn scroll_down(&mut self) {
        self.scroll += 1;
    }
    
    pub fn cycle_model_selection(&mut self) {
        if self.mode == AppMode::Chat {
            let num_models = self.available_models.len();
            
            // Find the current number of active models
            let active_count = self.active_models.iter().filter(|&&x| x).count();
            
            if active_count == 1 {
                // Activate 2 models
                self.active_models = vec![false; num_models];
                for i in 0..2.min(num_models) {
                    self.active_models[i] = true;
                }
            } else if active_count == 2 {
                // Activate 3 models
                self.active_models = vec![false; num_models];
                for i in 0..3.min(num_models) {
                    self.active_models[i] = true;
                }
            } else {
                // Activate 1 model
                self.active_models = vec![false; num_models];
                if num_models > 0 {
                    self.active_models[0] = true;
                }
            }
            
            self.update_session_models();
        }
    }
    
    pub async fn submit_message(&mut self) -> anyhow::Result<()> {
        if self.input.trim().is_empty() {
            return Ok(());
        }
        
        let user_message = self.input.clone();
        self.input.clear();
        
        // Add user message to display
        self.messages.push(DisplayMessage {
            role: Role::User,
            content: user_message.clone(),
            model_name: None,
        });
        
        // Add user message to session
        self.session.add_user_message(user_message);
        
        self.is_loading = true;
        
        // Get active models
        let active_models = self.session.models().to_vec();
        
        // Send to each active model with streaming
        for model in active_models {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let model_name = model.name.clone();
            
            // Start streaming in background
            let send_task = self.session.send_to_model_streaming(&model, tx);
            
            // Collect streamed chunks while they arrive
            let mut full_response = String::new();
            
            tokio::select! {
                result = send_task => {
                    // Streaming completed
                    if let Err(e) = result {
                        // If streaming failed, show error
                        self.messages.push(DisplayMessage {
                            role: Role::Assistant,
                            content: format!("Streaming error: {}", e),
                            model_name: Some(model_name.clone()),
                        });
                        continue;
                    }
                }
                _ = async {
                    while let Some(chunk) = rx.recv().await {
                        full_response.push_str(&chunk);
                    }
                } => {}
            }
            
            // Add complete message
            if !full_response.is_empty() {
                self.messages.push(DisplayMessage {
                    role: Role::Assistant,
                    content: full_response,
                    model_name: Some(model_name),
                });
            }
        }
        
        self.is_loading = false;
        
        Ok(())
    }
}

pub fn ui(f: &mut Frame, app: &App) {
    match app.mode {
        AppMode::Chat => render_chat_mode(f, app),
        AppMode::ModelSelection => render_model_selection_mode(f, app),
    }
}

fn render_chat_mode(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());
    
    // Header
    render_header(f, chunks[0], app);
    
    // Messages
    render_messages(f, chunks[1], app);
    
    // Input
    render_input(f, chunks[2], app);
}

fn render_model_selection_mode(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(f.area());
    
    // Title
    let title = Paragraph::new("Model Selection")
        .style(Style::default().add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);
    
    // Model list
    let items: Vec<ListItem> = app
        .available_models
        .iter()
        .enumerate()
        .map(|(i, model)| {
            let is_selected = i < app.active_models.len() && app.active_models[i];
            let is_highlighted = i == app.selected_model_index;
            
            let checkbox = if is_selected { "[✓]" } else { "[ ]" };
            let content = format!("{} {}", checkbox, model.name);
            
            let style = if is_highlighted {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            
            ListItem::new(content).style(style)
        })
        .collect();
    
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Available Models (↑↓ to navigate, Space/Enter to toggle)"),
    );
    
    f.render_widget(list, chunks[1]);
    
    // Help text
    let help_text = "Ctrl+M: Back to chat | Space/Enter: Toggle model | ↑↓: Navigate | q/Ctrl+C: Quit";
    let help = Paragraph::new(help_text)
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Help"));
    
    f.render_widget(help, chunks[2]);
}

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let active_models: Vec<_> = app
        .available_models
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < app.active_models.len() && app.active_models[*i])
        .map(|(_, m)| m)
        .collect();
    
    let mut model_spans = Vec::new();
    
    for model in &active_models {
        model_spans.push(Span::styled(
            format!("[{}] ", model.name),
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        ));
    }
    
    model_spans.push(Span::raw("(Tab: cycle | Ctrl+M: select models)"));
    
    let header = Paragraph::new(Line::from(model_spans))
        .block(Block::default().borders(Borders::ALL).title("Active Models"));
    
    f.render_widget(header, area);
}

fn render_messages(f: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = app
        .messages
        .iter()
        .map(|msg| {
            let (style, prefix) = match msg.role {
                Role::User => (
                    Style::default().fg(Color::Cyan),
                    "You: ".to_string(),
                ),
                Role::Assistant => {
                    let model_prefix = if let Some(ref name) = msg.model_name {
                        format!("{}: ", name)
                    } else {
                        "Assistant: ".to_string()
                    };
                    (Style::default().fg(Color::Green), model_prefix)
                }
                Role::System => (
                    Style::default().fg(Color::Yellow),
                    "System: ".to_string(),
                ),
            };
            
            let content = format!("{}{}", prefix, msg.content);
            ListItem::new(content).style(style)
        })
        .collect();
    
    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Chat (↑↓ to scroll)"),
    );
    
    f.render_widget(list, area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let input_text = if app.is_loading {
        "Loading...".to_string()
    } else {
        app.input.clone()
    };
    
    let input = Paragraph::new(input_text)
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title("Input (Enter to send, Ctrl+C/q to quit)"));
    
    f.render_widget(input, area);
}
