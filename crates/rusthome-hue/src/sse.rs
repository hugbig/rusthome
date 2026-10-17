//! Minimal Server-Sent Events parser for the Hue event stream.

/// Accumulates raw bytes (a chunk may split a UTF-8 character, e.g. in
/// « Pièce ») and returns the `data:` payload of each complete event.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
}

impl SseParser {
    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buf.extend(chunk.iter().filter(|b| **b != b'\r'));
        let mut out = Vec::new();
        while let Some(end) = self.buf.windows(2).position(|w| w == b"\n\n") {
            let event: Vec<u8> = self.buf.drain(..end + 2).collect();
            let event = String::from_utf8_lossy(&event);
            let data: Vec<&str> = event
                .lines()
                .filter_map(|l| l.strip_prefix("data:").map(str::trim_start))
                .collect();
            if !data.is_empty() {
                out.push(data.join("\n"));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_events_across_chunks() {
        let mut p = SseParser::default();
        assert_eq!(p.push(b": hi\n\nid: 1:0\ndata: [{\"a\"").len(), 0);
        assert_eq!(
            p.push(b":1}]\n\nid: 2:0\r\ndata: [2]\r\n\r\n"),
            vec!["[{\"a\":1}]", "[2]"]
        );
    }

    #[test]
    fn utf8_character_split_between_chunks() {
        let event = "data: {\"name\":\"Pièce\"}\n\n".as_bytes();
        let cut = event.iter().position(|b| *b >= 0x80).unwrap() + 1; // inside « è »
        let mut p = SseParser::default();
        assert_eq!(p.push(&event[..cut]).len(), 0);
        assert_eq!(p.push(&event[cut..]), vec!["{\"name\":\"Pièce\"}"]);
    }

    #[test]
    fn ignores_comments_and_keepalives() {
        let mut p = SseParser::default();
        assert_eq!(p.push(b": keep-alive\n\n").len(), 0);
    }
}
