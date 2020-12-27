use std::{collections::BTreeMap, convert::TryFrom};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
    Rgb(u8, u8, u8),
    Indexed(u8),
    Default,
}

impl Color {
    #[allow(clippy::many_single_char_names)]
    pub fn rgb(&self) -> (u8, u8, u8) {
        match self {
            Color::Black => (0x2e, 0x34, 0x36),
            Color::Red => (0xcc, 0x00, 0x00),
            Color::Green => (0x4e, 0x9a, 0x06),
            Color::Yellow => (0xc4, 0xa0, 0x00),
            Color::Blue => (0x34, 0x65, 0xa4),
            Color::Magenta => (0x75, 0x50, 0x7b),
            Color::Cyan => (0x06, 0x98, 0x9a),
            Color::White => (0xd3, 0xd7, 0xcf),
            Color::BrightBlack => (0x55, 0x57, 0x53),
            Color::BrightRed => (0xef, 0x29, 0x29),
            Color::BrightGreen => (0x8a, 0xe2, 0x34),
            Color::BrightYellow => (0xfc, 0xe9, 0x4f),
            Color::BrightBlue => (0x72, 0x9f, 0xcf),
            Color::BrightMagenta => (0xad, 0x7f, 0xa8),
            Color::BrightCyan => (0x34, 0xe2, 0xe2),
            Color::BrightWhite => (0xee, 0xee, 0xec),
            Color::Rgb(r, g, b) => (*r, *g, *b),
            Color::Indexed(x) => match x {
                0 => (0x2e, 0x34, 0x36),
                1 => (0xcc, 0x00, 0x00),
                2 => (0x4e, 0x9a, 0x06),
                3 => (0xc4, 0xa0, 0x00),
                4 => (0x34, 0x65, 0xa4),
                5 => (0x75, 0x50, 0x7b),
                6 => (0x06, 0x98, 0x9a),
                7 => (0xd3, 0xd7, 0xcf),
                8 => (0x55, 0x57, 0x53),
                9 => (0xef, 0x29, 0x29),
                10 => (0x8a, 0xe2, 0x34),
                11 => (0xfc, 0xe9, 0x4f),
                12 => (0x72, 0x9f, 0xcf),
                13 => (0xad, 0x7f, 0xa8),
                14 => (0x34, 0xe2, 0xe2),
                15 => (0xee, 0xee, 0xec),
                16..=231 => {
                    let r: u8 = (x - 16) / 36;
                    let g: u8 = ((x - 16) % 36) / 6;
                    let b: u8 = (x - 16) % 6;
                    let z: u8 = 255 / 6;
                    (r * z, g * z, b * z)
                }
                232..=255 => {
                    let val: u8 = (x - 232) * (255 / 24);
                    (val, val, val)
                }
            },
            Color::Default => (0xcb, 0xbf, 0xbf),
        }
    }
}

