use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

fn sanitize_terminal_text(input: &str) -> String {
    enum State {
        Normal,
        Esc,
        Csi,
        Osc,
        OscEsc,
    }

    let mut output = String::with_capacity(input.len());
    let mut state = State::Normal;

    for ch in input.chars() {
        match state {
            State::Normal => {
                if ch == '\x1b' {
                    state = State::Esc;
                } else if ch.is_control() && ch != '\n' && ch != '\t' {
                    continue;
                } else {
                    output.push(ch);
                }
            }
            State::Esc => {
                state = match ch {
                    '[' => State::Csi,
                    ']' => State::Osc,
                    _ => State::Normal,
                };
            }
            State::Csi => {
                if ('@'..='~').contains(&ch) {
                    state = State::Normal;
                }
            }
            State::Osc => {
                if ch == '\x07' {
                    state = State::Normal;
                } else if ch == '\x1b' {
                    state = State::OscEsc;
                }
            }
            State::OscEsc => {
                if ch == '\\' {
                    state = State::Normal;
                } else if ch != '\x1b' {
                    state = State::Osc;
                }
            }
        }
    }

    output
}

pub fn parse_markdown(markdown: &str) -> Text<'_> {
    let sanitized = sanitize_terminal_text(markdown);
    let mut lines = Vec::new();
    let mut current_line = Vec::new();
    let mut style_stack = Vec::new();

    // Default style
    style_stack.push(Style::default());

    let parser = Parser::new(&sanitized);

    for event in parser {
        match event {
            Event::Start(tag) => {
                let current_style = *style_stack.last().unwrap_or(&Style::default());
                match tag {
                    Tag::Heading { level, .. } => {
                        let header_style = match level {
                            pulldown_cmark::HeadingLevel::H1 => {
                                current_style.fg(Color::Cyan).add_modifier(Modifier::BOLD)
                            }
                            pulldown_cmark::HeadingLevel::H2 => current_style
                                .fg(Color::LightBlue)
                                .add_modifier(Modifier::BOLD),
                            _ => current_style.fg(Color::Blue).add_modifier(Modifier::BOLD),
                        };
                        style_stack.push(header_style);
                        // Header prefix
                        let prefix = "#".repeat(level as usize);
                        current_line.push(Span::styled(format!("{} ", prefix), header_style));
                    }
                    Tag::Strong => {
                        style_stack.push(current_style.add_modifier(Modifier::BOLD));
                    }
                    Tag::Emphasis => {
                        style_stack.push(current_style.add_modifier(Modifier::ITALIC));
                    }
                    Tag::CodeBlock(_) => {
                        style_stack.push(current_style.fg(Color::LightYellow));
                        // Start a new line for code blocks if not already at the start
                        if !current_line.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line)));
                        }
                    }
                    Tag::List(_first_num) => {
                        style_stack.push(current_style);
                    }
                    Tag::Item => {
                        current_line.push(Span::raw("  • "));
                        style_stack.push(current_style);
                    }
                    Tag::BlockQuote(_) => {
                        style_stack.push(
                            current_style
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::ITALIC),
                        );
                        current_line.push(Span::raw("> "));
                    }
                    _ => style_stack.push(current_style),
                }
            }
            Event::End(tag) => {
                style_stack.pop();
                match tag {
                    TagEnd::Heading(_)
                    | TagEnd::Item
                    | TagEnd::Paragraph
                    | TagEnd::BlockQuote(_) => {
                        if !current_line.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line)));
                        }
                        if !matches!(tag, TagEnd::Item) {
                            lines.push(Line::from(""));
                        }
                    }
                    TagEnd::CodeBlock => {
                        if !current_line.is_empty() {
                            lines.push(Line::from(std::mem::take(&mut current_line)));
                        }
                        lines.push(Line::from(""));
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                let current_style = *style_stack.last().unwrap_or(&Style::default());
                let content = text.to_string();
                if content.contains('\n') {
                    let parts: Vec<&str> = content.split('\n').collect();
                    for (i, part) in parts.iter().enumerate() {
                        if !part.is_empty() {
                            current_line.push(Span::styled(part.to_string(), current_style));
                        }
                        if i < parts.len() - 1 {
                            lines.push(Line::from(std::mem::take(&mut current_line)));
                        }
                    }
                } else {
                    current_line.push(Span::styled(content, current_style));
                }
            }
            Event::Code(code) => {
                let current_style = *style_stack.last().unwrap_or(&Style::default());
                current_line.push(Span::styled(
                    format!(" `{}` ", code),
                    current_style.fg(Color::LightYellow),
                ));
            }
            Event::SoftBreak | Event::HardBreak => {
                lines.push(Line::from(std::mem::take(&mut current_line)));
            }
            _ => {}
        }
    }

    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    Text::from(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let text = parse_markdown("Hello world");
        assert_eq!(text.lines.len(), 2); // Text line + empty line after paragraph
        assert_eq!(text.lines[0].spans[0].content, "Hello world");
    }

    #[test]
    fn test_sanitizes_ansi_sequences() {
        let text = parse_markdown("\x1b[31mred\x1b[0m \x1b]0;title\x07ok");
        assert_eq!(text.lines[0].spans[0].content, "red ok");
    }

    #[test]
    fn test_parse_bold_italic() {
        let text = parse_markdown("**bold** *italic*");
        assert_eq!(text.lines[0].spans[0].content, "bold");
        assert_eq!(text.lines[0].spans[0].style.add_modifier, Modifier::BOLD);
        assert_eq!(text.lines[0].spans[2].content, "italic");
        assert_eq!(text.lines[0].spans[2].style.add_modifier, Modifier::ITALIC);
    }

    #[test]
    fn test_parse_heading() {
        let text = parse_markdown("# Heading 1");
        assert_eq!(text.lines[0].spans[0].content, "# ");
        assert_eq!(text.lines[0].spans[1].content, "Heading 1");
        assert_eq!(text.lines[0].spans[1].style.fg, Some(Color::Cyan));
        assert_eq!(text.lines[0].spans[1].style.add_modifier, Modifier::BOLD);
    }

    #[test]
    fn test_parse_list() {
        let text = parse_markdown("- Item 1\n- Item 2");
        // Each item is a line, plus paragraph end logic might add lines
        assert!(text
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content == "  • ")));
        assert!(text
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content == "Item 1")));
        assert!(text
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content == "Item 2")));
    }

    #[test]
    fn test_parse_inline_code() {
        let text = parse_markdown("Use `code` here");
        assert_eq!(text.lines[0].spans[1].content, " `code` ");
        assert_eq!(text.lines[0].spans[1].style.fg, Some(Color::LightYellow));
    }

    #[test]
    fn test_parse_blockquote() {
        let text = parse_markdown("> quote");
        assert_eq!(text.lines[0].spans[0].content, "> ");
        assert_eq!(text.lines[0].spans[1].content, "quote");
        assert_eq!(text.lines[0].spans[1].style.fg, Some(Color::DarkGray));
        assert_eq!(text.lines[0].spans[1].style.add_modifier, Modifier::ITALIC);
    }

    #[test]
    fn test_parse_code_block() {
        let text = parse_markdown("```\nline 1\nline 2\n```");
        // Code blocks currently start on a new line if needed, and each line is a Line
        assert!(text
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content == "line 1")));
        assert!(text
            .lines
            .iter()
            .any(|l| l.spans.iter().any(|s| s.content == "line 2")));
        // Check style of code block content
        for line in &text.lines {
            for span in &line.spans {
                if span.content == "line 1" || span.content == "line 2" {
                    assert_eq!(span.style.fg, Some(Color::LightYellow));
                }
            }
        }
    }
}
