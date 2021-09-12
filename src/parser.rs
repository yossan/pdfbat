use std::collections::HashMap;

use crate::stream::{Stream, ReadSeek};
use crate::lexer::{Lexer};
use crate::primitives::{Name, Dictionary, Ref, Cmd}
use crate::primitives::Primitives;
use crate::error::Error;

macro_rules! primitive {
    ($token:expr) => { $token.ok_or_else(|| Error::ParserError) };
}

pub struct Parser<T> {
    lexer: Lexer<T>,
    allow_streams: bool,
    buf1: Option<Primitives>,
    buf2: Option<Primitives>,
}

impl<T: ReadSeek> Parser<T> {
    pub fn new(lexer: Lexer<T>, allow_streams: bool) -> Self {
        let mut p = Parser {
            lexer: lexer,
            allow_streams: allow_streams,
            buf1: None,
            buf2: None,
        };
        p.refill();
        p
    }

    pub fn buf1(&self) -> Option<Primitives> {
        self.buf1.clone()
    }

    pub fn buf2(&self) -> Option<Primitives> {
        self.buf2.clone()
    }

    fn refill(&mut self) {
        self.buf1 = self.lexer.get_obj().ok();
        self.buf2 = self.lexer.get_obj().ok();
    }

    fn shift(&mut self) -> Option<Primitives> {
        let gone = self.buf1.take();
        if self.buf2 == Primitives::cmd("ID") {
            self.buf1 = self.buf2.take();
            self.buf2 = None;
        } else {
            self.buf1 = self.buf2.take();
            self.buf2 = self.lexer.get_obj().ok();
        }
        gone
    }

    /*
    fn trye_shift() {
    }
    */

    pub fn get_obj(&mut self) -> Result<Primitives, Error> {
        let buf1 = primitive!(self.shift())?;

        if let Cmd(ref cmd) = buf1 {
            /*
            if cmd == b"BI" { // inline image
                returns self.make_inline_image();
            }*/
            if cmd == b"[" { // array
                let mut array = Vec::new();
                while self.buf1 != Primitives::cmd("]") && self.buf1 != EOF {
                    array.push(self.get_obj()?);
                }
                if self.buf1 == EOF {
                    eprintln!("End of file inside array");
                    return Ok(Array(array));
                }
                self.shift();
                return Ok(Array(array));
            } else if cmd == b"<<" {
                let mut dict = HashMap::<Name, Primitives>::new();

                let mut i = 0;
                while self.buf1 != Primitives::cmd(">>") && self.buf1 != EOF {
                    if primitive!(self.buf1.as_ref())?.is_name() {
                        if let Primitives::Name(name) = primitive!(self.buf1.take())? {
                            self.shift();
                            if self.buf1 == EOF {
                                break;
                            }
                            dict.insert(Name(name.0), self.get_obj()?);
                        }
                    } else {
                        eprintln!("Malformed dictionary: key must be a name object");
                        self.shift();
                        continue;
                    }
                }

                if self.buf1 == EOF {
                    eprintln!("End of file inside dictionary");
                    return Ok(Dict(dict));
                }

                // Stream objects are not allowed inside content streams or object streams.
                if self.buf2.as_ref().unwrap().is_cmd("stream") {
                    if self.allow_streams {
                        return self.make_stream(dict)
                    } else {
                        return Ok(Dict(dict));
                    }
                }
                self.shift();
                return Ok(Dict(dict));
            } else {
                return Ok(buf1);
            }
        }

        if let Some(num1) = buf1.get_integer() {
            if primitive!(self.buf1.as_ref())?.is_integer() && primitive!(self.buf2.as_ref())?.is_cmd("R") {

                if let Int(num2) = primitive!(self.buf1.take())? {
                    self.shift();
                    self.shift();
                    return Ok(Ref(num1 as u32, num2 as u32));
                }
            }
            return Ok(Int(num1));
        }

        if buf1.is_string() {
            // if (cipher_transform) {
            //     // cipherTransform.decrypt_string(buf1)
            // }
            return Ok(buf1);
        }

        // simple object
        Ok(buf1)
    }


    fn make_stream(&self, dict: HashMap<Name, Primitives>) -> Result<Primitives, Error> {
        Err(Error::ParserError)
    }

}



