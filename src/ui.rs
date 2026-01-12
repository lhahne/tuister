use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use tuister::{ChatSession, Model, OpenRouterClient, Role};
use tokio::sync::mpsc;
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
enum AppMode {
    Chat,
    ModelSelection,
}

pub struct App {
    session: ChatSession,
    input: String,
    messages: Vec<DisplayMessage>,
    model_responses: HashMap<String, Vec<String>>, // model_name -> responses
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
            model_responses: HashMap::new(),
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
            .unwrap_or_else(|_| "test_key".to_string()); // Use test key if env var not set
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
    
    pub fn is_in_chat_mode(&self) -> bool {
        self.mode == AppMode::Chat
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
        
        // Clear input immediately
        self.input.clear();
        
        // Show loading indicator
        self.is_loading = true;
        
        // Add user message to display
        self.messages.push(DisplayMessage {
            role: Role::User,
            content: user_message.clone(),
            model_name: None,
        });
        
        // Add user message to session
        self.session.add_user_message(user_message);
        
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
                        full_response = format!("Error: {}", e);
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
                    content: full_response.clone(),
                    model_name: Some(model_name.clone()),
                });
                
                // Also add to model_responses for side-by-side display
                self.model_responses
                    .entry(model_name)
                    .or_default()
                    .push(full_response);
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
            Constraint::Length(3),  // Header
            Constraint::Min(1),     // Model response panes
            Constraint::Length(3),  // Input
        ])
        .split(f.area());
    
    // Header
    render_header(f, chunks[0], app);
    
    // Model response panes (side by side)
    render_model_panes(f, chunks[1], app);
    
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
    let help_text = "Ctrl+M: Back to chat | Space/Enter: Toggle model | ↑↓: Navigate | Ctrl+C: Quit";
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

fn render_model_panes(f: &mut Frame, area: Rect, app: &App) {
    // Get active models
    let active_models: Vec<_> = app
        .available_models
        .iter()
        .enumerate()
        .filter(|(i, _)| *i < app.active_models.len() && app.active_models[*i])
        .map(|(_, m)| m)
        .collect();
    
    if active_models.is_empty() {
        let empty = Paragraph::new("No models selected")
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().fg(Color::DarkGray));
        f.render_widget(empty, area);
        return;
    }
    
    // Create horizontal layout for side-by-side panes
    let num_models = active_models.len();
    let constraints: Vec<Constraint> = (0..num_models)
        .map(|_| Constraint::Percentage(100 / num_models as u16))
        .collect();
    
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);
    
    // Render each model's pane
    for (idx, model) in active_models.iter().enumerate() {
        render_model_pane(f, panes[idx], app, &model.name);
    }
}

