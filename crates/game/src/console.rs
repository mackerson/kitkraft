//! The console channel (ADR 0010 Decision 2): one console command per HTTP/1.1 `POST` to a
//! loopback endpoint, the answer's body being the console's output.
//!
//! The client is std TCP, written here so `crates/game` needs no HTTP crate: it sends
//! `POST <path>` with the command as a `text/plain` body and `Connection: close`, and reads
//! a response with `Content-Length`, chunked encoding, or a body that ends when the
//! connection does. Only `http://` to a loopback address is accepted (`127.0.0.0/8`,
//! `::1`, `localhost`): the controller never talks to another machine. Answers are capped
//! at [`MAX_RESPONSE`] bytes; every call has a deadline.
//!
//! What the Console API plugin answers is grade D until M4.T1 (b) observes it; the
//! controller treats any `2xx` body as the command's output.

use std::io::{ErrorKind, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

/// Largest answer read, in bytes.
pub const MAX_RESPONSE: usize = 4_194_304;

/// Why a console call produced no output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleError {
    /// No complete answer before the deadline.
    Timeout,
    /// Nothing listens, or the connection broke.
    Unreachable(String),
    /// The endpoint answered with a non-2xx status.
    Status(u16, String),
    /// The answer is not HTTP the client understands.
    Malformed(String),
    /// The command was refused before it was sent (the controller's own rules).
    Refused(String),
}

impl std::fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsoleError::Timeout => write!(f, "timeout"),
            ConsoleError::Unreachable(e) => write!(f, "endpoint unreachable: {e}"),
            ConsoleError::Status(code, body) => write!(f, "HTTP {code}: {body}"),
            ConsoleError::Malformed(e) => write!(f, "malformed answer: {e}"),
            ConsoleError::Refused(e) => write!(f, "refused: {e}"),
        }
    }
}

/// Something that runs one console command and returns its output.
pub trait Console: Send {
    /// Runs `command`, waiting at most `timeout`.
    fn run(&mut self, command: &str, timeout: Duration) -> Result<String, ConsoleError>;
}

/// A parsed `http://` loopback URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// Where to connect.
    pub addr: SocketAddr,
    /// The `Host` header.
    pub host: String,
    /// The request path (`/console`).
    pub path: String,
}

impl Endpoint {
    /// Parses `http://<loopback>:<port>/<path>`.
    pub fn parse(url: &str) -> Result<Endpoint, String> {
        let rest = url
            .strip_prefix("http://")
            .ok_or_else(|| format!("`{url}`: only http:// to a loopback address is supported"))?;
        let (authority, path) = match rest.find('/') {
            Some(i) => (rest.get(..i).unwrap_or(""), rest.get(i..).unwrap_or("/")),
            None => (rest, "/"),
        };
        let (host, port) = if let Some(v6) = authority.strip_prefix('[') {
            let (h, p) = v6
                .split_once("]:")
                .ok_or_else(|| format!("`{url}`: no port"))?;
            (h, p)
        } else {
            authority
                .rsplit_once(':')
                .ok_or_else(|| format!("`{url}`: no port"))?
        };
        let port: u16 = port
            .parse()
            .map_err(|_| format!("`{url}`: bad port `{port}`"))?;
        let ip: IpAddr = if host.eq_ignore_ascii_case("localhost") {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            host.parse()
                .map_err(|_| format!("`{url}`: `{host}` is not a loopback address"))?
        };
        if !ip.is_loopback() {
            return Err(format!("`{url}`: `{host}` is not a loopback address"));
        }
        if path.chars().any(|c| c.is_control() || c == ' ') {
            return Err(format!("`{url}`: bad path"));
        }
        Ok(Endpoint {
            addr: SocketAddr::new(ip, port),
            host: authority.to_string(),
            path: path.to_string(),
        })
    }
}

/// The HTTP console client.
#[derive(Debug, Clone)]
pub struct HttpConsole {
    endpoint: Endpoint,
}

impl HttpConsole {
    /// A client for `url` (loopback only).
    pub fn new(url: &str) -> Result<HttpConsole, String> {
        Ok(HttpConsole {
            endpoint: Endpoint::parse(url)?,
        })
    }