#[test]
fn color() {
    assert_eq!(Color::Indexed(226).rgb(), (0xd2, 0xd2, 0));
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Intensity {
    Normal,
    Bold,
    Faint,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct FrameCell {
    /// Text for this segment.
    pub ch: Option<char>,
    /// Text foreground color.
    pub fg: Color,
    /// Text intensity.
    pub intensity: Intensity,
}

impl FrameCell {
    /// Check if the cell attributes are equal.
    fn attr_eq(&self, other: &FrameCell) -> bool {
        self.fg == other.fg && self.intensity == other.intensity
    }
}

impl Default for FrameCell {
    fn default() -> Self {
        FrameCell {
            ch: None,
            fg: Color::Default,
            intensity: Intensity::Normal,
        }
    }
}

/// Mode for clearing line.
///
/// Relative to cursor.
#[derive(Debug)]
pub enum LineClearMode {
    /// Clear right of cursor.
    Right,
    /// Clear left of cursor.
    Left,
    /// Clear entire line.
    All,
}

/// Mode for clearing terminal.
///
/// Relative to cursor.
#[derive(Debug)]
pub enum ClearMode {
    /// Clear below cursor.
    Below,
    /// Clear above cursor.
    Above,
    /// Clear entire terminal.
    All,
    /// Clear 'saved' lines (scrollback).
    Saved,
}

/// An SVG symbol.
///
/// A symbol is an SVG construct, in this case we use it to represent a
/// continuous region of `FrameCell`s that share the same attributes, on the
/// same line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol {
    pub x: usize,
    pub y: usize,
    pub fg: Color,
    pub intensity: Intensity,
    pub text: String,
}

impl Symbol {
    pub fn escaped_text(&self) -> String {
        // '<' is escaped by xmlwriter, but not the others.
        const ESCAPES: &[(&str, &str)] = &[
            ("&", "&amp;"), // must be first
            (">", "&lt;"),
            ("\"", "&quot;"),
            ("'", "&apos;"),
        ];
        let mut text = self.text.clone();
        for (find, replace) in ESCAPES {
            text = text.replace(find, replace);
        }
        text
    }
}

impl Default for Symbol {
    fn default() -> Self {
        Symbol {
            x: 0,
            y: 0,
            fg: Color::Default,
            intensity: Intensity::Normal,
            text: String::new(),
        }
    }
}

/// A asciicast frame.
pub struct Frame {
    /// x cursor position, zero index.
    pub x: usize,
    /// y cursor position, zero index.
    pub y: usize,
    /// asciicast width (number of columns).
    x_max: usize,
    /// asciicast height (number of rows).
    y_max: usize,
    /// Current foreground.
    fg: Color,
    /// Current text intensity.
    intensity: Intensity,
    /// Frame buffer.
    buf: Vec<FrameCell>,
}

impl Frame {
    /// Create a new frame.
    pub fn new(width: usize, height: usize) -> Frame {
        Frame {
            x: 0,
            y: 0,
            x_max: width,
            y_max: height,
            fg: Color::Default,
            intensity: Intensity::Normal,
            buf: vec![FrameCell::default(); width * height],
        }
    }

    fn move_up(&mut self, ammount: usize) {
        log::trace!("move_up: ammount={}, self.y={}", ammount, self.y);
        self.y -= ammount;
    }

    fn move_down(&mut self, ammount: usize) {
        log::trace!("move_down: ammount={}, self.y={}", ammount, self.y);
        self.y += ammount;
        assert!(self.y_max > self.y, "{} > {}", self.y_max, self.y);
    }

    fn increment_cursor(&mut self) {
        self.x += 1;
    }

    fn increment_line(&mut self) {
        log::trace!("increment_line: self.y={}", self.y);
        self.y += 1;
        // frame rollover
        if self.y == self.y_max {
            self.buf.drain(0..self.x_max);
            for _ in 0..self.x_max {
                self.buf.push(FrameCell::default());
            }
            self.y -= 1;
        }
        assert_ne!(self.y, self.y_max);
    }

    /// Insert symbols for the frame into a multimap.
    pub fn insert_symbols(&self, map: &mut BTreeMap<Symbol, Vec<usize>>, frame_number: usize) {
        let mut insert = |symbol: Symbol| {
            if let Some(v) = map.get_mut(&symbol) {
                v.push(frame_number);
            } else {
                map.insert(symbol, vec![frame_number]);
            }
        };

        for row in 0..self.y_max {
            let mut symbol: Symbol = Symbol::default();
            let mut previous: FrameCell = FrameCell::default();

            for column in 0..self.x_max {
                let idx: usize = row * self.x_max + column;
                let current: FrameCell = self.buf[idx];

                if let Some(ch) = current.ch {
                    // left strip spaces
                    if ch == ' ' && symbol.text.is_empty() {
                        continue;
                    }
                    if symbol.text.is_empty() {
                        symbol.x = column;
                        symbol.y = row;
                        symbol.fg = current.fg;
                        symbol.intensity = current.intensity;
                        symbol.text.push(ch);
                    } else if current.attr_eq(&previous) {
                        debug_assert!(!symbol.text.is_empty());
                        symbol.text.push(ch);
                    } else {
                        log::trace!(
                            "Ending symbol; previous does not match. symbol={}",
                            symbol.text
                        );
                        insert(symbol.clone());
                        if ch == ' ' {
                            symbol.text = String::new();
                        } else {
                            symbol.text = String::from(ch);
                        }
                        symbol.x = column;
                        symbol.y = row;
                        symbol.fg = current.fg;
                        symbol.intensity = current.intensity;
                    }
                } else if !symbol.text.is_empty() {
                    log::trace!("Ending symbol; unused cell. symbol={}", symbol.text);
                    insert(symbol.clone());
                    symbol.text = String::new();
                }

                previous = current;
            }

            if !symbol.text.is_empty() {
                log::trace!("Ending symbol; end of row. symbol={}", symbol.text);
                insert(symbol);
            }
        }
    }

    fn clear_terminal(&mut self, mode: ClearMode) {
        log::trace!("clearing terminal mode={:?}", mode);
        match mode {
            ClearMode::Below => {
                todo!()
            }
            ClearMode::Above => {
                todo!()
            }
            ClearMode::All => {
                for c in self.buf.iter_mut() {
                    c.ch = None;
                }
            }
            ClearMode::Saved => {
                log::warn!("Ignoring clear history");
            }
        }
    }

    fn clear_line(&mut self, mode: LineClearMode) {
        log::trace!("clearing line mode={:?}", mode);
        let (from, to): (usize, usize) = match mode {
            LineClearMode::Left => {
                todo!()
            }
            LineClearMode::Right => {
                let from: usize = self.buffer_index();
                let to: usize = self.buffer_row_index(self.y + 1);
                (from, to)
            }
            LineClearMode::All => {
                let from: usize = self.buffer_row_index(self.y);
                let to: usize = self.buffer_row_index(self.y + 1);
                (from, to)
            }
        };
        assert!(from < to, "{} < {}", from, to);
        for idx in from..to {
            self.buf[idx] = FrameCell::default();
        }
    }

    fn reset_text_formats(&mut self) {
        self.intensity = Intensity::Normal;
        self.fg = Color::Default;
    }

    fn buffer_row_index(&self, y: usize) -> usize {
        y * self.x_max
    }

    fn buffer_index(&self) -> usize {
        self.buffer_row_index(self.y) + self.x
    }
}

// This trait was designed for alacritty (a rust terminal emulator).
// There is a lot of useful information contained in the source of alacritty
// for how to use this trait.
impl vte::Perform for Frame {
    /// Draw a character to the screen and update states.
    fn print(&mut self, c: char) {
        let idx = self.buffer_index();
        self.buf[idx] = FrameCell {
            ch: Some(c),
            fg: self.fg,
            intensity: self.intensity,
        };
        self.increment_cursor();
    }

    /// Execute a C0 or C1 control function.
    ///
    /// For our purposes this is just newline and carriage return.
    fn execute(&mut self, byte: u8) {
        log::trace!("execute: 0x{:02X}", byte);
        match byte {
            C0::LF | C0::VT | C0::FF => self.increment_line(),
            C0::CR => self.x = 0,
            _ => log::error!("ignoring execute: 0x{:02}", byte),
        }
    }

    /// Invoked when a final character arrives in first part of device control string.
    ///
    /// The control function should be determined from the private marker, final character, and
    /// execute with a parameter list. A handler should be selected for remaining characters in the
    /// string; the handler function should subsequently be called by `put` for every character in
    /// the control string.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn hook(&mut self, params: &vte::Params, intermediates: &[u8], ignore: bool, action: char) {
        log::warn!(
            "hook(params={:?}, intermediates={:?}, ignore={:?}, action={:?})",
            params,
            intermediates,
            ignore,
            action
        );
        todo!()
    }

    /// Pass bytes as part of a device control string to the handle chosen in `hook`.
    /// C0 controls will also be passed to the handler.
    fn put(&mut self, byte: u8) {
        log::warn!("put(byte={:?})", byte);
    }

    /// Called when a device control string is terminated.
    ///
    /// The previously selected handler should be notified that the DCS has
    /// terminated.
    fn unhook(&mut self) {
        log::warn!("unhook");
    }

    /// Dispatch an operating system command.
    ///
    /// We are not a real terminal so this just gets ignored.
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        if params.is_empty() || params[0].is_empty() {
            return;
        }
        match params[0] {
            b"0" | b"2" => log::warn!("ignoring set window title"),
            b"4" => log::warn!("ignoring set color index"),
            b"10" | b"11" | b"12" => log::warn!("ignoring get/set fg/bg/cursor color"),
            b"50" => log::warn!("ignoring set cursor style"),
            b"52" => log::warn!("ignoring set clipboard"),
            b"104" => log::warn!("ignoring reset color index"),
            b"110" => log::warn!("ignoring set fg color"),
            b"111" => log::warn!("ignoring set bg color"),
            b"112" => log::warn!("ignoring set text cursor color"),
            _ => log::error!(
                "unknown osc_dispatch(params={:?}, bell_terminated={:?})",
                params,
                bell_terminated,
            ),
        }
    }

    /// A final character has arrived for a CSI sequence
    ///
    /// The `ignore` flag indicates that either more than two intermediates arrived
    /// or the number of parameters exceeded the maximum supported length,
    /// and subsequent characters were ignored.
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        ignore: bool,
        action: char,
    ) {
        let log_unknown = || {
            log::error!(
                "unknown csi_dispatch(params={:?}, intermediates={:?}, ignore={:?}, action={:?})",
                params,
                intermediates,
                ignore,
                action
            )
        };
        let mut params_iter = params.iter();
        let mut next_param_or = |default: i64| {
            params_iter
                .next()
                .map(|param| param[0])
                .filter(|&param| param != 0)
                .unwrap_or(default)
        };

        match action {
            'A' => self.move_up(next_param_or(1) as usize),
            'B' | 'e' => self.move_down(next_param_or(1) as usize),
            'm' => {
                if params.is_empty() {
                    self.reset_text_formats();
                    return;
                }
                for p in params_iter {
                    match p {
                        [0] => self.reset_text_formats(),
                        [1] => self.intensity = Intensity::Bold,
                        [2] => self.intensity = Intensity::Faint,
                        [3] => log::warn!("ignoring italic"),
                        [4] => log::warn!("ignoring underline"),
                        [5] => log::warn!("ignoring slow blink"),
                        [6] => log::warn!("ignoring rapid blink"),
                        [30] => self.fg = Color::Black,
                        [31] => self.fg = Color::Red,
                        [32] => self.fg = Color::Green,
                        [33] => self.fg = Color::Yellow,
                        [34] => self.fg = Color::Blue,
                        [35] => self.fg = Color::Magenta,
                        [36] => self.fg = Color::Cyan,
                        [37] => self.fg = Color::White,
                        [38] => {
                            self.fg = Color::Default;
                        }
                        [38, remain @ ..] => match remain[0] {
                            2 => {
                                self.fg = Color::Rgb(
                                    u8::try_from(remain[1]).unwrap(),
                                    u8::try_from(remain[2]).unwrap(),
                                    u8::try_from(remain[3]).unwrap(),
                                )
                            }
                            5 => self.fg = Color::Indexed(u8::try_from(remain[1]).unwrap_or(15)),
                            _ => log_unknown(),
                        },
                        [39] => self.fg = Color::Default,
                        // 40..=48 => log::warn!("ignoring set background color"),
                        [90] => self.fg = Color::BrightBlack,
                        [91] => self.fg = Color::BrightRed,
                        [92] => self.fg = Color::BrightGreen,
                        [93] => self.fg = Color::BrightYellow,
                        [94] => self.fg = Color::BrightBlue,
                        [95] => self.fg = Color::BrightMagenta,
                        [96] => self.fg = Color::BrightCyan,
                        [97] => self.fg = Color::BrightWhite,
                        x => {
                            log::warn!("ignoring value: {:?}", x);
                        }
                    }
                }
            }
            'H' | 'f' => {
                let y = next_param_or(1) as usize;
                let x = next_param_or(1) as usize;
                self.x = x;
                self.y = y;
            }
            'h' => log::warn!("ignoring set mode"),
            'J' => {
                let mode = match next_param_or(0) {
                    0 => ClearMode::Below,
                    1 => ClearMode::Above,
                    2 => ClearMode::All,
                    3 => ClearMode::Saved,
                    _ => {
                        log_unknown();
                        return;
                    }
                };

                self.clear_terminal(mode);
            }
            'K' => {
                let mode = match next_param_or(0) {
                    0 => LineClearMode::Right,
                    1 => LineClearMode::Left,
                    2 => LineClearMode::All,
                    _ => {
                        log_unknown();
                        return;
                    }
                };
                self.clear_line(mode);
            }
            _ => log_unknown(),
        }
    }

    /// The final character of an escape sequence has arrived.
    ///
    /// The `ignore` flag indicates that more than two intermediates arrived and
    /// subsequent characters were ignored.
    fn esc_dispatch(&mut self, intermediates: &[u8], ignore: bool, byte: u8) {
        match byte {
            b'B' => log::warn!("ignoring configure charset"),
            b'E' => log::warn!("ignoring CRLF in esc_dispatch"),
            b'7' => log::warn!("ignoring save cursor position"),
            b'=' => log::warn!("ignoring set keypad application mode"),
            _ => log::error!(
                "unknown esc_dispatch(intermediates={:?}, ignore={:?}, byte=0x{:02X})",
                intermediates,
                ignore,
                byte
            ),
        }
    }
}

