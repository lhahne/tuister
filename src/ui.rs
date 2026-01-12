use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use std::collections::HashMap;
use tokio::sync::mpsc;
use tuister::{ChatSession, Model, OpenRouterClient, Role};

#[derive(Debug, PartialEq)]
enum AppMode {
    Chat,
    ModelSelection,
}

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

pub struct App {
    session: ChatSession,
    input: String,
    messages: Vec<DisplayMessage>,
    model_responses: HashMap<String, Vec<String>>, // model_name -> responses
    active_models: Vec<bool>,
    is_loading: bool,
    mode: AppMode,
    available_models: Vec<Model>,
    selected_model_index: usize,
    model_list_offset: usize, // scroll offset for model list
    // Streaming state
    streaming_receivers: Vec<(String, mpsc::UnboundedReceiver<String>)>,
    streaming_buffers: HashMap<String, String>, // accumulates chunks per model
    // Message queue for messages submitted while streaming
    message_queue: Vec<String>,
    // Spinner animation
    spinner_frame: usize,
    last_spinner_update: std::time::Instant,
    // Panel scroll state
    panel_scrolls: HashMap<String, u16>, // model_name -> scroll offset
    focused_panel: usize,                // index of the currently focused panel for scrolling
    auto_scroll: HashMap<String, bool>,  // model_name -> whether auto-scroll is enabled
}

#[derive(Clone)]
struct DisplayMessage {
    role: Role,
    content: String,
    model_name: Option<String>,
}

