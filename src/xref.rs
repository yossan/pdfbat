use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::stream::{Stream, ReadSeek};
use crate::primitives::{Name, Dictionary, Ref, Cmd};
use crate::primitives::Primitives;
use crate::error::Error;

use anyhow::{Result, Context, bail};

macro_rules! get_integer {
    ($obj:expr) => { $obj.get_integer().ok_or_else(|| Error::ParserError) };
}
macro_rules! get_str {
    ($obj:expr) => { $obj.get_str().ok_or_else(|| Error::ParserError) };
}

macro_rules! get_cmd {
    ($obj:expr) => { $obj.get_cmd().ok_or_else(|| Error::ParserError) };
}

pub struct XRef<T> {
    stream: Stream<T>,
    trailer_dict: Dictionary,
    // startxref_queue: Vec<u64>,
    password: Option<String>,
    // table_state: Option<TableState>,
    entries: Vec<Entry>,
}

impl<T: ReadSeek> XRef<T> {

    pub fn parse(stream: Stream<T>, startxref: u64, password: Option<String>) -> Result<XRef<T>> {
        let reader = XRefReader::new(&stream, startxref);
        let entries = reader.read_xref(parser);

        let trailer_dict = stream.get_obj()
            .is_cmd("trailer")
            .then(|obj| obj.get_dict())
            .context("failed to fetch trailer dictionary")?
            .context("trailer obj must be Dict")?;

        // TODO: Encrypt
        /*
        if (trailer_dict.get("Encrypt").is_none()) {
        }
        */
        /*
        if (is_dict(root) && root.has("Page")) {
            self.root = root;
        }
        */

        Ok(XRef {
            stream: stream,
            trailer_dict: trailer_dict,
            entries: entries,
            password: password,
        })
    }
}

struct XRefReader<'a, T> {
    stream: &'a Stream<T>,
    startxref: u64,
}

impl<'a, T: ReadSeek> XRefReader<'a, T> {

    fn new(stream: &'a Stream<T>, startxref: u64) -> XRefReader<'a, T> {
        XRefReader {
            stream: stream,
            startxref: startxref,
        }
    }

    fn read_xref(&mut self) -> Result<Vec<Entry>> {

        self.stream.set_pos(self.startxref);

        let lexer = Lexer::new(stream);
        let mut parser = Parser::new(lexer, true);
        let obj = parser.get_obj().context("Failed to get xref obj.")?;

        obj.is_cmd("xref")
            .then(|| {self.process_xreftable(parser)})
            .context("obj must be a xref.")?
    }

    fn process_xreftable(&mut self, mut parser: Parser<T>) -> Result<Entry> {

        self.read_xreftable(&mut parser)?;


        // Read trailer dictionary, e.g.
        // trailer
        //    << /Size 22
        //      /Root 20R
        //      /Info 10R
        //      /ID [ <81b14aafa313db63dbd6f981e49f94f4> ]
        //    >>
        // The parser goes through the entire stream << ... >> and provides
        // a getter interface for the key-value table
        let dict = parser.get_obj()?;

        return Ok(dict);
    }

    fn read_xreftable(&mut self, parser: &mut Parser<T>) -> Result<Primitives> {
        // Example of cross-reference table:
        // xref
        // 0 1                    <-- subsection header (first obj #, obj count)
        // 0000000000 65535 f     <-- actual object (offset, generation #, f/n)
        // 23 2                   <-- subsection header ... and so on ...
        // 0000025518 00002 n
        // 0000025635 00000 n
        // trailer
        // ...

         let mut table_state = TableState::from_parser(&parser);

         let mut parser = parser;

        // Outer loop is over subsection headers.

        let trailer_obj = loop {
            if table_state.first_entry_num.is_none() && table_state.entry_count.is_none() {
                let obj = parser.get_obj()?;
                if obj.is_cmd("trailer") {
                    return obj;
                }
                table_state.set_first_entry_num(get_integer!(obj));
                let next = parser.get_obj()?;
                table_state.set_entry_count(get_integer!(next));
            }

            let mut first = table_state.first_entry_num.unwrap();
            let count = table_state.entry_count.unwrap();


            // Inner loop is over objects themselves
            for i in table_state.entry_num .. count {
                table_state.stream_pos = parser.lexer().stream().pos();
                table_state.entry_num = i;
                table_state.parser_buf1 = parser.buf1();
                table_state.parser_buf2 = parser.buf2();

                let offset = get_integer!(parser.get_obj()?);
                let gen = get_integer!(parser.get_obj()?);
                let ty = parser.get_obj()?;

                let (free, uncompressed) = {
                    if ty.is_cmd("f") {
                        (true, false)
                    } else if ty.is_cmd("n") {
                        (false, true)
                    } else {
                        (false, false)
                    }
                };

                let entry = Entry {
                    offset: offset,
                    gen: gen,
                    ty: ty,
                    free: free,
                    uncompressed: uncompressed,
                };

                // The first xref table entry, i.e. obj 0, should be free. Attempting
                // to adjust an incorrect first obj # (fixes issue 3248 and 7229).
                if i == 0 && entry.free && first == 1 {
                    first = 0;
                }

                if self.entries.len() > i as usize {
                    self.entries[(i + first) as usize] = entry;
                } else {
                    self.entries.push(entry);
                }
            }

            table_state.entry_num = 0;
            table_state.stream_pos = parser.lexer().stream().pos();
            table_state.parser_buf1 = parser.buf1();
            table_state.parser_buf2 = parser.buf2();
            table_state.first_entry_num = None;
            table_state.entry_count = None;
        }

        // Sanity check: as per spec, first ojbect must be free
        if self.entries.len() > 1 && !self.entries[0].free {
            return Err(Error::ParserError);
        }

        return trailer_obj.get_dict();
    }
}

struct TableState {
    entry_num: i64,
    stream_pos: u64,
    parser_buf1: Option<Primitives>,
    parser_buf2: Option<Primitives>,
    first_entry_num: Option<i64>,
    entry_count: Option<i64>,
}

impl TableState {
    fn from_parser<T: ReadSeek>(parser: &Parser<T>) -> Self {
        TableState {
            entry_num: 0,
            stream_pos: parser.lexer().stream().pos(),
            parser_buf1: parser.buf1(),
            parser_buf2: parser.buf2(),
            first_entry_num: None,
            entry_count: None,
        }
    }
    fn set_first_entry_num(&mut self, num: i64) {
        self.first_entry_num = Some(num);
    }
    fn set_entry_count(&mut self, count: i64) {
        self.entry_count = Some(count);
    }
}

#[derive(Debug)]
struct Entry { 
    offset: i64, 
    gen: i64, 
    ty: Primitives, 
    free: bool, 
    uncompressed: bool 
}


