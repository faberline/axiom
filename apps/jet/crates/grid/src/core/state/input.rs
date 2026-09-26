use serde::{Deserialize, Serialize};

/// Represents all possible user input actions
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InputAction {
    // Navigation
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveToStart,
    MoveToEnd,
    MoveToTop,
    MoveToBottom,
    PageUp,
    PageDown,

    // Selection
    ExtendUp,
    ExtendDown,
    ExtendLeft,
    ExtendRight,
    ExtendToStart,
    ExtendToEnd,
    ExtendToTop,
    ExtendToBottom,
    SelectAll,

    // Editing
    StartEdit,
    StartEditClear,
    ConfirmEdit,
    CancelEdit,
    Delete,
    Backspace,

    // Clipboard
    Copy,
    Cut,
    Paste,

    // Other
    Undo,
    Redo,
    Find,
    Replace,
    Save,

    // Character input
    InsertChar(char),
    InsertText(String),

    // Unknown/unmapped
    None,
}

/// Key codes for common keys
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    // Arrow keys
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,

    // Special keys
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,

    // Navigation
    Home,
    End,
    PageUp,
    PageDown,

    // Character key
    Char(char),

    // Unknown
    Unknown,
}

/// Modifier keys state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_shift(mut self, shift: bool) -> Self {
        self.shift = shift;
        self
    }

    pub fn with_ctrl(mut self, ctrl: bool) -> Self {
        self.ctrl = ctrl;
        self
    }

    pub fn with_alt(mut self, alt: bool) -> Self {
        self.alt = alt;
        self
    }

    pub fn with_meta(mut self, meta: bool) -> Self {
        self.meta = meta;
        self
    }

    pub fn none_pressed(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }

    pub fn only_shift(&self) -> bool {
        self.shift && !self.ctrl && !self.alt && !self.meta
    }

    pub fn only_ctrl(&self) -> bool {
        !self.shift && self.ctrl && !self.alt && !self.meta
    }

    pub fn only_meta(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && self.meta
    }

    pub fn ctrl_or_meta(&self) -> bool {
        self.ctrl || self.meta
    }
}

/// Maps a key and modifiers to an InputAction
pub fn key_to_action(key: Key, modifiers: Modifiers) -> InputAction {
    match key {
        // Arrow keys - navigation
        Key::ArrowUp if modifiers.none_pressed() => InputAction::MoveUp,
        Key::ArrowDown if modifiers.none_pressed() => InputAction::MoveDown,
        Key::ArrowLeft if modifiers.none_pressed() => InputAction::MoveLeft,
        Key::ArrowRight if modifiers.none_pressed() => InputAction::MoveRight,

        // Arrow keys with Shift - selection
        Key::ArrowUp if modifiers.only_shift() => InputAction::ExtendUp,
        Key::ArrowDown if modifiers.only_shift() => InputAction::ExtendDown,
        Key::ArrowLeft if modifiers.only_shift() => InputAction::ExtendLeft,
        Key::ArrowRight if modifiers.only_shift() => InputAction::ExtendRight,

        // Home/End keys
        Key::Home if modifiers.none_pressed() => InputAction::MoveToStart,
        Key::End if modifiers.none_pressed() => InputAction::MoveToEnd,
        Key::Home if modifiers.only_shift() => InputAction::ExtendToStart,
        Key::End if modifiers.only_shift() => InputAction::ExtendToEnd,

        // Ctrl+Home/End (or Meta on Mac)
        Key::Home if modifiers.ctrl_or_meta() && !modifiers.shift => InputAction::MoveToTop,
        Key::End if modifiers.ctrl_or_meta() && !modifiers.shift => InputAction::MoveToBottom,
        Key::Home if modifiers.ctrl_or_meta() && modifiers.shift => InputAction::ExtendToTop,
        Key::End if modifiers.ctrl_or_meta() && modifiers.shift => InputAction::ExtendToBottom,

        // Page Up/Down
        Key::PageUp if modifiers.none_pressed() => InputAction::PageUp,
        Key::PageDown if modifiers.none_pressed() => InputAction::PageDown,

        // Enter key
        Key::Enter if modifiers.none_pressed() => InputAction::ConfirmEdit,
        Key::Enter if modifiers.shift => InputAction::MoveUp,

        // Escape
        Key::Escape => InputAction::CancelEdit,

        // Tab
        Key::Tab if modifiers.none_pressed() => InputAction::MoveRight,
        Key::Tab if modifiers.only_shift() => InputAction::MoveLeft,

        // Delete/Backspace
        Key::Delete => InputAction::Delete,
        Key::Backspace => InputAction::Backspace,

        // Ctrl/Meta shortcuts
        Key::Char('a') if modifiers.ctrl_or_meta() => InputAction::SelectAll,
        Key::Char('c') if modifiers.ctrl_or_meta() => InputAction::Copy,
        Key::Char('x') if modifiers.ctrl_or_meta() => InputAction::Cut,
        Key::Char('v') if modifiers.ctrl_or_meta() => InputAction::Paste,
        Key::Char('z') if modifiers.ctrl_or_meta() && !modifiers.shift => InputAction::Undo,
        Key::Char('z') if modifiers.ctrl_or_meta() && modifiers.shift => InputAction::Redo,
        Key::Char('y') if modifiers.ctrl_or_meta() => InputAction::Redo,
        Key::Char('f') if modifiers.ctrl_or_meta() => InputAction::Find,
        Key::Char('h') if modifiers.ctrl_or_meta() => InputAction::Replace,
        Key::Char('s') if modifiers.ctrl_or_meta() => InputAction::Save,

        // Regular character input
        Key::Char(c) if modifiers.none_pressed() || modifiers.only_shift() => {
            InputAction::InsertChar(c)
        }

        // Unknown
        _ => InputAction::None,
    }
}

