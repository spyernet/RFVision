//! Small authenticated, same-origin UI gateway. No radio mutation endpoints.
use rfv_ipc::{Record, DISCOVER, SAMPLE};
use std::{
    io::{self, BufRead, Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};
const PAGE: &str = include_str!("ui.html");
struct Request {
    method: String,
    path: String,
    authorization: String,
    body: String,
}
fn read_request(stream: &mut TcpStream) -> io::Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    stream.set_write_timeout(Some(Duration::from_secs(15)))?;
    let mut reader = io::BufReader::new(stream);
    let mut header = Vec::new();
    loop {
        let mut line = Vec::new();
        let n = reader
            .by_ref()
            .take(8193 - header.len() as u64)
            .read_until(b'\n', &mut line)?;
        if n == 0 || header.len() + n > 8192 {
            return Err(io::Error::other("HTTP header limit/truncation"));
        }
        let end = line == b"\r\n";
        header.extend(line);
        if end {
            break;
        }
    }
    let h = std::str::from_utf8(&header).map_err(io::Error::other)?;
    let mut lines = h.split("\r\n");
    let parts: Vec<_> = lines.next().unwrap_or("").split_whitespace().collect();
    if parts.len() != 3 || parts[2] != "HTTP/1.1" || !["GET", "POST"].contains(&parts[0]) {
        return Err(io::Error::other("unsupported HTTP request"));
    }
    let mut fields = std::collections::BTreeMap::new();
    for line in lines.filter(|s| !s.is_empty()) {
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| io::Error::other("bad header"))?;
        if fields.insert(k.to_ascii_lowercase(), v.trim()).is_some() {
            return Err(io::Error::other("duplicate header"));
        }
    }
    if fields.contains_key("transfer-encoding") {
        return Err(io::Error::other("chunked requests not supported"));
    }
    let length = fields
        .get("content-length")
        .unwrap_or(&"0")
        .parse::<usize>()
        .map_err(io::Error::other)?;
    if length > 4 * 1024 * 1024 {
        return Err(io::Error::other("4 MiB body limit"));
    }
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    Ok(Request {
        method: parts[0].into(),
        path: parts[1].into(),
        authorization: fields.get("authorization").unwrap_or(&"").to_string(),
        body: String::from_utf8(body).map_err(io::Error::other)?,
    })
}
fn respond(stream: &mut TcpStream, status: &str, mime: &str, body: &str) -> io::Result<()> {
    write!(stream,"HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'self'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; connect-src 'self'; object-src 'none'; base-uri 'none'\r\nConnection: close\r\n\r\n{body}",body.len())
}
fn api(
    req: &Request,
    socket: &str,
    interface: &str,
    history: &mut rfv_core::coverage::History,
) -> Result<Vec<Record>, String> {
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/report") => super::request(socket, DISCOVER, None).map_err(|e| e.to_string()),
        ("GET" | "POST", "/api/sample") => {
            let position = if req.body.is_empty() {
                None
            } else {
                let p: Vec<_> = req.body.trim_end_matches('\n').split('\t').collect();
                if p.len() != 5 {
                    return Err("position: reference, label, x_m, y_m, source".into());
                }
                let pos = rfv_core::coverage::Position {
                    reference: p[0].into(),
                    label: p[1].into(),
                    x_m: p[2].parse().map_err(|_| "bad X")?,
                    y_m: p[3].parse().map_err(|_| "bad Y")?,
                    source: p[4].into(),
                };
                pos.validate()?;
                Some(pos)
            };
            let mut rows =
                super::request(socket, SAMPLE, Some(interface)).map_err(|e| e.to_string())?;
            // Revalidate at browser boundary; service data is not blindly trusted.
            for r in rows
                .iter()
                .filter(|r| r.get("kind").map(String::as_str) == Some("measurement"))
            {
                rfv_ipc::measurement::validate(r)?;
            }
            for o in rfv_core::coverage::from_batch(&rows, position.clone()) {
                history.push(o)?;
            }
            if let Some(p) = position {
                rows.push(rfv_ipc::record(&[
                    ("kind", "position"),
                    ("interface", interface),
                    ("reference", &p.reference),
                    ("label", &p.label),
                    ("x_m", &p.x_m.to_string()),
                    ("y_m", &p.y_m.to_string()),
                    ("source", &p.source),
                ]));
            }
            Ok(rows)
        }
        ("GET", "/api/coverage") => {
            Ok(history.summarize(rfv_hal::now_ns().parse().map_err(|_| "invalid clock")?))
        }
        ("POST", "/api/budget") => rfv_core::link_budget::parse(&req.body)
            .and_then(|i| rfv_core::link_budget::calculate(&i)),
        ("POST", "/api/range") => rfv_core::range::assess(&req.body),
        ("POST", "/api/csi") => {
            rfv_core::csi_capture::Capture::parse(&req.body).and_then(|c| c.analyze())
        }
        _ => Err("unknown API route".into()),
    }
}
pub fn serve(socket: &str, interface: &str, port: u16) -> io::Result<()> {
    if !rfv_hal::valid_interface(interface) || port == 0 {
        return Err(io::Error::other(
            "valid interface and nonzero port required",
        ));
    }
    let token = std::env::var("RFV_UI_TOKEN").map_err(|_| {
        io::Error::other("set RFV_UI_TOKEN to a random secret of at least 32 characters")
    })?;
    if token.len() < 32
        || token.len() > 256
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
    {
        return Err(io::Error::other(
            "token needs 32..256 alphanumeric/-/_ characters",
        ));
    }
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let mut history = rfv_core::coverage::History::default();
    eprintln!("RFVision dashboard listening on 0.0.0.0:{port}; API authentication required. Use HTTPS proxy or trusted network; no radio configuration endpoints.");
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let result = read_request(&mut stream);
        let _ = match result {
            Ok(req) if req.method == "GET" && req.path == "/" => {
                respond(&mut stream, "200 OK", "text/html; charset=utf-8", PAGE)
            }
            Ok(req) if req.authorization != format!("Bearer {token}") => respond(
                &mut stream,
                "401 Unauthorized",
                "application/json",
                "[{\"kind\":\"error\",\"message\":\"Enter the dashboard token to connect\"}]",
            ),
            Ok(req) => match api(&req, socket, interface, &mut history) {
                Ok(rows) => respond(
                    &mut stream,
                    "200 OK",
                    "application/json",
                    &rfv_ipc::json(&rows),
                ),
                Err(e) => respond(
                    &mut stream,
                    "400 Bad Request",
                    "application/json",
                    &rfv_ipc::json(&[rfv_ipc::error("UI_REQUEST_FAILED", &e)]),
                ),
            },
            Err(e) => respond(
                &mut stream,
                "400 Bad Request",
                "application/json",
                &rfv_ipc::json(&[rfv_ipc::error("INVALID_HTTP", &e.to_string())]),
            ),
        };
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_routes() {
        let req = Request {
            method: "POST".into(),
            path: "/api/range".into(),
            authorization: String::new(),
            body: "AP\tmesh\tunknown\tunknown\tuser registration".into(),
        };
        assert_eq!(
            api(&req, "/not-used", "wlan0", &mut Default::default()).unwrap()[0]["id"],
            "AP"
        );
    }
}
