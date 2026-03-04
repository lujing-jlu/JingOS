#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEvent {
    Char(char),
    Ctrl(char),
    Alt(char),
    CtrlAlt(char),
    Enter,
    Backspace,
    Tab,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    WordLeft,
    WordRight,
    Insert,
    Delete,
    WordDeleteLeft,
    WordDeleteRight,
    Home,
    End,
    PageUp,
    PageDown,
    Function(u8),
    Unknown(u8),
}

#[derive(Debug, Clone, Copy)]
pub struct KeyboardDecoder {
    extended: bool,
    left_shift: bool,
    right_shift: bool,
    left_ctrl: bool,
    right_ctrl: bool,
    left_alt: bool,
    right_alt: bool,
    caps_lock: bool,
}

impl KeyboardDecoder {
    pub const fn new() -> Self {
        Self {
            extended: false,
            left_shift: false,
            right_shift: false,
            left_ctrl: false,
            right_ctrl: false,
            left_alt: false,
            right_alt: false,
            caps_lock: false,
        }
    }

    pub fn feed(&mut self, scancode: u8) -> Option<KeyEvent> {
        if scancode == 0xE0 {
            self.extended = true;
            return None;
        }

        if self.extended {
            self.extended = false;
            return self.decode_extended(scancode);
        }

        let released = scancode & 0x80 != 0;
        let code = scancode & 0x7f;

        match code {
            0x2A => {
                self.left_shift = !released;
                return None;
            }
            0x36 => {
                self.right_shift = !released;
                return None;
            }
            0x1D => {
                self.left_ctrl = !released;
                return None;
            }
            0x38 => {
                self.left_alt = !released;
                return None;
            }
            0x3A => {
                if !released {
                    self.caps_lock = !self.caps_lock;
                }
                return None;
            }
            _ => {}
        }

        if released {
            return None;
        }

        if let Some(letter) = decode_letter(code) {
            let upper = self.shift_active() ^ self.caps_lock;
            let character = if upper {
                letter.to_ascii_uppercase()
            } else {
                letter
            };
            return Some(self.classify_char(character));
        }

        if let Some((normal, shifted)) = decode_symbol_pair(code) {
            let ch = if self.shift_active() { shifted } else { normal };
            return Some(self.classify_char(ch));
        }

        match code {
            0x1C => Some(KeyEvent::Enter),
            0x0E => {
                if self.ctrl_active() {
                    Some(KeyEvent::WordDeleteLeft)
                } else {
                    Some(KeyEvent::Backspace)
                }
            }
            0x0F => Some(KeyEvent::Tab),
            0x01 => Some(KeyEvent::Escape),
            0x3B..=0x44 => Some(KeyEvent::Function(code - 0x3A)),
            0x57 => Some(KeyEvent::Function(11)),
            0x58 => Some(KeyEvent::Function(12)),
            _ => Some(KeyEvent::Unknown(scancode)),
        }
    }

    fn decode_extended(&mut self, scancode: u8) -> Option<KeyEvent> {
        let released = scancode & 0x80 != 0;
        let code = scancode & 0x7f;

        match code {
            0x1D => {
                self.right_ctrl = !released;
                return None;
            }
            0x38 => {
                self.right_alt = !released;
                return None;
            }
            _ => {}
        }

        if released {
            return None;
        }

        match code {
            0x48 => Some(KeyEvent::ArrowUp),
            0x50 => Some(KeyEvent::ArrowDown),
            0x4B => {
                if self.ctrl_active() {
                    Some(KeyEvent::WordLeft)
                } else {
                    Some(KeyEvent::ArrowLeft)
                }
            }
            0x4D => {
                if self.ctrl_active() {
                    Some(KeyEvent::WordRight)
                } else {
                    Some(KeyEvent::ArrowRight)
                }
            }
            0x52 => Some(KeyEvent::Insert),
            0x53 => {
                if self.ctrl_active() {
                    Some(KeyEvent::WordDeleteRight)
                } else {
                    Some(KeyEvent::Delete)
                }
            }
            0x47 => Some(KeyEvent::Home),
            0x4F => Some(KeyEvent::End),
            0x49 => Some(KeyEvent::PageUp),
            0x51 => Some(KeyEvent::PageDown),
            _ => Some(KeyEvent::Unknown(scancode)),
        }
    }

    fn shift_active(&self) -> bool {
        self.left_shift || self.right_shift
    }

    fn ctrl_active(&self) -> bool {
        self.left_ctrl || self.right_ctrl
    }

    fn alt_active(&self) -> bool {
        self.left_alt || self.right_alt
    }

    fn classify_char(&self, character: char) -> KeyEvent {
        match (self.ctrl_active(), self.alt_active()) {
            (true, true) => KeyEvent::CtrlAlt(character),
            (true, false) => KeyEvent::Ctrl(character),
            (false, true) => KeyEvent::Alt(character),
            (false, false) => KeyEvent::Char(character),
        }
    }
}

fn decode_letter(code: u8) -> Option<char> {
    match code {
        0x1E => Some('a'),
        0x30 => Some('b'),
        0x2E => Some('c'),
        0x20 => Some('d'),
        0x12 => Some('e'),
        0x21 => Some('f'),
        0x22 => Some('g'),
        0x23 => Some('h'),
        0x17 => Some('i'),
        0x24 => Some('j'),
        0x25 => Some('k'),
        0x26 => Some('l'),
        0x32 => Some('m'),
        0x31 => Some('n'),
        0x18 => Some('o'),
        0x19 => Some('p'),
        0x10 => Some('q'),
        0x13 => Some('r'),
        0x1F => Some('s'),
        0x14 => Some('t'),
        0x16 => Some('u'),
        0x2F => Some('v'),
        0x11 => Some('w'),
        0x2D => Some('x'),
        0x15 => Some('y'),
        0x2C => Some('z'),
        _ => None,
    }
}

fn decode_symbol_pair(code: u8) -> Option<(char, char)> {
    match code {
        0x02 => Some(('1', '!')),
        0x03 => Some(('2', '@')),
        0x04 => Some(('3', '#')),
        0x05 => Some(('4', '$')),
        0x06 => Some(('5', '%')),
        0x07 => Some(('6', '^')),
        0x08 => Some(('7', '&')),
        0x09 => Some(('8', '*')),
        0x0A => Some(('9', '(')),
        0x0B => Some(('0', ')')),
        0x39 => Some((' ', ' ')),
        0x0C => Some(('-', '_')),
        0x0D => Some(('=', '+')),
        0x1A => Some(('[', '{')),
        0x1B => Some((']', '}')),
        0x27 => Some((';', ':')),
        0x28 => Some(('\'', '"')),
        0x29 => Some(('`', '~')),
        0x2B => Some(('\\', '|')),
        0x33 => Some((',', '<')),
        0x34 => Some(('.', '>')),
        0x35 => Some(('/', '?')),
        _ => None,
    }
}
