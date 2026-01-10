use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};
use tuister::{ChatSession, Role};

pub struct App {
    session: ChatSession,
    input: String,
    messages: Vec<DisplayMessage>,
    scroll: usize,
    active_models: Vec<bool>,
    is_loading: bool,
}

#[derive(Clone)]
struct DisplayMessage {
    role: Role,
    content: String,
    model_name: Option<String>,
}

impl App {
    pub fn new(session: ChatSession) -> Self {
        let num_models = session.models().len();
        let active_models = vec![true; num_models.min(3)];
        
        Self {
            session,
            input: String::new(),
            messages: Vec::new(),
            scroll: 0,
            active_models,
            is_loading: false,
        }
    }
    
    pub fn input_char(&mut self, c: char) {
        self.input.push(c);
    }
    
    pub fn delete_char(&mut self) {
        self.input.pop();
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
        let num_models = self.session.models().len();
        
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
        let active_models: Vec<_> = self
            .session
            .models()
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < self.active_models.len() && self.active_models[*i])
            .map(|(_, m)| m.clone())
            .collect();
        
        // Send to each active model
        for model in active_models {
            match self.session.send_to_model(&model).await {
                Ok(response) => {
                    self.messages.push(DisplayMessage {
                        role: Role::Assistant,
                        content: response,
                        model_name: Some(model.name.clone()),
                    });
                }
                Err(e) => {
                    self.messages.push(DisplayMessage {
                        role: Role::Assistant,
                        content: format!("Error: {}", e),
                        model_name: Some(model.name.clone()),
                    });
                }
            }
        }
        
        self.is_loading = false;
        
        Ok(())
    }
}

pub fn ui(f: &mut Frame, app: &App) {
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

fn render_header(f: &mut Frame, area: Rect, app: &App) {
    let models = app.session.models();
    let mut model_spans = Vec::new();
    
    for (i, model) in models.iter().enumerate() {
        if i < app.active_models.len() && app.active_models[i] {
            model_spans.push(Span::styled(
                format!("[{}] ", model.name),
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            ));
        } else {
            model_spans.push(Span::styled(
                format!("[{}] ", model.name),
                Style::default().fg(Color::DarkGray),
            ));
        }
    }
    
    model_spans.push(Span::raw("(Tab to cycle)"));
    
    let header = Paragraph::new(Line::from(model_spans))
        .block(Block::default().borders(Borders::ALL).title("Models"));
    
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
