//! UI Rendering module
//!
//! Contains all functions for rendering the TUI using ratatui.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};
use tuister::Role;
use unicode_width::UnicodeWidthStr;

use crate::ui::app::App;

/// Render the chat mode UI
pub fn render_chat_mode(f: &mut Frame, app: &App) {
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

/// Render the model selection mode UI
pub fn render_model_selection_mode(f: &mut Frame, app: &App) {
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
                let line_len = line.width();
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
        let cursor_x = area.x + 1 + app.input.width() as u16;
        let cursor_y = area.y + 1;

        if cursor_x < area.x + area.width - 1 {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }
}
