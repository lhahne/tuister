//! App state management module
//!
//! Contains the App struct that holds all application state and handles user interactions.

use std::collections::HashMap;
use tokio::sync::mpsc;
use tuister::config::Config;
use tuister::{ChatSession, Model, OpenRouterClient, Role};

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum AppMode {
    Chat,
    ModelSelection,
}

#[derive(Clone)]
pub struct DisplayMessage {
    pub role: Role,
    pub content: String,
    pub model_name: Option<String>,
}

pub struct App {
    pub(crate) session: ChatSession,
    pub(crate) input: String,
    pub(crate) messages: Vec<DisplayMessage>,
    pub(crate) model_responses: HashMap<String, Vec<String>>,
    pub(crate) active_models: Vec<bool>,
    pub(crate) is_loading: bool,
    pub(crate) mode: AppMode,
    pub(crate) available_models: Vec<Model>,
    pub(crate) selected_model_index: usize,
    pub(crate) model_list_offset: usize,
    // Streaming state
    pub(crate) streaming_receivers: Vec<(String, mpsc::UnboundedReceiver<String>)>,
    pub(crate) streaming_buffers: HashMap<String, String>,
    // Message queue for messages submitted while streaming
    pub(crate) message_queue: Vec<String>,
    // Spinner animation
    pub(crate) spinner_frame: usize,
    pub(crate) last_spinner_update: std::time::Instant,
    // Panel scroll state
    pub(crate) panel_scrolls: HashMap<String, u16>,
    pub(crate) focused_panel: usize,
    pub(crate) auto_scroll: HashMap<String, bool>,
}

impl App {
    /// Create a new App, optionally with saved model selection
    pub fn new(
        session: ChatSession,
        available_models: Vec<Model>,
        saved_model_ids: Option<Vec<String>>,
    ) -> Self {
        let num_models = available_models.len();

        // Determine active models based on saved selection or defaults
        let active_models: Vec<bool> = if let Some(saved_ids) = saved_model_ids {
            // Mark models as active if their ID is in the saved list
            available_models
                .iter()
                .map(|m| saved_ids.contains(&m.id))
                .collect()
        } else {
            // Default: first 3 models
            (0..num_models).map(|i| i < 3).collect()
        };

        // Ensure at least one model is active
        let active_models = if active_models.iter().any(|&x| x) {
            active_models
        } else {
            (0..num_models).map(|i| i < 1).collect()
        };

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

        // Save the selected model IDs to config
        let selected_ids: Vec<String> = selected_models.iter().map(|m| m.id.clone()).collect();
        let config = Config::with_selected_models(selected_ids);
        let _ = config.save(); // Ignore save errors silently

        // Create a new session with selected models
        // We need to preserve the API key from the old session
        let api_key =
            std::env::var("OPENROUTER_API_KEY").unwrap_or_else(|_| "test_key".to_string());
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
    pub fn get_focused_model_name(&self) -> Option<String> {
        let active_names = self.get_active_model_names();
        active_names.get(self.focused_panel).cloned()
    }

    /// Get list of active model names in display order
    pub fn get_active_model_names(&self) -> Vec<String> {
        self.available_models
            .iter()
            .enumerate()
            .filter(|(i, _)| *i < self.active_models.len() && self.active_models[*i])
            .map(|(_, m)| m.name.clone())
            .collect()
    }

    /// Enable auto-scroll for a model (called when streaming starts)
    pub fn enable_auto_scroll(&mut self, model_name: &str) {
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
    pub fn is_model_streaming(&self, model_name: &str) -> bool {
        self.streaming_receivers
            .iter()
            .any(|(name, _)| name == model_name)
    }

    /// Get the current spinner character
    pub fn spinner_char(&self) -> char {
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
