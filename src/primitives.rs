//use super::xref::XRef;
use crate::stream::Stream;

use std::collections::HashMap;
use std::fmt::{self, Debug};

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Name(pub Vec<u8>);
impl Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Name").field(&String::from_utf8(self.0.clone()).unwrap()).finish()
    }
}

pub struct Dictionary(HashMap<Name, Primitives>);

pub struct Ref(u32, u32);

pub struct Cmd(Vec<u8>);

#[derive(PartialEq, Clone)]
pub enum Primitives {
    Null,
    Int(i64),
    Str(Vec<u8>),
    HexStr(Vec<u8>),
    Real(f64),
    Name(Name),
    Array(Vec<Primitives>),
    Dict(Dictionary),
    //Stream(Stream),
    Ref(Ref),
    Cmd(Cmd),
    EOF,
}

impl Eq for Primitives {}

impl PartialEq<Primitives> for Option<Primitives> {
    fn eq(&self, other: &Primitives) -> bool {
        self.as_ref().map_or(false, |me| *me == *other)
    }
}

impl Primitives {

    pub fn cmd(cmd: &str) -> Primitives {
        Primitives::Cmd(cmd.as_bytes().to_vec())
    }

    pub fn name(bytes: Vec<u8>) -> Primitives {
        Primitives::Name(Name(bytes))
    }

    pub fn is_cmd(&self, cmd: &str) -> bool {
        if let Primitives::Cmd(bytes) = self {
            if bytes == cmd.as_bytes() {
                return true;
            }
        }
        false
    }

    pub fn is_dict(&self) -> bool {
        match(self) {
            Primitives::Dict => true,
            _ => false
        }
    }

    pub fn is_name(&self) -> bool {
        if let Primitives::Name(_) = self {
            true
        } else {
            false
        }
    }

    pub fn is_integer(&self) -> bool {
        if let Primitives::Int(num) = self {
            true
        } else {
            false
        }
    }

    pub fn is_string(&self) -> bool {
        if let Primitives::Str(_) = self {
            true
        } else {
            false
        }
    }

    pub fn get_dict(self) -> Option<Dictionary> {
        if let Primitives::Dict(dictionary) = self {
            return Some(dictionary)
        }
        None
    }

    pub fn get_cmd(&self) -> Option<&Vec<u8>> {
        if let Primitives::Cmd(bytes) = self {
            return Some(&bytes);
        }
        None
    }

    pub fn get_integer(&self) -> Option<i64> {
        if let Primitives::Int(num) = self {
            return Some(*num as i64);
        }
        None
    }

    pub fn get_str(&self) -> Option<&Vec<u8>> {
        if let Primitives::Str(bytes) = self {
            return Some(&bytes);
        }
        None
    }

    pub fn get_hexstr(&self) -> Option<&Vec<u16>> {
        if let Primitives::HexStr(bytes) = self {
            return Some(&bytes);
        }
        None
    }
}


