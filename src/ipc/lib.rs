//! Independent RFV/1 framing; no driver implementation or third-party crates.
#![forbid(unsafe_code)]
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
pub mod transport;

pub type Record = BTreeMap<String, String>;
pub const MAX_FRAME: usize = 1_048_576;
pub const HELLO: u16 = 1;
pub const DISCOVER: u16 = 2;
pub const SAMPLE: u16 = 3;
pub const ERROR: u16 = 255;
pub const RESPONSE: u16 = 1;

pub fn record(fields: &[(&str, &str)]) -> Record {
    fields
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}
pub fn error(code: &str, message: &str) -> Record {
    record(&[("kind", "error"), ("code", code), ("message", message)])
}
pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: u16,
    pub flags: u16,
    pub id: u64,
    pub records: Vec<Record>,
}
impl Frame {
    pub fn request(kind: u16, id: u64, records: Vec<Record>) -> Self {
        Self {
            kind,
            flags: 0,
            id,
            records,
        }
    }
    pub fn reply(&self, records: Vec<Record>) -> Self {
        Self {
            kind: self.kind,
            flags: RESPONSE,
            id: self.id,
            records,
        }
    }
    pub fn failure(&self, code: &str, message: &str) -> Self {
        Self {
            kind: ERROR,
            flags: RESPONSE,
            id: self.id,
            records: vec![error(code, message)],
        }
    }
    pub fn write(&self, mut out: impl Write) -> io::Result<()> {
        if self.flags > 1 || self.records.len() > 4096 {
            return Err(invalid("invalid frame"));
        }
        let mut payload = Vec::new();
        payload.extend_from_slice(&(self.records.len() as u32).to_be_bytes());
        for rec in &self.records {
            if rec.len() > 64 {
                return Err(invalid("too many fields"));
            }
            payload.extend_from_slice(&(rec.len() as u16).to_be_bytes());
            for (key, value) in rec {
                if key.is_empty() || key.len() > 64 || value.len() > 65536 {
                    return Err(invalid("field limit"));
                }
                payload.extend_from_slice(&(key.len() as u16).to_be_bytes());
                payload.extend_from_slice(&(value.len() as u32).to_be_bytes());
                payload.extend_from_slice(key.as_bytes());
                payload.extend_from_slice(value.as_bytes());
                if payload.len() > MAX_FRAME {
                    return Err(invalid("frame limit"));
                }
            }
        }
        let mut header = Vec::with_capacity(24);
        header.extend_from_slice(b"RFV1");
        header.extend_from_slice(&1u16.to_be_bytes());
        header.extend_from_slice(&0u16.to_be_bytes());
        header.extend_from_slice(&self.kind.to_be_bytes());
        header.extend_from_slice(&self.flags.to_be_bytes());
        header.extend_from_slice(&self.id.to_be_bytes());
        header.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.write_all(&header)?;
        out.write_all(&payload)
    }
    pub fn read(mut input: impl Read) -> io::Result<Self> {
        let mut h = [0u8; 24];
        input.read_exact(&mut h)?;
        if &h[..4] != b"RFV1" || h[4..8] != [0, 1, 0, 0] {
            return Err(invalid("unsupported protocol version"));
        }
        let kind = u16::from_be_bytes([h[8], h[9]]);
        let flags = u16::from_be_bytes([h[10], h[11]]);
        if flags > 1 {
            return Err(invalid("unknown flags"));
        }
        let id = u64::from_be_bytes(h[12..20].try_into().unwrap());
        let len = u32::from_be_bytes(h[20..24].try_into().unwrap()) as usize;
        if !(4..=MAX_FRAME).contains(&len) {
            return Err(invalid("frame size"));
        }
        let mut buf = vec![0; len];
        input.read_exact(&mut buf)?;
        let mut c = Cursor { b: &buf, at: 0 };
        let count = c.u32()? as usize;
        if count > 4096 {
            return Err(invalid("record limit"));
        }
        let mut records = Vec::with_capacity(count);
        for _ in 0..count {
            let fields = c.u16()?;
            if fields > 64 {
                return Err(invalid("field count"));
            }
            let mut rec = Record::new();
            for _ in 0..fields {
                let kl = c.u16()? as usize;
                let vl = c.u32()? as usize;
                if kl == 0 || kl > 64 || vl > 65536 {
                    return Err(invalid("field length"));
                }
                let key = c.text(kl)?;
                let value = c.text(vl)?;
                if rec.insert(key, value).is_some() {
                    return Err(invalid("duplicate key"));
                }
            }
            records.push(rec);
        }
        if c.at != buf.len() {
            return Err(invalid("trailing payload"));
        }
        Ok(Self {
            kind,
            flags,
            id,
            records,
        })
    }
}
struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}
impl Cursor<'_> {
    fn take(&mut self, n: usize) -> io::Result<&[u8]> {
        let end = self.at.checked_add(n).ok_or_else(|| invalid("overflow"))?;
        let s = self
            .b
            .get(self.at..end)
            .ok_or_else(|| invalid("truncated payload"))?;
        self.at = end;
        Ok(s)
    }
    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_be_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn text(&mut self, n: usize) -> io::Result<String> {
        String::from_utf8(self.take(n)?.to_vec()).map_err(|_| invalid("invalid UTF-8"))
    }
}
pub fn json(records: &[Record]) -> String {
    let rows: Vec<_> = records
        .iter()
        .map(|r| {
            let fields: Vec<_> = r
                .iter()
                .map(|(k, v)| format!("{}:{}", quote(k), quote(v)))
                .collect();
            format!("{{{}}}", fields.join(","))
        })
        .collect();
    format!("[{}]", rows.join(","))
}
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if c < ' ' => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip() {
        let f = Frame::request(
            SAMPLE,
            42,
            vec![record(&[("interface", "wlan0"), ("unicode", "λ\n")])],
        );
        let mut bytes = Vec::new();
        f.write(&mut bytes).unwrap();
        assert_eq!(Frame::read(&bytes[..]).unwrap(), f);
        for i in 0..bytes.len() {
            assert!(Frame::read(&bytes[..i]).is_err());
        }
    }
    #[test]
    fn reject_limits_and_version() {
        let mut b = Vec::new();
        Frame::request(1, 0, vec![]).write(&mut b).unwrap();
        b[5] = 2;
        assert!(Frame::read(&b[..]).is_err());
        b[5] = 1;
        b[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(Frame::read(&b[..]).is_err());
        assert!(
            Frame::request(1, 0, vec![record(&[("k", &"x".repeat(65537))])])
                .write(Vec::new())
                .is_err()
        );
    }
    #[test]
    fn duplicate_and_trailing_rejected() {
        let mut b = Vec::new();
        Frame::request(1, 1, vec![record(&[("a", "b")])])
            .write(&mut b)
            .unwrap();
        let pair = b[30..].to_vec();
        b[29] = 2;
        b.extend(pair);
        let len = (b.len() - 24) as u32;
        b[20..24].copy_from_slice(&len.to_be_bytes());
        assert!(Frame::read(&b[..]).is_err());
    }
    #[test]
    fn json_escape() {
        assert_eq!(
            json(&[record(&[("x", "\n\"\\")])]),
            "[{\"x\":\"\\u000a\\\"\\\\\"}]"
        );
    }
}