/// C0 set of 7-bit control characters (from ANSI X3.4-1977).
#[allow(non_snake_case, unused)]
pub mod C0 {
    /// Null filler, terminal should ignore this character.
    pub const NUL: u8 = 0x00;
    /// Start of Header.
    pub const SOH: u8 = 0x01;
    /// Start of Text, implied end of header.
    pub const STX: u8 = 0x02;
    /// End of Text, causes some terminal to respond with ACK or NAK.
    pub const ETX: u8 = 0x03;
    /// End of Transmission.
    pub const EOT: u8 = 0x04;
    /// Enquiry, causes terminal to send ANSWER-BACK ID.
    pub const ENQ: u8 = 0x05;
    /// Acknowledge, usually sent by terminal in response to ETX.
    pub const ACK: u8 = 0x06;
    /// Bell, triggers the bell, buzzer, or beeper on the terminal.
    pub const BEL: u8 = 0x07;
    /// Backspace, can be used to define overstruck characters.
    pub const BS: u8 = 0x08;
    /// Horizontal Tabulation, move to next predetermined position.
    pub const HT: u8 = 0x09;
    /// Linefeed, move to same position on next line (see also NL).
    pub const LF: u8 = 0x0A;
    /// Vertical Tabulation, move to next predetermined line.
    pub const VT: u8 = 0x0B;
    /// Form Feed, move to next form or page.
    pub const FF: u8 = 0x0C;
    /// Carriage Return, move to first character of current line.
    pub const CR: u8 = 0x0D;
    /// Shift Out, switch to G1 (other half of character set).
    pub const SO: u8 = 0x0E;
    /// Shift In, switch to G0 (normal half of character set).
    pub const SI: u8 = 0x0F;
    /// Data Link Escape, interpret next control character specially.
    pub const DLE: u8 = 0x10;
    /// (DC1) Terminal is allowed to resume transmitting.
    pub const XON: u8 = 0x11;
    /// Device Control 2, causes ASR-33 to activate paper-tape reader.
    pub const DC2: u8 = 0x12;
    /// (DC2) Terminal must pause and refrain from transmitting.
    pub const XOFF: u8 = 0x13;
    /// Device Control 4, causes ASR-33 to deactivate paper-tape reader.
    pub const DC4: u8 = 0x14;
    /// Negative Acknowledge, used sometimes with ETX and ACK.
    pub const NAK: u8 = 0x15;
    /// Synchronous Idle, used to maintain timing in Sync communication.
    pub const SYN: u8 = 0x16;
    /// End of Transmission block.
    pub const ETB: u8 = 0x17;
    /// Cancel (makes VT100 abort current escape sequence if any).
    pub const CAN: u8 = 0x18;
    /// End of Medium.
    pub const EM: u8 = 0x19;
    /// Substitute (VT100 uses this to display parity errors).
    pub const SUB: u8 = 0x1A;
    /// Prefix to an escape sequence.
    pub const ESC: u8 = 0x1B;
    /// File Separator.
    pub const FS: u8 = 0x1C;
    /// Group Separator.
    pub const GS: u8 = 0x1D;
    /// Record Separator (sent by VT132 in block-transfer mode).
    pub const RS: u8 = 0x1E;
    /// Unit Separator.
    pub const US: u8 = 0x1F;
    /// Delete, should be ignored by terminal.
    pub const DEL: u8 = 0x7f;
}