/// Helper function to create an action for starting edit and typing a character
pub fn start_edit_with_char(c: char) -> InputAction {
    InputAction::InsertChar(c)
}

/// Check if an action is a navigation action
pub fn is_navigation_action(action: &InputAction) -> bool {
    matches!(
        action,
        InputAction::MoveUp
            | InputAction::MoveDown
            | InputAction::MoveLeft
            | InputAction::MoveRight
            | InputAction::MoveToStart
            | InputAction::MoveToEnd
            | InputAction::MoveToTop
            | InputAction::MoveToBottom
            | InputAction::PageUp
            | InputAction::PageDown
    )
}

/// Check if an action is a selection action
pub fn is_selection_action(action: &InputAction) -> bool {
    matches!(
        action,
        InputAction::ExtendUp
            | InputAction::ExtendDown
            | InputAction::ExtendLeft
            | InputAction::ExtendRight
            | InputAction::ExtendToStart
            | InputAction::ExtendToEnd
            | InputAction::ExtendToTop
            | InputAction::ExtendToBottom
            | InputAction::SelectAll
    )
}

/// Check if an action is an editing action
pub fn is_editing_action(action: &InputAction) -> bool {
    matches!(
        action,
        InputAction::StartEdit
            | InputAction::StartEditClear
            | InputAction::ConfirmEdit
            | InputAction::CancelEdit
            | InputAction::Delete
            | InputAction::Backspace
            | InputAction::InsertChar(_)
            | InputAction::InsertText(_)
    )
}

