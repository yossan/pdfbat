use crate::document::PdfDocument;
use crate::stream::{Stream, ReadSeek};
use crate::utils::is_whitespace;

use std::fs::File;

impl PdfDocument {
    pub fn load_data(data: Vec<u8>) {
    }

    pub fn load_file(file: File) {
        let stream = Stream::from_file(&file);
        let start_xref = {
            let mut reader = Reader { stream: stream };
            reader.parse_startxref()
        };

        // let xref = Xref::parse(stream, start_xref);
    }
}

struct Reader<T> {
    stream: Stream<T>,
}

impl<T: ReadSeek+std::fmt::Debug> Reader<T> {
    fn parse(&mut self) -> String {
        // 1. header
        self.stream.reset();
        // 2. startxref
        let startxref = self.parse_startxref();

        // let mut xref = XRef::parse(self.stream.clone(), startxref);
        // return format!("{:?}", xref);
        // dbg!(&xref);

        /*
        let catalog = Catalog::new(&xref);
        if let Some(version) = catalog.version() {
            dbg!(version);
        }
        */

        String::from("")
    }

    fn parse_startxref(&mut self) -> u64 {
        let mut start_xref = 0;
        /*
        if self.linearization {
        } else
        */

        // Find `startxref`.
        let start_xref_length = "startxref".len() as i64;
        let step: i64 = 1024;
        let mut found = false;
        let mut pos = self.stream.end() as i64;

        while !found && pos > 0 {
            pos -= step - start_xref_length;
            if pos < 0 {
                pos = 0;
            }
            self.stream.set_pos(pos as u64);
            found = self.find("startxref".as_bytes(), step as u64);
        }

        if found {
            self.stream.skip("startxref".len() as i64);
        }

        let mut ch;
        loop {
            ch = self.stream.get_byte().unwrap();
            if !is_whitespace(ch) {
                break;
            }
        }

        let mut str = String::new();
        while ch >= /* Space */ 0x20 && ch <= /* '9' = */ 0x39 {
            str.push(ch as char);
            ch = self.stream.get_byte().unwrap();
        }
        start_xref = str.parse::<u64>().unwrap_or(0);
        start_xref
    }

    fn find(&mut self, signature: &[u8], limit: u64) -> bool {
        let signature_length = signature.len();
        let scan_bytes = self.stream.peek_bytes(limit as usize).expect("stream can not peek_byte");
        let scan_length = scan_bytes.len() - signature_length;
        if scan_length <= 0 {
            return false;
        }
        let mut pos: usize = 0;
        while pos <= scan_length {
            let mut j: usize = 0;
            while j < signature_length && scan_bytes[pos + j] == signature[j] {
                j += 1;
            }
            if j >= signature_length {
                // `signature` found.
                self.stream.seek_pos(pos as i64);
                return true;
            }
            pos += 1;
        }
        return false;
    }
}

#[cfg(test)]
mod tests {
    use std::io::{self};
    use std::fs::File;
    use std::fs::{self};

    use crate::stream::Stream;
    use super::Reader;

    // const EXAMPLES_DIR: &str = "tests/examples";
    const EXAMPLES: [(&str, u64); 5] = [
        ("tests/examples/dummy.pdf", 12787),
        ("tests/examples/sample.pdf", 2714),
        ("tests/examples/PDF_sample.pdf", 60009),
        ("tests/examples/140514041111253731pdf1.pdf", 14443),
        ("tests/examples/7a79c35f7ce0704dec63be82440c8182.pdf", 16595),
    ];

    #[test]
    fn read_start_xref() -> io::Result<()> {
        for entry in EXAMPLES {
            let file = File::open(entry.0)?;
            let stream = Stream::from_file(&file);
            let mut reader = Reader { stream: stream };
            let start_xref = reader.parse_startxref();
            assert_eq!(start_xref, entry.1);
        }

        Ok(())
    }
}
