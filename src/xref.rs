use crate::lexer::Lexer;
use crate::parser::Parser;
use crate::stream::{Stream, ReadSeek};
use crate::primitives::*;
use crate::error::Error;

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
    startxref_queue: Vec<u64>,
    password: Option<String>,
    table_state: Option<TableState>,
    entries: Vec<Entry>,
}


impl<T: ReadSeek> XRef<T> {
    pub fn new(stream: Stream<T>, startxref: u64, password: Option<String>) -> XRef<T> {
        XRef {
            stream: stream,
            startxref_queue: vec![startxref],
            password: password,
            table_state: None,
            entries: Vec::new(),
        }
    }
    pub fn set_startxref(&mut self, startxref: u64) {
        self.startxref_queue.push(startxref);
    }

    pub fn parse(&mut self) {
        let trait_dict = self.read_xref();

        // TODO: Encrypt

        let root = trait_dict.get("Root");
        /*
        if (is_dict(root) && root.has("Page")) {
            self.root = root;
        }
        */
    }

    fn read_xref(&mut self) {
        let mut startxref_parsed_cache = vec![0_u64; self.startxref_queue.len()];

        while self.startxref_queue.len() > 0 {
            let startxref = self.startxref_queue[0];
            if startxref_parsed_cache.contains(&startxref) {
                println!("read_xref - skipping XRef table since it was already parsed.");
                self.startxref_queue.remove(0);
                continue;
            }
            startxref_parsed_cache.push(startxref);

            self.stream.set_pos(startxref + self.stream.start());

            let lexer = Lexer::new(self.stream.clone());
            let mut parser = Parser::new(lexer, true);
            let obj = parser.get_obj().unwrap();
            if obj.is_cmd("xref") {
                // Parse end-of-file XRef
                let dict = self.process_xreftable(parser);
                if self.top_dict.is_none() {
                    self.top_dict = dict;
                }
            }
        }
    }

    fn process_xreftable(&mut self, mut parser: Parser<T>) -> Result<Primitives, Error> {
        self.table_state = Some(TableState {
            entry_num: 0,
            stream_pos: parser.lexer().stream().pos(),
            parser_buf1: None,
            parser_buf2: None,
            first_entry_num: None,
            entry_count: None,
        });

        let obj = self.read_xreftable(&mut parser)?;

        if !obj.is_cmd("trailer") {
            return Err(Error::ParserError);
        }

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

        self.table_state = None;
        return Ok(dict);
    }

    fn read_xreftable(&mut self, parser: &mut Parser<T>) -> Result<Primitives, Error> {
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
        let mut obj;

        loop {
            obj = parser.get_obj()?;
            if table_state.first_entry_num.is_none() && table_state.entry_count.is_none() {
                if obj.is_cmd("trailer") {
                    break;
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

                dbg!(&entry);
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

        return Ok(obj);
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