    /// The endpoint.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}

fn io_err(e: &std::io::Error) -> ConsoleError {
    match e.kind() {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => ConsoleError::Timeout,
        _ => ConsoleError::Unreachable(e.to_string()),
    }
}

impl Console for HttpConsole {
    fn run(&mut self, command: &str, timeout: Duration) -> Result<String, ConsoleError> {
        let start = Instant::now();
        let timeout = timeout.max(Duration::from_millis(10));
        let mut stream =
            TcpStream::connect_timeout(&self.endpoint.addr, timeout).map_err(|e| io_err(&e))?;
        let left = || timeout.saturating_sub(start.elapsed());
        stream
            .set_write_timeout(Some(left().max(Duration::from_millis(1))))
            .map_err(|e| io_err(&e))?;
        let body = command.as_bytes();
        let head = format!(
            "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            self.endpoint.path,
            self.endpoint.host,
            body.len()
        );
        stream
            .write_all(head.as_bytes())
            .and_then(|()| stream.write_all(body))
            .and_then(|()| stream.flush())
            .map_err(|e| io_err(&e))?;
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let remaining = left();
            if remaining.is_zero() {
                return Err(ConsoleError::Timeout);
            }
            stream
                .set_read_timeout(Some(remaining))
                .map_err(|e| io_err(&e))?;
            let n = match stream.read(&mut chunk) {
                Ok(n) => n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(io_err(&e)),
            };
            let eof = n == 0;
            if let Some(got) = chunk.get(..n) {
                if buf.len().saturating_add(got.len()) > MAX_RESPONSE {
                    return Err(ConsoleError::Malformed(format!(
                        "answer longer than {MAX_RESPONSE} bytes"
                    )));
                }
                buf.extend_from_slice(got);
            }
            match parse_response(&buf, eof) {
                Parsed::Incomplete if eof => {
                    return Err(ConsoleError::Malformed("connection closed early".into()))
                }
                Parsed::Incomplete => {}
                Parsed::Error(e) => return Err(ConsoleError::Malformed(e)),
                Parsed::Done(status, body) => {
                    let text = String::from_utf8_lossy(&body).into_owned();
                    return if (200..300).contains(&status) {
                        Ok(text)
                    } else {
                        Err(ConsoleError::Status(status, text))
                    };
                }
            }
        }
    }
}

/// The state of a response read so far.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Parsed {
    Incomplete,
    Done(u16, Vec<u8>),
    Error(String),
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

/// Parses `buf` as an HTTP/1.x response; `eof` says the peer closed the connection.
pub(crate) fn parse_response(buf: &[u8], eof: bool) -> Parsed {
    let Some(end) = find(buf, b"\r\n\r\n") else {
        return Parsed::Incomplete;
    };
    let head = String::from_utf8_lossy(buf.get(..end).unwrap_or_default()).into_owned();
    let body = buf.get(end.saturating_add(4)..).unwrap_or_default();
    let mut lines = head.split("\r\n");
    let status_line = lines.next().unwrap_or_default();
    let mut parts = status_line.split_whitespace();
    let version = parts.next().unwrap_or_default();
    if !version.starts_with("HTTP/1.") {
        return Parsed::Error(format!("status line `{status_line}`"));
    }
    let Some(status) = parts.next().and_then(|s| s.parse::<u16>().ok()) else {
        return Parsed::Error(format!("status line `{status_line}`"));
    };
    let mut length: Option<usize> = None;
    let mut chunked = false;
    for line in lines {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let (k, v) = (k.trim(), v.trim());
        if k.eq_ignore_ascii_case("content-length") {
            match v.parse::<usize>() {
                Ok(n) => length = Some(n),
                Err(_) => return Parsed::Error(format!("Content-Length `{v}`")),
            }
        } else if k.eq_ignore_ascii_case("transfer-encoding")
            && v.to_ascii_lowercase().contains("chunked")
        {
            chunked = true;
        }
    }
    if chunked {
        return match dechunk(body) {
            Ok(Some(b)) => Parsed::Done(status, b),
            Ok(None) if eof => Parsed::Error("chunked body cut short".into()),
            Ok(None) => Parsed::Incomplete,
            Err(e) => Parsed::Error(e),
        };
    }
    match length {
        Some(n) if body.len() >= n => {
            Parsed::Done(status, body.get(..n).unwrap_or_default().to_vec())
        }
        Some(_) if eof => Parsed::Error("body shorter than Content-Length".into()),
        Some(_) => Parsed::Incomplete,
        None if eof => Parsed::Done(status, body.to_vec()),
        None => Parsed::Incomplete,
    }
}