fn render_model_pane(f: &mut Frame, area: Rect, app: &App, model_name: &str) {
    // Get all messages for this model
    let model_messages: Vec<_> = app
        .messages
        .iter()
        .filter_map(|msg| {
            match &msg.model_name {
                Some(name) if name == model_name => Some(msg.content.clone()),
                None if msg.role == Role::User => Some(format!("You: {}", msg.content)),
                _ => None,
            }
        })
        .collect();
    
    let content = if app.is_loading && model_messages.is_empty() {
        "⏳ Waiting for response...".to_string()
    } else if model_messages.is_empty() {
        "No messages yet".to_string()
    } else {
        model_messages.join("\n\n")
    };
    
    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(model_name)
                .style(Style::default()),
        )
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(Color::White));
    
    f.render_widget(paragraph, area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    let (input_text, style) = if app.is_loading {
        ("⏳ Waiting for responses...".to_string(), Style::default().fg(Color::Yellow))
    } else {
        (app.input.clone(), Style::default())
    };
    
    let input = Paragraph::new(input_text)
        .style(style)
        .block(Block::default().borders(Borders::ALL).title("Input (Enter to send, Ctrl+C to quit)"));
    
    f.render_widget(input, area);
    
    // Set cursor position in input box when in chat mode and not loading
    if app.is_in_chat_mode() && !app.is_loading {
        // Position cursor at the end of input text
        // Account for border (1) and current input length
        let cursor_x = area.x + 1 + app.input.len() as u16;
        let cursor_y = area.y + 1;
        
        // Make sure cursor is within bounds
        if cursor_x < area.x + area.width - 1 {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_app() -> App {
        let api_key = "test_key".to_string();
        let client = OpenRouterClient::new(api_key).unwrap();
        let models = vec![
            Model::new("model1", "Model 1"),
            Model::new("model2", "Model 2"),
            Model::new("model3", "Model 3"),
        ];
        let available_models = models.clone();
        let session = ChatSession::new(client, models);
        App::new(session, available_models)
    }
    
    #[test]
    fn test_app_creation() {
        let app = create_test_app();
        assert_eq!(app.input, "");
        assert_eq!(app.messages.len(), 0);
        assert!(!app.is_loading);
        assert_eq!(app.mode, AppMode::Chat);
        assert_eq!(app.active_models, vec![true, true, true]);
    }
    
    #[test]
    fn test_input_char_in_chat_mode() {
        let mut app = create_test_app();
        app.input_char('h');
        app.input_char('i');
        assert_eq!(app.input, "hi");
    }
    
    #[test]
    fn test_input_char_in_model_selection_mode() {
        let mut app = create_test_app();
        app.toggle_model_selection();
        assert_eq!(app.mode, AppMode::ModelSelection);
        
        app.input_char('h');
        assert_eq!(app.input, ""); // Should not add character in model selection mode
    }
    
    #[test]
    fn test_delete_char() {
        let mut app = create_test_app();
        app.input_char('h');
        app.input_char('e');
        app.input_char('l');
        app.input_char('l');
        app.input_char('o');
        assert_eq!(app.input, "hello");
        
        app.delete_char();
        assert_eq!(app.input, "hell");
        
        app.delete_char();
        app.delete_char();
        assert_eq!(app.input, "he");
    }
    
    #[test]
    fn test_is_in_chat_mode() {
        let mut app = create_test_app();
        assert!(app.is_in_chat_mode());
        
        app.toggle_model_selection();
        assert!(!app.is_in_chat_mode());
        
        app.toggle_model_selection();
        assert!(app.is_in_chat_mode());
    }
    
    #[test]
    fn test_toggle_model_selection() {
        let mut app = create_test_app();
        assert_eq!(app.mode, AppMode::Chat);
        assert_eq!(app.selected_model_index, 0);
        
        app.toggle_model_selection();
        assert_eq!(app.mode, AppMode::ModelSelection);
        assert_eq!(app.selected_model_index, 0);
        
        app.toggle_model_selection();
        assert_eq!(app.mode, AppMode::Chat);
    }
    
    #[test]
    fn test_toggle_current_model_in_selection_mode() {
        let mut app = create_test_app();
        app.toggle_model_selection();
        
        // Initially all 3 models are active
        assert_eq!(app.active_models, vec![true, true, true]);
        
        // Toggle first model
        app.toggle_current_model();
        assert_eq!(app.active_models, vec![false, true, true]);
        
        // Toggle it back
        app.toggle_current_model();
        assert_eq!(app.active_models, vec![true, true, true]);
    }
    
    #[test]
    fn test_toggle_current_model_in_chat_mode() {
        let mut app = create_test_app();
        
        // In chat mode, toggle_current_model adds a space
        app.toggle_current_model();
        assert_eq!(app.input, " ");
    }
    
    #[test]
    fn test_handle_up_down_in_chat_mode() {
        let mut app = create_test_app();
        
        // In chat mode, up/down should scroll
        assert_eq!(app.scroll, 0);
        app.handle_down();
        assert_eq!(app.scroll, 1);
        app.handle_up();
        assert_eq!(app.scroll, 0);
        app.handle_up(); // Should not go below 0
        assert_eq!(app.scroll, 0);
    }
    
    #[test]
    fn test_handle_up_down_in_model_selection_mode() {
        let mut app = create_test_app();
        app.toggle_model_selection();
        
        assert_eq!(app.selected_model_index, 0);
        app.handle_down();
        assert_eq!(app.selected_model_index, 1);
        app.handle_down();
        assert_eq!(app.selected_model_index, 2);
        app.handle_down(); // Should not go beyond last model
        assert_eq!(app.selected_model_index, 2);
        
        app.handle_up();
        assert_eq!(app.selected_model_index, 1);
        app.handle_up();
        assert_eq!(app.selected_model_index, 0);
        app.handle_up(); // Should not go below 0
        assert_eq!(app.selected_model_index, 0);
    }
    
    #[test]
    fn test_cycle_model_selection() {
        let mut app = create_test_app();
        
        // Start with 3 models active
        assert_eq!(app.active_models.iter().filter(|&&x| x).count(), 3);
        
        app.cycle_model_selection();
        // Should cycle to 1 model
        assert_eq!(app.active_models.iter().filter(|&&x| x).count(), 1);
        assert_eq!(app.active_models, vec![true, false, false]);
        
        app.cycle_model_selection();
        // Should cycle to 2 models
        assert_eq!(app.active_models.iter().filter(|&&x| x).count(), 2);
        assert_eq!(app.active_models, vec![true, true, false]);
        
        app.cycle_model_selection();
        // Should cycle back to 3 models
        assert_eq!(app.active_models.iter().filter(|&&x| x).count(), 3);
        assert_eq!(app.active_models, vec![true, true, true]);
    }
    
    #[test]
    fn test_scroll_up_down() {
        let mut app = create_test_app();
        
        assert_eq!(app.scroll, 0);
        
        app.scroll_down();
        assert_eq!(app.scroll, 1);
        
        app.scroll_down();
        assert_eq!(app.scroll, 2);
        
        app.scroll_up();
        assert_eq!(app.scroll, 1);
        
        app.scroll_up();
        assert_eq!(app.scroll, 0);
        
        // Should not go below 0
        app.scroll_up();
        assert_eq!(app.scroll, 0);
    }
}
