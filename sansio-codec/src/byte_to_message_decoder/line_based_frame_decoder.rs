use super::MessageDecoder;

use bytes::BytesMut;

/// Delimiter with different terminator type \n` or `\r\n`
#[derive(Default, PartialEq, Eq)]
pub enum TerminatorType {
    /// Delimiter with \n` or `\r\n`
    #[default]
    BOTH,

    /// Delimiter with `\n` only
    NEWLINE,

    /// Delimiter with `\r\n` only
    CarriageNewline,
}

/// A line based frame decoder with [TerminatorType] as delimiter
#[derive(Default)]
pub struct LineBasedFrameDecoder {
    max_length: usize,
    strip_delimiter: bool,
    terminator_type: TerminatorType,

    discarding: bool,
    discarded_bytes: usize,
}

impl LineBasedFrameDecoder {
    /// Creates a new LineBasedFrameDecoder
    pub fn new(max_length: usize, strip_delimiter: bool, terminator_type: TerminatorType) -> Self {
        Self {
            max_length,
            strip_delimiter,
            terminator_type,
            ..Default::default()
        }
    }

    fn find_end_of_line(&mut self, buf: &BytesMut) -> Option<usize> {
        let mut i = 0usize;
        while i < self.max_length && i < buf.len() {
            let b = buf[i];
            if (b == b'\n' && self.terminator_type != TerminatorType::CarriageNewline)
                || (self.terminator_type != TerminatorType::NEWLINE
                    && b == b'\r'
                    && i + 1 < buf.len()
                    && buf[i + 1] == b'\n')
            {
                return Some(i);
            }
            i += 1;
        }

        None
    }
}

impl MessageDecoder for LineBasedFrameDecoder {
    fn decode(&mut self, buf: &mut BytesMut) -> Result<Option<BytesMut>, std::io::Error> {
        let eol = self.find_end_of_line(buf);
        let mut offset = 0;
        if !self.discarding {
            if let Some(eol) = eol {
                offset += eol;
                let delim_length = if buf[offset] == b'\r' { 2 } else { 1 };
                if eol > self.max_length {
                    return Err(std::io::Error::other(format!(
                        "frame length {} exceeds max {}",
                        eol, self.max_length
                    )));
                }

                let frame = if self.strip_delimiter {
                    let frame = buf.split_to(eol);
                    let _ = buf.split_to(delim_length);
                    frame
                } else {
                    buf.split_to(eol + delim_length)
                };

                Ok(Some(frame))
            } else {
                let len = buf.len();
                if len > self.max_length {
                    self.discarded_bytes = len;
                    let _ = buf.split_to(len);
                    self.discarding = true;
                    Err(std::io::Error::other(format!("over {}", len)))
                } else {
                    Ok(None)
                }
            }
        } else {
            if let Some(eol) = eol {
                offset += eol;
                let delim_length = if buf[offset] == b'\r' { 2 } else { 1 };
                let _ = buf.split_to(eol + delim_length);
                self.discarded_bytes = 0;
                self.discarding = false;
            } else {
                self.discarded_bytes = buf.len();
                let _ = buf.split_to(buf.len());
            }

            Ok(None)
        }
    }
}