impl App {
    pub fn new(session: ChatSession, available_models: Vec<Model>) -> Self {
        let num_models = available_models.len();
        let active_models: Vec<bool> = (0..num_models).map(|i| i < 3).collect();

        Self {
            session,
            input: String::new(),
            messages: Vec::new(),
            model_responses: HashMap::new(),
            active_models,
            is_loading: false,
            mode: AppMode::Chat,
            available_models,
            selected_model_index: 0,
            model_list_offset: 0,
            streaming_receivers: Vec::new(),
            streaming_buffers: HashMap::new(),
            message_queue: Vec::new(),
            spinner_frame: 0,
            last_spinner_update: std::time::Instant::now(),
            panel_scrolls: HashMap::new(),
            focused_panel: 0,
            auto_scroll: HashMap::new(),
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
                    // Adjust offset if selected item is above visible area
                    if self.selected_model_index < self.model_list_offset {
                        self.model_list_offset = self.selected_model_index;
                    }
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
                    // Adjust offset if selected item is below visible area
                    // Assume visible height of ~15 items (will be adjusted by ListState if needed)
                    let visible_height = 15;
                    if self.selected_model_index >= self.model_list_offset + visible_height {
                        self.model_list_offset = self.selected_model_index - visible_height + 1;
                    }
                }
            }
        }
    }

    pub fn handle_page_up(&mut self) {
        if self.mode == AppMode::ModelSelection {
            let page_size = 10;
            self.selected_model_index = self.selected_model_index.saturating_sub(page_size);
            self.model_list_offset = self.model_list_offset.saturating_sub(page_size);
        }
    }

    pub fn handle_page_down(&mut self) {
        if self.mode == AppMode::ModelSelection {
            let page_size = 10;
            let max_index = self.available_models.len().saturating_sub(1);
            self.selected_model_index = (self.selected_model_index + page_size).min(max_index);
        }
    }

    pub fn handle_enter(&mut self) {
        match self.mode {
            AppMode::Chat => self.submit_message(),
            AppMode::ModelSelection => {
                self.toggle_current_model();
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
        let api_key =
            std::env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| "test_key".to_string()); // Use test key if env var not set
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
        if let Some(model_name) = self.get_focused_model_name() {
            let scroll = self.panel_scrolls.entry(model_name.clone()).or_insert(0);
            if *scroll > 0 {
                *scroll -= 1;
            }
            // Disable auto-scroll when manually scrolling
            self.auto_scroll.insert(model_name, false);
        }
    }

    pub fn scroll_down(&mut self) {
        if let Some(model_name) = self.get_focused_model_name() {
            let scroll = self.panel_scrolls.entry(model_name.clone()).or_insert(0);
            *scroll += 1;
            // Disable auto-scroll when manually scrolling down
            // (user might be scrolling to catch up, re-enable if at bottom)
            self.auto_scroll.insert(model_name, false);
        }
    }

    /// Cycle focus to the next panel (left/right navigation)
    pub fn cycle_panel_focus_right(&mut self) {
        if self.mode == AppMode::Chat {
            let num_active = self.get_active_model_names().len();
            if num_active > 0 {
                self.focused_panel = (self.focused_panel + 1) % num_active;
            }
        }
    }

    pub fn cycle_panel_focus_left(&mut self) {
        if self.mode == AppMode::Chat {
            let num_active = self.get_active_model_names().len();
            if num_active > 0 {
                if self.focused_panel == 0 {
                    self.focused_panel = num_active - 1;
                } else {
                    self.focused_panel -= 1;
                }
            }
        }
    }

    /// Get the model name of the currently focused panel
    fn get_focused_model_name(&self) -> Option<String> {
        let active_names = self.get_active_model_names();
        active_names.get(self.focused_panel).cloned()
    }

    /// Get list of active model names in display order
    fn get_active_model_names(&self) -> Vec<String> {
        self.available_models
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < self.active_models.len() && self.active_models[*i])
            .map(|(_, m)| m.name.clone())
            .collect()
    }

    /// Enable auto-scroll for a model (called when streaming starts)
    fn enable_auto_scroll(&mut self, model_name: &str) {
        self.auto_scroll.insert(model_name.to_string(), true);
    }

    /// Get scroll offset for a model panel
    pub fn get_panel_scroll(&self, model_name: &str) -> u16 {
        *self.panel_scrolls.get(model_name).unwrap_or(&0)
    }

    /// Check if a panel is the focused one
    pub fn is_panel_focused(&self, panel_index: usize) -> bool {
        self.focused_panel == panel_index
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

    pub fn submit_message(&mut self) {
        if self.input.trim().is_empty() {
            return;
        }

        let user_message = self.input.clone();
        self.input.clear();

        // If already streaming, queue the message for later
        if self.is_loading {
            self.message_queue.push(user_message);
            return;
        }

        self.start_streaming(user_message);
    }

    fn start_streaming(&mut self, user_message: String) {
        self.is_loading = true;

        // Add user message to display
        self.messages.push(DisplayMessage {
            role: Role::User,
            content: user_message.clone(),
            model_name: None,
        });

        // Add user message to session
        self.session.add_user_message(user_message);

        // Get active models and spawn streaming tasks
        let active_models = self.session.models().to_vec();

        for model in active_models {
            let model_name = model.name.clone();
            let rx = self.session.spawn_streaming(&model);
            self.streaming_receivers.push((model_name.clone(), rx));
            self.streaming_buffers
                .insert(model_name.clone(), String::new());
            self.enable_auto_scroll(&model_name);
        }
    }

    /// Poll streaming receivers for new chunks (non-blocking)
    pub fn poll_streaming(&mut self) {
        // Update spinner animation (every 80ms)
        if self.last_spinner_update.elapsed() >= std::time::Duration::from_millis(80) {
            self.spinner_frame = (self.spinner_frame + 1) % SPINNER_FRAMES.len();
            self.last_spinner_update = std::time::Instant::now();
        }

        if self.streaming_receivers.is_empty() {
            return;
        }

        let mut completed = Vec::new();

        for (i, (model_name, rx)) in self.streaming_receivers.iter_mut().enumerate() {
            loop {
                match rx.try_recv() {
                    Ok(chunk) => {
                        // Append chunk to buffer
                        if let Some(buffer) = self.streaming_buffers.get_mut(model_name) {
                            buffer.push_str(&chunk);
                        }
                    }
                    Err(mpsc::error::TryRecvError::Empty) => {
                        // No more chunks available right now
                        break;
                    }
                    Err(mpsc::error::TryRecvError::Disconnected) => {
                        // Streaming finished for this model
                        completed.push(i);
                        break;
                    }
                }
            }
        }

        // Remove completed receivers (in reverse order to preserve indices)
        for i in completed.into_iter().rev() {
            let (model_name, _) = self.streaming_receivers.remove(i);

            // Move completed buffer to model_responses
            if let Some(response) = self.streaming_buffers.remove(&model_name) {
                if !response.is_empty() {
                    self.messages.push(DisplayMessage {
                        role: Role::Assistant,
                        content: response.clone(),
                        model_name: Some(model_name.clone()),
                    });
                    self.model_responses
                        .entry(model_name)
                        .or_default()
                        .push(response);
                }
            }
        }

        // Check if all streaming is complete
        if self.streaming_receivers.is_empty() {
            self.is_loading = false;

            // Process next queued message if any
            if let Some(next_message) = self.message_queue.pop() {
                self.start_streaming(next_message);
            }
        }
    }

    /// Check if a specific model is currently streaming
    fn is_model_streaming(&self, model_name: &str) -> bool {
        self.streaming_receivers
            .iter()
            .any(|(name, _)| name == model_name)
    }

    /// Get the current spinner character
    fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame]
    }

    /// Cancel any ongoing streaming and clear buffers
    /// Returns true if there was an active streaming operation to cancel
    /// Preserves any text that was already received/displayed
    pub fn cancel_streaming(&mut self) -> bool {
        let was_streaming = self.is_loading || !self.streaming_receivers.is_empty();

        // Clear all streaming receivers
        self.streaming_receivers.clear();

        // Preserve any partial responses that were already displayed
        // Move streaming buffers to messages so text remains visible
        for (model_name, mut buffer) in self.streaming_buffers.drain() {
            if !buffer.is_empty() {
                buffer.push_str("\n\n[response terminated]");
                self.messages.push(DisplayMessage {
                    role: Role::Assistant,
                    content: buffer.clone(),
                    model_name: Some(model_name.clone()),
                });
                self.model_responses
                    .entry(model_name)
                    .or_default()
                    .push(buffer);
            }
        }

        // Clear message queue
        self.message_queue.clear();

        // Reset loading state
        self.is_loading = false;

        was_streaming
    }

    /// Check if there is any ongoing streaming activity
    pub fn is_streaming(&self) -> bool {
        self.is_loading || !self.streaming_receivers.is_empty()
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
            Constraint::Length(3), // Header
            Constraint::Min(1),    // Model response panes
            Constraint::Length(3), // Input
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

    // Title with count
    let title = Paragraph::new(format!(
        "Model Selection ({}/{})",
        app.selected_model_index + 1,
        app.available_models.len()
    ))
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

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Available Models (↑↓/PgUp/PgDn to navigate, Space/Enter to toggle)"),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    // Use ListState for scrolling
    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_model_index));
    *list_state.offset_mut() = app.model_list_offset;

    f.render_stateful_widget(list, chunks[1], &mut list_state);

    // Help text
    let help_text =
        "Esc: Back to chat | Space/Enter: Toggle | ↑↓/PgUp/PgDn: Navigate | Ctrl+C: Quit";
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
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ));
    }

    model_spans.push(Span::raw(
        "(←→: focus panel | ↑↓: scroll | Tab: cycle | Esc: models)",
    ));

    let header = Paragraph::new(Line::from(model_spans)).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Active Models"),
    );

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
        let is_focused = app.is_panel_focused(idx);
        render_model_pane(f, panes[idx], app, &model.name, is_focused);
    }
}