/// Check if an action is a clipboard action
pub fn is_clipboard_action(action: &InputAction) -> bool {
    matches!(
        action,
        InputAction::Copy | InputAction::Cut | InputAction::Paste
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // Key enum tests
    #[test]
    fn test_key_variants() {
        assert_ne!(Key::ArrowUp, Key::ArrowDown);
        assert_ne!(Key::Enter, Key::Escape);
        assert_eq!(Key::Char('a'), Key::Char('a'));
        assert_ne!(Key::Char('a'), Key::Char('b'));
    }

    // Modifiers tests
    #[test]
    fn test_modifiers_new() {
        let mods = Modifiers::new();
        assert!(!mods.shift);
        assert!(!mods.ctrl);
        assert!(!mods.alt);
        assert!(!mods.meta);
    }

    #[test]
    fn test_modifiers_default() {
        let mods = Modifiers::default();
        assert_eq!(mods, Modifiers::new());
    }

    #[test]
    fn test_modifiers_with_shift() {
        let mods = Modifiers::new().with_shift(true);
        assert!(mods.shift);
        assert!(!mods.ctrl);
    }

    #[test]
    fn test_modifiers_with_ctrl() {
        let mods = Modifiers::new().with_ctrl(true);
        assert!(!mods.shift);
        assert!(mods.ctrl);
    }

    #[test]
    fn test_modifiers_with_meta() {
        let mods = Modifiers::new().with_meta(true);
        assert!(mods.meta);
        assert!(!mods.ctrl);
    }

    #[test]
    fn test_modifiers_chaining() {
        let mods = Modifiers::new().with_shift(true).with_ctrl(true);
        assert!(mods.shift);
        assert!(mods.ctrl);
        assert!(!mods.alt);
    }

    #[test]
    fn test_modifiers_none_pressed() {
        let mods = Modifiers::new();
        assert!(mods.none_pressed());

        let mods = Modifiers::new().with_shift(true);
        assert!(!mods.none_pressed());
    }

    #[test]
    fn test_modifiers_only_shift() {
        let mods = Modifiers::new().with_shift(true);
        assert!(mods.only_shift());

        let mods = Modifiers::new().with_shift(true).with_ctrl(true);
        assert!(!mods.only_shift());
    }

    #[test]
    fn test_modifiers_only_ctrl() {
        let mods = Modifiers::new().with_ctrl(true);
        assert!(mods.only_ctrl());

        let mods = Modifiers::new().with_ctrl(true).with_shift(true);
        assert!(!mods.only_ctrl());
    }

    #[test]
    fn test_modifiers_only_meta() {
        let mods = Modifiers::new().with_meta(true);
        assert!(mods.only_meta());

        let mods = Modifiers::new().with_meta(true).with_shift(true);
        assert!(!mods.only_meta());
    }

    #[test]
    fn test_modifiers_ctrl_or_meta() {
        let ctrl = Modifiers::new().with_ctrl(true);
        assert!(ctrl.ctrl_or_meta());

        let meta = Modifiers::new().with_meta(true);
        assert!(meta.ctrl_or_meta());

        let none = Modifiers::new();
        assert!(!none.ctrl_or_meta());
    }

    // key_to_action tests
    #[test]
    fn test_arrow_keys_navigation() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::ArrowUp, mods), InputAction::MoveUp);
        assert_eq!(key_to_action(Key::ArrowDown, mods), InputAction::MoveDown);
        assert_eq!(key_to_action(Key::ArrowLeft, mods), InputAction::MoveLeft);
        assert_eq!(key_to_action(Key::ArrowRight, mods), InputAction::MoveRight);
    }

    #[test]
    fn test_arrow_keys_selection() {
        let mods = Modifiers::new().with_shift(true);
        assert_eq!(key_to_action(Key::ArrowUp, mods), InputAction::ExtendUp);
        assert_eq!(key_to_action(Key::ArrowDown, mods), InputAction::ExtendDown);
        assert_eq!(key_to_action(Key::ArrowLeft, mods), InputAction::ExtendLeft);
        assert_eq!(
            key_to_action(Key::ArrowRight, mods),
            InputAction::ExtendRight
        );
    }

    #[test]
    fn test_home_end_keys() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::Home, mods), InputAction::MoveToStart);
        assert_eq!(key_to_action(Key::End, mods), InputAction::MoveToEnd);
    }

    #[test]
    fn test_home_end_with_shift() {
        let mods = Modifiers::new().with_shift(true);
        assert_eq!(key_to_action(Key::Home, mods), InputAction::ExtendToStart);
        assert_eq!(key_to_action(Key::End, mods), InputAction::ExtendToEnd);
    }

    #[test]
    fn test_home_end_with_ctrl() {
        let mods = Modifiers::new().with_ctrl(true);
        assert_eq!(key_to_action(Key::Home, mods), InputAction::MoveToTop);
        assert_eq!(key_to_action(Key::End, mods), InputAction::MoveToBottom);
    }

    #[test]
    fn test_home_end_with_ctrl_shift() {
        let mods = Modifiers::new().with_ctrl(true).with_shift(true);
        assert_eq!(key_to_action(Key::Home, mods), InputAction::ExtendToTop);
        assert_eq!(key_to_action(Key::End, mods), InputAction::ExtendToBottom);
    }

    #[test]
    fn test_home_end_with_meta() {
        let mods = Modifiers::new().with_meta(true);
        assert_eq!(key_to_action(Key::Home, mods), InputAction::MoveToTop);
        assert_eq!(key_to_action(Key::End, mods), InputAction::MoveToBottom);
    }

    #[test]
    fn test_page_up_down() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::PageUp, mods), InputAction::PageUp);
        assert_eq!(key_to_action(Key::PageDown, mods), InputAction::PageDown);
    }

    #[test]
    fn test_enter_key() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::Enter, mods), InputAction::ConfirmEdit);

        let mods = Modifiers::new().with_shift(true);
        assert_eq!(key_to_action(Key::Enter, mods), InputAction::MoveUp);
    }

    #[test]
    fn test_escape_key() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::Escape, mods), InputAction::CancelEdit);
    }

    #[test]
    fn test_tab_key() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::Tab, mods), InputAction::MoveRight);

        let mods = Modifiers::new().with_shift(true);
        assert_eq!(key_to_action(Key::Tab, mods), InputAction::MoveLeft);
    }

    #[test]
    fn test_delete_backspace() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::Delete, mods), InputAction::Delete);
        assert_eq!(key_to_action(Key::Backspace, mods), InputAction::Backspace);
    }

    #[test]
    fn test_ctrl_shortcuts() {
        let mods = Modifiers::new().with_ctrl(true);

        assert_eq!(key_to_action(Key::Char('a'), mods), InputAction::SelectAll);
        assert_eq!(key_to_action(Key::Char('c'), mods), InputAction::Copy);
        assert_eq!(key_to_action(Key::Char('x'), mods), InputAction::Cut);
        assert_eq!(key_to_action(Key::Char('v'), mods), InputAction::Paste);
        assert_eq!(key_to_action(Key::Char('z'), mods), InputAction::Undo);
        assert_eq!(key_to_action(Key::Char('y'), mods), InputAction::Redo);
        assert_eq!(key_to_action(Key::Char('f'), mods), InputAction::Find);
        assert_eq!(key_to_action(Key::Char('h'), mods), InputAction::Replace);
        assert_eq!(key_to_action(Key::Char('s'), mods), InputAction::Save);
    }

    #[test]
    fn test_ctrl_shift_z_for_redo() {
        let mods = Modifiers::new().with_ctrl(true).with_shift(true);
        assert_eq!(key_to_action(Key::Char('z'), mods), InputAction::Redo);
    }

    #[test]
    fn test_meta_shortcuts() {
        let mods = Modifiers::new().with_meta(true);

        assert_eq!(key_to_action(Key::Char('a'), mods), InputAction::SelectAll);
        assert_eq!(key_to_action(Key::Char('c'), mods), InputAction::Copy);
        assert_eq!(key_to_action(Key::Char('x'), mods), InputAction::Cut);
        assert_eq!(key_to_action(Key::Char('v'), mods), InputAction::Paste);
    }

    #[test]
    fn test_char_input() {
        let mods = Modifiers::new();
        assert_eq!(
            key_to_action(Key::Char('a'), mods),
            InputAction::InsertChar('a')
        );
        assert_eq!(
            key_to_action(Key::Char('1'), mods),
            InputAction::InsertChar('1')
        );
    }

    #[test]
    fn test_char_input_with_shift() {
        let mods = Modifiers::new().with_shift(true);
        assert_eq!(
            key_to_action(Key::Char('A'), mods),
            InputAction::InsertChar('A')
        );
    }

    #[test]
    fn test_unknown_key() {
        let mods = Modifiers::new();
        assert_eq!(key_to_action(Key::Unknown, mods), InputAction::None);
    }

    #[test]
    fn test_unknown_modifier_combination() {
        let mods = Modifiers::new().with_alt(true);
        assert_eq!(key_to_action(Key::ArrowUp, mods), InputAction::None);
    }

    // Helper function tests
    #[test]
    fn test_start_edit_with_char() {
        assert_eq!(start_edit_with_char('a'), InputAction::InsertChar('a'));
        assert_eq!(start_edit_with_char('5'), InputAction::InsertChar('5'));
    }

    #[test]
    fn test_is_navigation_action() {
        assert!(is_navigation_action(&InputAction::MoveUp));
        assert!(is_navigation_action(&InputAction::MoveDown));
        assert!(is_navigation_action(&InputAction::PageUp));
        assert!(!is_navigation_action(&InputAction::Copy));
        assert!(!is_navigation_action(&InputAction::ExtendUp));
    }

    #[test]
    fn test_is_selection_action() {
        assert!(is_selection_action(&InputAction::ExtendUp));
        assert!(is_selection_action(&InputAction::SelectAll));
        assert!(!is_selection_action(&InputAction::MoveUp));
        assert!(!is_selection_action(&InputAction::Copy));
    }

    #[test]
    fn test_is_editing_action() {
        assert!(is_editing_action(&InputAction::StartEdit));
        assert!(is_editing_action(&InputAction::Delete));
        assert!(is_editing_action(&InputAction::InsertChar('a')));
        assert!(!is_editing_action(&InputAction::Copy));
        assert!(!is_editing_action(&InputAction::MoveUp));
    }

    #[test]
    fn test_is_clipboard_action() {
        assert!(is_clipboard_action(&InputAction::Copy));
        assert!(is_clipboard_action(&InputAction::Cut));
        assert!(is_clipboard_action(&InputAction::Paste));
        assert!(!is_clipboard_action(&InputAction::Delete));
        assert!(!is_clipboard_action(&InputAction::MoveUp));
    }

    #[test]
    fn test_input_action_equality() {
        assert_eq!(InputAction::MoveUp, InputAction::MoveUp);
        assert_ne!(InputAction::MoveUp, InputAction::MoveDown);
        assert_eq!(InputAction::InsertChar('a'), InputAction::InsertChar('a'));
        assert_ne!(InputAction::InsertChar('a'), InputAction::InsertChar('b'));
    }

    #[test]
    fn test_serialization() {
        let action = InputAction::Copy;
        let serialized = serde_json::to_string(&action).unwrap();
        let deserialized: InputAction = serde_json::from_str(&serialized).unwrap();
        assert_eq!(action, deserialized);
    }

    #[test]
    fn test_insert_char_serialization() {
        let action = InputAction::InsertChar('x');
        let serialized = serde_json::to_string(&action).unwrap();
        let deserialized: InputAction = serde_json::from_str(&serialized).unwrap();
        assert_eq!(action, deserialized);
    }
}
