// Chekcs if ch is one of the following characters: SPACE, TAB, CR or LF,
pub fn is_whitespace(ch: u8) -> bool {
    ch == 0x20 || ch == 0x09 || ch == 0x0d || ch == 0x0a
}

