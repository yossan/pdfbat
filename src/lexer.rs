pub enum Token {
    Int(i64),
    Real(f64),
    Str(Vec<u8>),
    Name(Vec<u8>),
    Cmd(Vec<u8>),
}

pub struct Lexer<'a> {
    stream: Stream<'a>,
    current_char: Option<u8>,
}

impl<'a> Lexer<'a> {
    pub fn new(stream: Stream<'a>, /*known_commands: Option<HashMap<&'a [u8], &'a [u8]>> */) -> Lexer {
        let mut lexer = Lexer {
            stream: stream,
            current_char: None,
        };

        lexer.next_char();
        lexer
    }

    fn next_char(&mut self) -> Option<u8> {
        self.current_char = self.stream.get_byte();
        self.current_char
    }
}

/*
#[cfg(test)]
mod tests {
    #[test]
    fn xxxx() {
    }
}
*/