fn render_model_pane(f: &mut Frame, area: Rect, app: &App, model_name: &str, is_focused: bool) {
    let is_streaming = app.is_model_streaming(model_name);

    // Get all completed messages for this model
    let model_messages: Vec<_> = app
        .messages
        .iter()
        .filter_map(|msg| match &msg.model_name {
            Some(name) if name == model_name => Some(msg.content.clone()),
            None if msg.role == Role::User => Some(format!("You: {}", msg.content)),
            _ => None,
        })
        .collect();

    // Build content: completed messages + streaming buffer + spinner
    let mut content = if model_messages.is_empty() {
        String::new()
    } else {
        model_messages.join("\n\n")
    };

    // Add streaming buffer content if this model is streaming
    if is_streaming {
        if let Some(buffer) = app.streaming_buffers.get(model_name) {
            if !buffer.is_empty() {
                if !content.is_empty() {
                    content.push_str("\n\n");
                }
                content.push_str(buffer);
            }
        }
        // Add spinner
        if !content.is_empty() {
            content.push(' ');
        }
        content.push(app.spinner_char());
    }

    if content.is_empty() {
        content = "No messages yet".to_string();
    }

    // Build title with focus indicator and spinner if streaming
    let focus_indicator = if is_focused { "► " } else { "" };
    let title = if is_streaming {
        format!("{}{} {}", focus_indicator, app.spinner_char(), model_name)
    } else {
        format!("{}{}", focus_indicator, model_name)
    };

    // Determine border color based on focus
    let border_style = if is_focused {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };

    // Get scroll offset for this panel
    let scroll_offset = app.get_panel_scroll(model_name);

    // Check if auto-scroll is enabled for this model
    let should_auto_scroll = app.auto_scroll.get(model_name).copied().unwrap_or(true);

    // Calculate wrapped content height for auto-scroll
    // We need to account for text wrapping based on the panel width
    let inner_width = area.width.saturating_sub(2) as usize; // -2 for borders
    let visible_height = area.height.saturating_sub(2); // -2 for borders

    // Count wrapped lines by calculating how many visual lines each content line takes
    let wrapped_lines: u16 = if inner_width > 0 {
        content
            .lines()
            .map(|line| {
                let line_len = line.chars().count();
                if line_len == 0 {
                    1 // empty lines still take 1 line
                } else {
                    // Use div_ceil to get number of wrapped lines
                    line_len.div_ceil(inner_width) as u16
                }
            })
            .sum()
    } else {
        content.lines().count() as u16
    };

    // Determine final scroll position
    let final_scroll = if should_auto_scroll && wrapped_lines > visible_height {
        // Auto-scroll to bottom
        wrapped_lines.saturating_sub(visible_height)
    } else if should_auto_scroll {
        // Content fits, no scroll needed
        0
    } else {
        scroll_offset
    };

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .border_style(border_style),
        )
        .wrap(Wrap { trim: true })
        .scroll((final_scroll, 0))
        .style(Style::default().fg(Color::White));

    f.render_widget(paragraph, area);
}

