use std::collections::HashMap;

use crate::stream::{Stream, ReadSeek};
use crate::primitives::Primitives;
use crate::primitives::Name;
use crate::error::Error;
use crate::utils::{is_whitespace};

pub struct Lexer<T> {
    stream: Stream<T>,

    current_char: Option<u8>,

    // The PDFs might have "glued" commands with other commands, operands or
    // literals, e.g. "q1". The known_commands is a dictionary of the valid
    // commands and their prefixes. The prefixes are built the following way:
    // if there a command that is a prefix of the other valid command or
    // literal (e.g. 'f' and 'false') the following prefixes must be included,
    // 'fa', 'fal', 'fals'. The prefixes are not needed, if the command has no
    // other commands or literals as a prefix. The knowCommands is optional.
    known_commands: Option<HashMap<&'static [u8], &'static [u8]>>,

    _hex_string_num_warn: i32,

    begin_inline_image_pos: Option<u64>,
}

macro_rules! special_chars {
    ($ch:expr) => { SPECIAL_CHARS[$ch as usize] }
}

macro_rules! ch {
    ($f: expr) => { $f.ok_or_else(|| Error::LexicalError)};
}

// A '1' in this array means the character is white space. A '1' or
// '2' means the character ends a name or command.
// prettier-ignore
const SPECIAL_CHARS: [u8; 256] = [
  1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0, // 0x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 1x
  1, 0, 0, 0, 0, 2, 0, 0, 2, 2, 0, 0, 0, 0, 0, 2, // 2x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, // 3x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 4x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 0, // 5x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 6x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 0, // 7x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 8x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 9x
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // ax
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // bx
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // cx
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // dx
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // ex
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0  // fx
];

fn to_hex_digit(ch: u8) -> Option<u8> {
    if ch >= /* '0' = */ 0x30 && ch /* '9' = */ <= 0x39 {
        Some(ch & 0x0f)  // Get number 0 ~ 9
    } else if (ch >= /* 'A' = */ 0x41 && ch <= /* 'F' = */ 0x46) ||
       (ch >= /* 'a' = */ 0x61 && ch <= /* 'f' = */ 0x66) {
        Some((ch & 0x0f) + 9)
    } else {
        None
    }
}

impl<T: ReadSeek> Lexer<T> {
    pub fn new(stream: Stream<T>, /*known_commands: Option<HashMap<&'a [u8], &'a [u8]>> */) -> Lexer<T> {
        let mut l = Lexer {
            stream: stream,
            known_commands: None, //known_commands,
            current_char: None,
            _hex_string_num_warn: -1,
            begin_inline_image_pos: None,
        };
        l.next_char();
        l
    }

    fn next_char(&mut self) -> Option<u8> {
        self.current_char = self.stream.get_byte();
        self.current_char
    }

    fn peek_char(&mut self) -> Option<u8> {
        self.stream.peek_byte()
    }

    fn next_char_or_error(&mut self) -> Result<u8, Error> {
        self.next_char().ok_or_else(|| Error::LexicalError)
    }

