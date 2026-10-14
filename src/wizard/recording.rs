//! Private, bounded, versioned measurement recordings using existing RFV framing.
use rfv_ipc::{Frame, Record, RESPONSE, SAMPLE};
use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
    os::unix::fs::OpenOptionsExt,
    path::Path,
};
const MAGIC: &[u8] = b"RFVREC1\n";
const LIMIT: u64 = 64 * 1024 * 1024;
pub fn record(
    socket: &str,
    interface: &str,
    path: &str,
    count: usize,
    delay: u64,
    position: Option<rfv_core::coverage::Position>,
) -> io::Result<()> {
    if !(1..=10000).contains(&count) || !(1000..=3600000).contains(&delay) {
        return Err(io::Error::other("count 1..10000; delay 1000..3600000 ms"));
    }
    if let Some(p) = &position {
        p.validate().map_err(io::Error::other)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(MAGIC)?;
    let mut size = MAGIC.len() as u64;
    for i in 0..count {
        let mut rows = super::request(socket, SAMPLE, Some(interface))?;
        if let Some(p) = &position {
            rows.push(rfv_ipc::record(&[
                ("kind", "position"),
                ("interface", interface),
                ("reference", &p.reference),
                ("label", &p.label),
                ("x_m", &p.x_m.to_string()),
                ("y_m", &p.y_m.to_string()),
                ("source", &p.source),
                ("basis", "user_recorded_position"),
            ]));
        }
        let f = Frame {
            kind: SAMPLE,
            flags: RESPONSE,
            id: i as u64 + 1,
            records: rows,
        };
        let mut buf = Vec::new();
        f.write(&mut buf)?;
        if size + buf.len() as u64 > LIMIT {
            return Err(io::Error::other(
                "64 MiB recording limit; complete earlier frames retained",
            ));
        }
        file.write_all(&buf)?;
        file.flush()?;
        size += buf.len() as u64;
        if i + 1 < count {
            std::thread::sleep(std::time::Duration::from_millis(delay));
        }
    }
    file.sync_all()
}
pub fn replay(path: &Path) -> io::Result<Vec<Vec<Record>>> {
    let mut data = Vec::new();
    std::fs::File::open(path)?
        .take(LIMIT + 1)
        .read_to_end(&mut data)?;
    if data.len() as u64 > LIMIT || !data.starts_with(MAGIC) {
        return Err(io::Error::other("invalid/oversized RFVREC1 recording"));
    }
    let mut reader = std::io::Cursor::new(&data[MAGIC.len()..]);
    let mut batches = Vec::new();
    let mut last = 0;
    while reader.position() < (data.len() - MAGIC.len()) as u64 {
        let f = Frame::read(&mut reader)?;
        if f.kind != SAMPLE || f.flags != RESPONSE || f.id <= last || batches.len() >= 10000 {
            return Err(io::Error::other("invalid recording sequence"));
        }
        last = f.id;
        let mut rows = f.records;
        rows.retain(|r| {
            !matches!(
                r.get("kind").map(String::as_str),
                Some("processing" | "recommendation")
            )
        });
        batches.push(rfv_core::process(rows).map_err(io::Error::other)?);
    }
    if batches.is_empty() {
        return Err(io::Error::other("empty recording"));
    }
    Ok(batches)
}
pub fn coverage(batches: &[Vec<Record>]) -> io::Result<Vec<Record>> {
    let mut history = rfv_core::coverage::History::default();
    for rows in batches {
        for o in rfv_core::coverage::from_batch(rows, None) {
            let mut o = o;
            if let Some(p) = rows.iter().find(|r| {
                r.get("kind").map(String::as_str) == Some("position")
                    && r.get("interface") == o.measurement.get("interface")
            }) {
                let get = |k: &str| {
                    p.get(k)
                        .cloned()
                        .ok_or_else(|| io::Error::other(format!("missing position {k}")))
                };
                o.position = Some(rfv_core::coverage::Position {
                    reference: get("reference")?,
                    label: get("label")?,
                    x_m: get("x_m")?.parse().map_err(io::Error::other)?,
                    y_m: get("y_m")?.parse().map_err(io::Error::other)?,
                    source: get("source")?,
                });
            }
            history.push(o).map_err(io::Error::other)?;
        }
    }
    Ok(history.summarize(rfv_hal::now_ns().parse().map_err(io::Error::other)?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_and_truncated_recording() {
        let p = std::env::temp_dir().join(format!("rfv-rec-{}", std::process::id()));
        std::fs::write(&p, b"RFVREC1\nRFV").unwrap();
        assert!(replay(&p).is_err());
        std::fs::remove_file(p).unwrap();
    }
}
