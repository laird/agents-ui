use ratatui::text::{Line, Span};
use super::theme;

/// A text input with cursor support for editing text in-place.
#[derive(Debug, Clone)]
pub struct TextInput {
    /// The text content.
    text: String,
    /// Cursor position as a byte offset into `text`.
    cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor: 0,
        }
    }

    /// Create a TextInput pre-filled with text (cursor at end).
    #[allow(dead_code)] // Public API for callers that need pre-filled input
    pub fn with_text(text: String) -> Self {
        let cursor = text.len();
        Self { text, cursor }
    }

    /// Get the current text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Get the cursor position (byte offset).
    #[allow(dead_code)] // Public API for testing and future use
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Check if the input is empty.
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Take the text out, resetting the input.
    pub fn drain(&mut self) -> String {
        self.cursor = 0;
        std::mem::take(&mut self.text)
    }

    /// Set the text content (cursor moves to end).
    pub fn set_text(&mut self, text: String) {
        self.cursor = text.len();
        self.text = text;
    }

    /// Insert a character at the cursor position.
    pub fn insert_char(&mut self, c: char) {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
    }

    /// Delete the character before the cursor (Backspace).
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            // Find the previous character boundary
            let prev = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.text.remove(prev);
            self.cursor = prev;
        }
    }

    /// Delete the character at the cursor position (Delete key).
    pub fn delete(&mut self) {
        if self.cursor < self.text.len() {
            self.text.remove(self.cursor);
        }
    }

    /// Move cursor one character to the left.
    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor = self.text[..self.cursor]
                .char_indices()
                .next_back()
                .map(|(i, _)| i)
                .unwrap_or(0);
        }
    }

    /// Move cursor one character to the right.
    pub fn move_right(&mut self) {
        if self.cursor < self.text.len() {
            self.cursor = self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len());
        }
    }

    /// Move cursor to the beginning of the text.
    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to the end of the text.
    pub fn move_end(&mut self) {
        self.cursor = self.text.len();
    }

    /// Render the input as a styled Line with a visible cursor.
    pub fn render_line(&self, prefix: &str) -> Line<'static> {
        let before = self.text[..self.cursor].to_string();
        let cursor_char = if self.cursor < self.text.len() {
            let ch = self.text[self.cursor..].chars().next().unwrap();
            ch.to_string()
        } else {
            " ".to_string()
        };
        let after = if self.cursor < self.text.len() {
            let next_boundary = self.cursor + cursor_char.len();
            self.text[next_boundary..].to_string()
        } else {
            String::new()
        };

        Line::from(vec![
            Span::styled(format!("{prefix}{before}"), theme::input_style()),
            Span::styled(cursor_char, theme::cursor_style()),
            Span::styled(after, theme::input_style()),
        ])
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_empty() {
        let input = TextInput::new();
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
        assert_eq!(input.text(), "");
    }

    #[test]
    fn insert_and_text() {
        let mut input = TextInput::new();
        input.insert_char('h');
        input.insert_char('i');
        assert_eq!(input.text(), "hi");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn backspace_at_end() {
        let mut input = TextInput::with_text("abc".into());
        input.backspace();
        assert_eq!(input.text(), "ab");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn backspace_at_beginning() {
        let mut input = TextInput::with_text("abc".into());
        input.move_home();
        input.backspace();
        assert_eq!(input.text(), "abc");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn delete_at_cursor() {
        let mut input = TextInput::with_text("abc".into());
        input.move_home();
        input.delete();
        assert_eq!(input.text(), "bc");
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn delete_at_end() {
        let mut input = TextInput::with_text("abc".into());
        input.delete();
        assert_eq!(input.text(), "abc"); // no-op
    }

    #[test]
    fn cursor_movement() {
        let mut input = TextInput::with_text("abc".into());
        assert_eq!(input.cursor(), 3);
        input.move_left();
        assert_eq!(input.cursor(), 2);
        input.move_left();
        assert_eq!(input.cursor(), 1);
        input.move_right();
        assert_eq!(input.cursor(), 2);
        input.move_home();
        assert_eq!(input.cursor(), 0);
        input.move_end();
        assert_eq!(input.cursor(), 3);
    }

    #[test]
    fn insert_in_middle() {
        let mut input = TextInput::with_text("ac".into());
        input.move_home();
        input.move_right(); // cursor at 1
        input.insert_char('b');
        assert_eq!(input.text(), "abc");
        assert_eq!(input.cursor(), 2);
    }

    #[test]
    fn backspace_in_middle() {
        let mut input = TextInput::with_text("abc".into());
        input.move_left(); // cursor at 2
        input.backspace();
        assert_eq!(input.text(), "ac");
        assert_eq!(input.cursor(), 1);
    }

    #[test]
    fn drain_resets() {
        let mut input = TextInput::with_text("hello".into());
        let text = input.drain();
        assert_eq!(text, "hello");
        assert!(input.is_empty());
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn set_text_moves_cursor_to_end() {
        let mut input = TextInput::new();
        input.set_text("hello".into());
        assert_eq!(input.text(), "hello");
        assert_eq!(input.cursor(), 5);
    }

    #[test]
    fn with_text_constructor() {
        let input = TextInput::with_text("test".into());
        assert_eq!(input.text(), "test");
        assert_eq!(input.cursor(), 4);
    }

    #[test]
    fn move_left_at_beginning() {
        let mut input = TextInput::with_text("abc".into());
        input.move_home();
        input.move_left(); // should be no-op
        assert_eq!(input.cursor(), 0);
    }

    #[test]
    fn move_right_at_end() {
        let mut input = TextInput::with_text("abc".into());
        input.move_right(); // should be no-op
        assert_eq!(input.cursor(), 3);
    }
}