    pub fn get_number(&mut self) -> Result<Primitives, Error> {
        let mut ch = ch!(self.current_char)?;

        let mut e_notation = false;
        let mut divide_by = 0; // Different from 0 if it's a floating point value.
        let mut sign = 0;

        if ch == /* '-' = */ 0x2d {
            sign = -1;
            ch = ch!(self.next_char())?;
            if ch == 0x2d {
                // Ignore double negative (This is consistent with Adobe Reader).
                ch = ch!(self.next_char())?;
            }
        } else if ch == /* '+' = */ 0x2b {
            sign = 1;
            ch = ch!(self.next_char())?;
        }
        if ch == /* LF = */ 0x0a || ch == /* CR = */ 0x0d {
            // Ignore line-breaks
            loop {
                ch = ch!(self.next_char())?;
                if ch == 0x0a || ch == 0x0d { break; }
            }
        }
        if ch == /* '.' = */ 0x2e {
            divide_by = 10;
            ch = ch!(self.next_char())?;
        }
        if ch < /* '0' = */ 0x30 || ch > /* '9' = */ 0x39 {
            if divide_by == 10 && sign == 0 && is_whitespace(ch) {
                // This is consistent with Adobe Reader (fiexes issue9252.pdf).
                eprintln!("Lexer.get_number - treating a signal decimal point as zero.");
                return Ok(Primitives::Real(0.0));
            }

            eprintln!("Invalid number: {}", ch as char);
            return Err(Error::LexicalError);
        }

        let sign: f64 = if sign >= 0 { 1.0 } else { -1.0 };
        let mut base_value: f64 = (ch - 0x30) as f64; // '0'
        let mut power_value: f64 = 0.0;
        let mut power_value_sign: f64 = 1.0;

        while let Some(mut ch) = self.next_char() {
            if ch >= /* '0 = */ 0x30 && ch <= /* '9 = */ 0x39 {
                let current_digit = (ch - 0x30) as f64;
                if e_notation {
                    // We are after 'e' or 'E'.
                    power_value = power_value * 10.0 + current_digit;
                } else {
                    if divide_by != 0 {
                        // We are afetr a point.
                        divide_by *= 10;
                    }
                    base_value = base_value * 10.0 + current_digit;
                }
            } else if ch == /* '.' = */ 0x2e {
                if divide_by == 0 {
                    divide_by = 1;
                } else {
                    // A number can have only one dot.
                    break;
                }
            } else if ch == /* '-' = */ 0x2d {
                // Ignore minus signs in the middle of the numbers to match Adobe's behavior
                eprintln!("Badly formatted number: minus sign in the middle");
            } else if ch == /* 'E' = */ 0x45 || ch == /* 'e' = */ 0x65 {
                // 'E' can be either a scientific notation or the beginning of a new operator.
                ch = ch!(self.peek_char())?;
                if ch == /* '+' = */ 0x2b || ch == /* '-' = */ 0x2d {
                    power_value_sign = if ch == 0x2d { -1.0 } else { 1.0 };
                    self.next_char(); // Consume the sign character.
                } else if ch < /* '0' = */ 0x30 || ch > /* '9' = */ 0x39 {
                    // The 'E' must be the beginning of a new operator.
                    break
                }
                e_notation = true;
            } else {
                // The lost character doesn't belogn to us.
                break;
            }
        }
        if divide_by != 0 {
            base_value /= divide_by as f64;
        }

        if e_notation {
            base_value *= 10_f64.powf(power_value_sign * power_value);
        }

        let num = sign * base_value;
        if num.fract() == 0.0 {
            return Ok(Primitives::Int(num as i64));
        } else {
            return Ok(Primitives::Real(num));
        }
    }

    pub fn get_string(&mut self) -> Result<Primitives, Error> {
        let mut num_paren = 1;
        let mut done = false;

        let mut str_buf: Vec<u8> = vec![];

        let mut ch = ch!(self.next_char())?;
        loop {
            let mut char_buffered = false;
            if ch == 0x28 { // '('
                num_paren += 1;
                str_buf.push('(' as u8);
            } else if ch == 0x29 { // ')'
                num_paren -= 1;
                if num_paren == 0 {
                    self.next_char(); // consume strings ')'
                    done = true;
                } else {
                    str_buf.push(')' as u8);
                }
            } else if ch == 0x5c { // '\\'
                ch = ch!(self.next_char())?;
                if ch == 0x6e {
                    str_buf.push('\n' as u8);
                } else if ch == 0x72 {
                    str_buf.push('\r' as u8);
                } else if ch == 0x74 {
                    str_buf.push('\t' as u8);
                } else if ch == 0x62 {
                    str_buf.push('\x08' as u8);
                } else if ch == 0x66 {
                    str_buf.push('\x0c' as u8);
                } else if ch == 0x5c {
                    str_buf.push('\\' as u8);
                } else if ch == 0x28 {
                    str_buf.push('(' as u8);
                } else if ch == 0x29 {
                    str_buf.push(')' as u8);
                } else if ch >= /* '0' = */  0x30  && ch <= /* '7' = */ 0x37 {
                    // character code (\ddd)
                    // \053 , \53 = plus sign (+)
                    let mut x = ch & 0x0f;
                    ch = ch!(self.next_char())?;
                    char_buffered = true;
                    if ch >= /* '0' */ 0x30 && ch <= /* '7' */ 0x37 {
                        x = (x << 3) + (ch & 0x0f);
                        ch = ch!(self.next_char())?;
                        if ch >= 0x30 && ch <= 0x37 { 
                            char_buffered = false;
                            x = (x << 3) + (ch & 0x0f);
                        }
                    }
                    str_buf.push(x);
                } else if ch == 0x0d {
                    if ch!(self.peek_char())? == /* LF = */ 0x0a {
                        self.next_char();
                    }
                } else if ch == 0x0a {}
                else {
                    str_buf.push(ch);
                }
            } else {
                str_buf.push(ch);
            }

            if done  { break; }

            if !char_buffered {
                ch = ch!(self.next_char())?;
            }
        }
        // str_buf :: vec<u8>
        return Ok(Primitives::Str(str_buf));
    }

