//! UI Module
//!
//! This module provides the main UI entry point and re-exports from submodules.

mod app;
mod markdown;
mod render;

pub use app::{App, AppMode};
use ratatui::Frame;
use render::{render_chat_mode, render_model_selection_mode};

/// Main UI rendering function - dispatches to appropriate renderer based on app mode
pub fn ui(f: &mut Frame, app: &mut App) {
    match app.mode {
        AppMode::Chat | AppMode::About => render_chat_mode(f, app),
        AppMode::ModelSelection => render_model_selection_mode(f, app),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::app::DisplayMessage;
    use tokio::sync::mpsc;
    use tuister::{ChatSession, Model, OpenRouterClient, Role};

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
        App::new(session, available_models, None)
    }

    fn create_test_app_with_model_names(names: &[&str]) -> App {
        let api_key = "test_key".to_string();
        let client = OpenRouterClient::new(api_key).unwrap();
        let models: Vec<Model> = names
            .iter()
            .enumerate()
            .map(|(index, name)| Model::new(format!("model-{}", index), (*name).to_string()))
            .collect();
        let available_models = models.clone();
        let session = ChatSession::new(client, models);
        App::new(session, available_models, None)
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
        app.set_model_list_height(3);

        // With only 3 models, offset shouldn't change much
        assert_eq!(app.model_list_offset, 0);

        app.handle_down();
        assert_eq!(app.model_list_offset, 0); // Still visible

        app.handle_down();
        assert_eq!(app.model_list_offset, 0); // Still visible
    }

    #[test]
    fn test_model_list_offset_on_up() {
        let mut app = create_test_app_with_model_names(&[
            "Alpha", "Beta", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel",
            "India",
        ]);
        app.toggle_model_selection();
        app.set_model_list_height(5);

        // Set offset and selected index
        app.model_list_offset = 5;
        app.selected_model_index = 5;

        // Move up should adjust offset
        app.handle_up();
        assert_eq!(app.selected_model_index, 4);
        assert_eq!(app.model_list_offset, 4); // Offset follows selection
    }

    #[test]
    fn test_model_list_offset_moves_with_selection() {
        let mut app = create_test_app_with_model_names(&[
            "Alpha", "Beta", "Bravo", "Charlie", "Delta", "Echo", "Foxtrot", "Golf", "Hotel",
            "India",
        ]);
        app.toggle_model_selection();
        app.set_model_list_height(5);

        for _ in 0..7 {
            app.handle_down();
        }

        assert_eq!(app.selected_model_index, 7);
        assert_eq!(app.model_list_offset, 3);

        app.handle_up();
        assert_eq!(app.selected_model_index, 6);
        assert_eq!(app.model_list_offset, 3);

        app.handle_up();
        assert_eq!(app.selected_model_index, 5);
        assert_eq!(app.model_list_offset, 3);

        app.handle_up();
        assert_eq!(app.selected_model_index, 4);
        assert_eq!(app.model_list_offset, 3);

        app.handle_up();
        assert_eq!(app.selected_model_index, 3);
        assert_eq!(app.model_list_offset, 3);

        app.handle_up();
        assert_eq!(app.selected_model_index, 2);
        assert_eq!(app.model_list_offset, 2);
    }

    #[test]
    fn test_jump_to_next_model_starting_with_letter() {
        let mut app = create_test_app_with_model_names(&[
            "Alpha", "Beta", "Bravo", "Charlie", "Delta", "Echo",
        ]);
        app.toggle_model_selection();
        app.set_model_list_height(3);

        app.input_char('b');
        assert_eq!(app.selected_model_index, 1);
        assert_eq!(app.model_list_offset, 0);

        app.input_char('b');
        assert_eq!(app.selected_model_index, 2);
        assert_eq!(app.model_list_offset, 0);

        app.input_char('b');
        assert_eq!(app.selected_model_index, 1);
        assert_eq!(app.model_list_offset, 0);

        app.input_char('d');
        assert_eq!(app.selected_model_index, 4);
        assert_eq!(app.model_list_offset, 2);
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
        assert_eq!(app.model_list_height, 0);
    }

    #[test]
    fn test_clear_chat() {
        let mut app = create_test_app();

        // Add some state
        app.input = "test input".to_string();
        app.messages.push(DisplayMessage {
            role: Role::User,
            content: "test message".to_string(),
            model_name: None,
        });
        app.session.add_user_message("test message".to_string());

        // Clear chat
        app.clear_chat();

        // Verify state is cleared
        assert!(app.input.is_empty());
        assert!(app.messages.is_empty());
        assert!(app.model_responses.is_empty());
        assert!(app.message_queue.is_empty());
        assert!(app.session.messages().is_empty());
    }

    #[test]
    fn test_clear_chat_shortcut_in_chat_mode() {
        let mut app = create_test_app();

        app.input = "test input".to_string();
        app.messages.push(DisplayMessage {
            role: Role::User,
            content: "test message".to_string(),
            model_name: None,
        });
        app.session.add_user_message("test message".to_string());

        let cleared = app.clear_chat_if_in_chat_mode();

        assert!(cleared);
        assert!(app.input.is_empty());
        assert!(app.messages.is_empty());
        assert!(app.model_responses.is_empty());
        assert!(app.message_queue.is_empty());
        assert!(app.session.messages().is_empty());
    }

    #[test]
    fn test_clear_chat_shortcut_ignored_outside_chat_mode() {
        let mut app = create_test_app();
        app.toggle_model_selection();

        app.input = "test input".to_string();
        app.messages.push(DisplayMessage {
            role: Role::User,
            content: "test message".to_string(),
            model_name: None,
        });

        let cleared = app.clear_chat_if_in_chat_mode();

        assert!(!cleared);
        assert_eq!(app.input, "test input");
        assert_eq!(app.messages.len(), 1);
    }

    #[test]
    fn test_is_streaming_when_loading() {
        let mut app = create_test_app();
        app.is_loading = true;
        assert!(app.is_streaming());
    }

    #[test]
    fn test_is_streaming_with_receivers() {
        let mut app = create_test_app();
        let (_tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        assert!(app.is_streaming());
    }

    #[test]
    fn test_cancel_streaming_when_idle() {
        let mut app = create_test_app();
        let was_streaming = app.cancel_streaming();
        assert!(!was_streaming);
        assert!(!app.is_streaming());
    }

    #[test]
    fn test_cancel_streaming_clears_state() {
        let mut app = create_test_app();

        // Set up streaming state
        app.is_loading = true;
        let (_tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        app.streaming_buffers
            .insert("Model 1".to_string(), "partial response".to_string());
        app.message_queue.push("queued message".to_string());

        let was_streaming = app.cancel_streaming();

        assert!(was_streaming);
        assert!(!app.is_loading);
        assert!(app.streaming_receivers.is_empty());
        assert!(app.streaming_buffers.is_empty());
        assert!(app.message_queue.is_empty());
    }

    #[test]
    fn test_cancel_streaming_preserves_partial_response() {
        let mut app = create_test_app();

        // Set up streaming state with partial response
        app.is_loading = true;
        let (_tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        app.streaming_buffers
            .insert("Model 1".to_string(), "This is a partial".to_string());

        app.cancel_streaming();

        // Check that the partial response was preserved in messages
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].content.contains("This is a partial"));
        assert!(app.messages[0].content.contains("[response terminated]"));
    }

    #[test]
    fn test_cancel_streaming_empty_buffer_not_preserved() {
        let mut app = create_test_app();

        // Set up streaming state with empty buffer
        app.is_loading = true;
        let (_tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        app.streaming_buffers
            .insert("Model 1".to_string(), "".to_string());

        app.cancel_streaming();

        // Empty buffers should not create messages
        assert_eq!(app.messages.len(), 0);
    }

    // Snapshot tests for UI rendering
    use ratatui::{backend::TestBackend, Terminal};

    #[test]
    fn test_render_chat_mode_snapshot() {
        let mut app = create_test_app();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_model_selection_snapshot() {
        let mut app = create_test_app();
        app.toggle_model_selection();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_with_input_snapshot() {
        let mut app = create_test_app();
        app.input = "Hello, world!".to_string();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_loading_state_snapshot() {
        let mut app = create_test_app();
        app.is_loading = true;
        app.streaming_buffers
            .insert("Model 1".to_string(), "Loading response...".to_string());
        let (_tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_render_about_snapshot() {
        let mut app = create_test_app();
        app.toggle_about();
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal.draw(|f| ui(f, &mut app)).unwrap();
        insta::assert_snapshot!(terminal.backend());
    }

    #[test]
    fn test_error_message_displayed_in_messages() {
        let mut app = create_test_app();

        // Set up streaming with a channel we control
        app.is_loading = true;
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        app.streaming_buffers
            .insert("Model 1".to_string(), String::new());

        // Send error message and close channel (simulates what chat.rs should do on error)
        tx.send("[Error: Connection failed]".to_string()).unwrap();
        drop(tx);

        // Poll streaming to process the message
        app.poll_streaming();

        // Verify error appears in messages
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].content.contains("[Error"));
        assert_eq!(app.messages[0].model_name, Some("Model 1".to_string()));
    }

    #[test]
    fn test_error_message_with_partial_response() {
        let mut app = create_test_app();

        // Set up streaming with partial content already received
        app.is_loading = true;
        let (tx, rx) = mpsc::unbounded_channel::<String>();
        app.streaming_receivers.push(("Model 1".to_string(), rx));
        app.streaming_buffers
            .insert("Model 1".to_string(), "Partial response...".to_string());

        // Send error after partial response
        tx.send("\n\n[Error: Connection lost]".to_string()).unwrap();
        drop(tx);

        // Poll streaming
        app.poll_streaming();

        // Verify both partial response and error are preserved
        assert_eq!(app.messages.len(), 1);
        assert!(app.messages[0].content.contains("Partial response"));
        assert!(app.messages[0].content.contains("[Error"));
    }
}