fn render_input(f: &mut Frame, area: Rect, app: &App) {
    // Build title with queue indicator if messages are queued
    let title = if !app.message_queue.is_empty() {
        format!("Input ({} queued)", app.message_queue.len())
    } else {
        "Input".to_string()
    };

    let input = Paragraph::new(app.input.clone())
        .style(Style::default())
        .block(Block::default().borders(Borders::ALL).title(title));

    f.render_widget(input, area);

    // Always show cursor in chat mode
    if app.is_in_chat_mode() {
        let cursor_x = area.x + 1 + app.input.len() as u16;
        let cursor_y = area.y + 1;

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

        // Get the focused model name (first active model)
        let model_name = app.get_active_model_names()[0].clone();

        // In chat mode, up/down should scroll the focused panel
        assert_eq!(app.get_panel_scroll(&model_name), 0);
        app.handle_down();
        assert_eq!(app.get_panel_scroll(&model_name), 1);
        app.handle_up();
        assert_eq!(app.get_panel_scroll(&model_name), 0);
        app.handle_up(); // Should not go below 0
        assert_eq!(app.get_panel_scroll(&model_name), 0);
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

        // Get the focused model name (first active model)
        let model_name = app.get_active_model_names()[0].clone();

        assert_eq!(app.get_panel_scroll(&model_name), 0);

        app.scroll_down();
        assert_eq!(app.get_panel_scroll(&model_name), 1);

        app.scroll_down();
        assert_eq!(app.get_panel_scroll(&model_name), 2);

        app.scroll_up();
        assert_eq!(app.get_panel_scroll(&model_name), 1);

        app.scroll_up();
        assert_eq!(app.get_panel_scroll(&model_name), 0);

        // Should not go below 0
        app.scroll_up();
        assert_eq!(app.get_panel_scroll(&model_name), 0);
    }

    #[test]
    fn test_message_queue_when_loading() {
        let mut app = create_test_app();

        // Simulate loading state
        app.is_loading = true;

        // Type and submit a message while loading
        app.input = "queued message".to_string();
        app.submit_message();

        // Message should be queued, not sent
        assert_eq!(app.message_queue.len(), 1);
        assert_eq!(app.message_queue[0], "queued message");
        assert_eq!(app.input, ""); // Input should be cleared
    }

    #[test]
    fn test_message_queue_multiple() {
        let mut app = create_test_app();
        app.is_loading = true;

        app.input = "first".to_string();
        app.submit_message();
        app.input = "second".to_string();
        app.submit_message();
        app.input = "third".to_string();
        app.submit_message();

        assert_eq!(app.message_queue.len(), 3);
        assert_eq!(app.message_queue[0], "first");
        assert_eq!(app.message_queue[1], "second");
        assert_eq!(app.message_queue[2], "third");
    }

    #[test]
    fn test_submit_empty_message() {
        let mut app = create_test_app();

        app.input = "   ".to_string(); // whitespace only
        app.submit_message();

        assert_eq!(app.messages.len(), 0);
        assert!(!app.is_loading);
    }

    #[test]
    fn test_spinner_char() {
        let app = create_test_app();

        // Default spinner frame is 0
        assert_eq!(app.spinner_char(), '⠋');
    }

    #[test]
    fn test_spinner_frames() {
        let mut app = create_test_app();

        // Test all spinner frames
        let expected = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
        for (i, &expected_char) in expected.iter().enumerate() {
            app.spinner_frame = i;
            assert_eq!(app.spinner_char(), expected_char);
        }
    }

    #[test]
    fn test_is_model_streaming() {
        let mut app = create_test_app();

        // Initially no models are streaming
        assert!(!app.is_model_streaming("Model 1"));

        // Add a streaming receiver
        let (_tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));

        assert!(app.is_model_streaming("Model 1"));
        assert!(!app.is_model_streaming("Model 2"));
    }

    #[test]
    fn test_handle_page_up() {
        let mut app = create_test_app();
        app.toggle_model_selection();

        // Start at index 0
        assert_eq!(app.selected_model_index, 0);

        // Page up from 0 should stay at 0
        app.handle_page_up();
        assert_eq!(app.selected_model_index, 0);
    }

    #[test]
    fn test_handle_page_down() {
        let mut app = create_test_app();
        app.toggle_model_selection();

        // Start at index 0
        assert_eq!(app.selected_model_index, 0);

        // Page down should go to last item (only 3 models)
        app.handle_page_down();
        assert_eq!(app.selected_model_index, 2); // last model
    }

    #[test]
    fn test_page_navigation_not_in_model_selection() {
        let mut app = create_test_app();

        // In chat mode, page up/down should do nothing
        app.selected_model_index = 0;
        app.handle_page_up();
        assert_eq!(app.selected_model_index, 0);

        app.handle_page_down();
        assert_eq!(app.selected_model_index, 0);
    }

    #[test]
    fn test_model_list_offset_on_down() {
        let mut app = create_test_app();
        app.toggle_model_selection();

        // With only 3 models, offset shouldn't change much
        assert_eq!(app.model_list_offset, 0);

        app.handle_down();
        assert_eq!(app.model_list_offset, 0); // Still visible

        app.handle_down();
        assert_eq!(app.model_list_offset, 0); // Still visible
    }

    #[test]
    fn test_model_list_offset_on_up() {
        let mut app = create_test_app();
        app.toggle_model_selection();

        // Set offset and selected index
        app.model_list_offset = 5;
        app.selected_model_index = 5;

        // Move up should adjust offset
        app.handle_up();
        assert_eq!(app.selected_model_index, 4);
        assert_eq!(app.model_list_offset, 4); // Offset follows selection
    }

    #[tokio::test]
    async fn test_handle_enter_in_chat_mode() {
        let mut app = create_test_app();

        app.input = "test message".to_string();
        app.handle_enter();

        // Should have started streaming
        assert!(app.is_loading);
        assert_eq!(app.input, ""); // Input cleared
        assert_eq!(app.messages.len(), 1); // User message added
    }

    #[test]
    fn test_handle_enter_in_model_selection() {
        let mut app = create_test_app();
        app.toggle_model_selection();

        // All models initially active
        assert!(app.active_models[0]);

        // Enter should toggle current model
        app.handle_enter();
        assert!(!app.active_models[0]);
    }

    #[test]
    fn test_app_initial_state() {
        let app = create_test_app();

        // Check all new fields are properly initialized
        assert!(app.streaming_receivers.is_empty());
        assert!(app.streaming_buffers.is_empty());
        assert!(app.message_queue.is_empty());
        assert_eq!(app.spinner_frame, 0);
        assert_eq!(app.model_list_offset, 0);
    }
}