    pub fn get_name(&mut self) -> Result<Primitives, Error> {
        let mut previous_ch: u8;
        let mut str_buf: Vec<u8> = vec![];
        while let Some(mut ch) = self.next_char() {
            if special_chars![ch] != 0 {
                break;
            }
            if ch == /* '#' = */ 0x23 { // /Name#20Green => Name Green
                ch = ch!(self.next_char())?;
                if special_chars![ch] != 0 {
                    eprintln!("Lexer_get_name: \"NUMBER SIGN (#) should be followed by a hexadecimal number.");
                    str_buf.push('#' as u8);
                    break;
                }
                let x = to_hex_digit(ch);
                if x.is_some() {
                    previous_ch = ch;
                    ch = ch!(self.next_char())?;
                    let x2 = to_hex_digit(ch);
                    if x2.is_none() {
                        eprintln!("Lexer_get_name: Illeagal digit {} in hexdecimal number.", ch as char);
                        str_buf.extend(&['#' as u8, previous_ch]);
                        if special_chars![ch] != 0 {
                            break;
                        }
                        str_buf.push(ch);

                        continue;
                    }
                    str_buf.push((x.unwrap() << 4) | x2.unwrap());
                } else {
                    str_buf.extend(&['#' as u8, ch]);
                }
            } else {
                str_buf.push(ch);
            }
        }
        if str_buf.len() > 127 {
            eprintln!("Name token is longer than allowed by the spec: {}", str_buf.len());
        }
        return Ok(Primitives::name(str_buf));
    }

    fn hex_string_warn(&mut self, ch: u8) {
        let max_hex_string_num_warn = 5;
        if self._hex_string_num_warn == max_hex_string_num_warn {
            self._hex_string_num_warn += 1;
            eprintln!("get_hex_string - ignoring additional invalid characters.");
            return;
        }
        if self._hex_string_num_warn > max_hex_string_num_warn {
          // Limit the number of warning messages printed for a `this.getHexString`
          // invocation, since corrupt PDF documents may otherwise spam the console
          // enough to affect general performance negatively.
          return;
        }
        eprintln!("get_hex_string - ignoring invalid character: {}", ch);
    }

    pub fn get_hex_string(&mut self) -> Result<Primitives, Error> {
        let mut str_buf = Vec::new();
        let mut ch = ch!(self.current_char)?;
        let mut is_first_hex = true;
        let mut first_digit: Option<u16> = None;
        let mut second_digit: Option<u16> = None;
        self._hex_string_num_warn = 0;

        loop {
            if ch == /* '>' = */ 0x3e {
                ch!(self.next_char())?;
                break;
            } else if special_chars![ch] == 1 {
                ch = ch!(self.next_char())?;
                continue;
            } else {
                if is_first_hex {
                    first_digit = to_hex_digit(ch).map(|digit| digit as u16);
                    if first_digit.is_none() {
                        self.hex_string_warn(ch);
                        ch = ch!(self.next_char())?;
                        continue;
                    }
                } else {
                    second_digit = to_hex_digit(ch).map(|digit|digit as u16);
                    if second_digit.is_none() {
                        self.hex_string_warn(ch);
                        ch = ch!(self.next_char())?;
                        continue;
                    }
                    str_buf.push(first_digit.unwrap() << 4 | second_digit.unwrap());
                }
                is_first_hex = !is_first_hex;
                ch = ch!(self.next_char())?;
            }
        }
        return Ok(Primitives::HexStr(str_buf));
    }