/// The body of a complete HTTP request with a `Content-Length` (for the test server).
pub(crate) fn parse_response_request(buf: &[u8]) -> Option<Vec<u8>> {
    let end = find(buf, b"\r\n\r\n")?;
    let head = String::from_utf8_lossy(buf.get(..end)?).into_owned();
    let length = head
        .split("\r\n")
        .filter_map(|l| l.split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case("content-length"))
        .and_then(|(_, v)| v.trim().parse::<usize>().ok())
        .unwrap_or(0);
    let start = end.checked_add(4)?;
    buf.get(start..start.checked_add(length)?)
        .map(<[u8]>::to_vec)
}

/// Decodes a chunked body: `Ok(None)` while incomplete.
fn dechunk(mut rest: &[u8]) -> Result<Option<Vec<u8>>, String> {
    let mut out = Vec::new();
    loop {
        let Some(eol) = find(rest, b"\r\n") else {
            return Ok(None);
        };
        let size_text = String::from_utf8_lossy(rest.get(..eol).unwrap_or_default()).into_owned();
        let size_hex = size_text.split(';').next().unwrap_or_default().trim();
        let size =
            usize::from_str_radix(size_hex, 16).map_err(|_| format!("chunk size `{size_hex}`"))?;
        let start = eol.saturating_add(2);
        if size == 0 {
            return Ok(Some(out));
        }
        let end = start
            .checked_add(size)
            .ok_or_else(|| "chunk size overflows".to_string())?;
        let Some(data) = rest.get(start..end) else {
            return Ok(None);
        };
        if out.len().saturating_add(size) > MAX_RESPONSE {
            return Err(format!("answer longer than {MAX_RESPONSE} bytes"));
        }
        out.extend_from_slice(data);
        let Some(next) = rest.get(end..) else {
            return Ok(None);
        };
        if next.len() < 2 {
            return Ok(None);
        }
        if next.get(..2) != Some(b"\r\n".as_slice()) {
            return Err("chunk not followed by CRLF".into());
        }
        rest = next.get(2..).unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoints_are_loopback_only() {
        let e = Endpoint::parse("http://127.0.0.1:55555/console").unwrap();
        assert_eq!(e.addr.port(), 55555);
        assert_eq!(e.path, "/console");
        assert!(Endpoint::parse("http://localhost:1/").is_ok());
        assert!(Endpoint::parse("http://[::1]:8080/x").is_ok());
        assert!(Endpoint::parse("http://127.5.0.1:80").is_ok());
        for bad in [
            "https://127.0.0.1:1/console",
            "http://192.168.1.2:55555/console",
            "http://example.com:80/",
            "http://127.0.0.1/console",
            "http://127.0.0.1:99999/console",
        ] {
            assert!(Endpoint::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn responses_parse_by_length_chunks_and_eof() {
        let r = b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
        assert_eq!(
            parse_response(r, false),
            Parsed::Done(200, b"hello".to_vec())
        );
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\n\r\nhel", false),
            Parsed::Incomplete
        );
        let c = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n3\r\nabc\r\n2\r\nde\r\n0\r\n\r\n";
        assert_eq!(
            parse_response(c, false),
            Parsed::Done(200, b"abcde".to_vec())
        );
        assert_eq!(
            parse_response(b"HTTP/1.0 404 Not Found\r\n\r\nnope", true),
            Parsed::Done(404, b"nope".to_vec())
        );
        assert!(matches!(
            parse_response(b"SSH-2.0\r\n\r\n", true),
            Parsed::Error(_)
        ));
        assert!(matches!(
            parse_response(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n",
                false
            ),
            Parsed::Error(_)
        ));
        assert!(matches!(
            parse_response(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nffffffffffffffff\r\n",
                false
            ),
            Parsed::Error(_) | Parsed::Incomplete
        ));
    }
}