    pub fn get_obj(&mut self) -> Result<Primitives, Error> {
        // Skip whitespace and comments.
        let mut comment = false;
        let mut ch = self.current_char;
        loop {
            if ch.is_none() {
                return Ok(Primitives::EOF);
            }

            let raw_ch = ch.unwrap();
            if comment {
                if raw_ch == /* LF = */ 0x0a || raw_ch == /* CR = */ 0x0d {
                    comment = false;
                }
            } else if raw_ch == /* '%' = */ 0x25 {
                comment = true;
            } else if special_chars![raw_ch] != 1 {
                break;
            }
            ch = self.next_char();
        }

        let mut ch = ch.unwrap();
        if ch >=  /* '0' = */ 0x30 && ch <= /* '9' = */ 0x39 || ch == /* '+' = */ 0x2b || ch == /* '-' = */ 0x2d || ch == /* '.' = */ 0x2e {
            return self.get_number();
        } else if ch == /* '(' = */ 0x28 {
            return self.get_string();
        } else if ch == /* '/' = */ 0x2f {
            return self.get_name();
        } else if ch == /* '[' = */ 0x5b {
            ch!(self.next_char())?;
            return Ok(Primitives::cmd("["));
        } else if ch == /* ']' = */ 0x5d {
            ch!(self.next_char())?;
            return Ok(Primitives::cmd("]"));
        } else if ch == /* '<' = */ 0x3c {
            ch = ch!(self.next_char())?;
            if ch == 0x3c {
                // dict puncuation
                self.next_char();
                return Ok(Primitives::cmd("<<"));
            }
            return self.get_hex_string();
        } else if ch == /* '>' = */ 0x3e {
            ch = ch!(self.next_char())?;
            if ch == 0x3e {
                self.next_char();
                return Ok(Primitives::cmd(">>"));
            }
            return Ok(Primitives::cmd(">"));
        } else if ch == /* '{' = */ 0x7b {
            self.next_char();
            return Ok(Primitives::cmd("{"));
        } else if ch == /* '}' = */ 0x7d {
            self.next_char();
            return Ok(Primitives::cmd("}"));
        } else if ch == /* ')' = */ 0x29 {
            // Consume the current character in order to avoid permanently hanging
            // the worker thread if `Lexer.getObject` is called from within a loop
            // containing try-catch statements, since we would otherwise attempt
            // to parse the *same* character over and over (fixes issue8061.pdf).
            self.next_char();
            panic!("Illegal character: {}", ch);
        }

        // Start reading a command.
        let mut str = vec![ch];
        let mut known_command_found = self.known_commands.as_ref().map_or(false, |map| map.contains_key(&str[..]));


        while let Some(mut ch) = self.next_char() {
            if special_chars![ch] != 0 { break; }

            let mut possible_command = str.clone();
            possible_command.push(ch);
            if known_command_found && !self.known_commands.as_ref().unwrap().contains_key(&possible_command[..]) {
                break;
            }
            
            if str.len() == 128 {
                eprintln!("Command token too long: {}", str.len());
                return Err(Error::LexicalError)
            }
            str = possible_command;
            known_command_found = self.known_commands.as_ref().map_or(false, |map| map.contains_key(&str[..]));
        }
        if str == b"BI" {
            // Keep track of the current stream position, since it's needed in order to correctly cache inline images;
            // see `Parser.makeInlineImage`.
            self.begin_inline_image_pos = Some(self.stream.pos());
        }
        return Ok(Primitives::Cmd(str));
    }

    pub fn peek_obj(&mut self) -> Result<Primitives, Error> {
        let stream_pos = self.stream.pos();
        let current_char = self.current_char;
        let begin_inline_image_pos = self.begin_inline_image_pos;

        let mut next_obj = self.get_obj();

        self.stream.set_pos(stream_pos);
        self.current_char = current_char;
        self.begin_inline_image_pos = begin_inline_image_pos;

        next_obj
    }

    pub fn skip_to_next_line(&mut self) {
        let mut ch = self.current_char;
        loop {
            ch = {
                if let Some(ch) = ch {
                    if ch == /* CR = */ 0x0d {
                        if let Some(ch) = self.next_char() {
                            if ch == /* LF = */ 0x0a {
                                self.next_char();
                            }
                        }
                        break;
                    } else if ch == /* LF = */ 0x0a {
                        self.next_char();
                        break;
                    }
                }
                self.next_char()
            }
        }
    }
}
