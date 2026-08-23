use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    fmt,
    io::{self, Read, Write},
};

pub const MAX_FRAME: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Hello {
    pub t: String,
    pub svc: String,
    pub v: Vec<u16>,
    #[serde(default)]
    pub caps: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ready {
    pub t: String,
    pub v: u16,
    #[serde(default)]
    pub caps: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Call<T> {
    pub t: String,
    pub op: String,
    pub body: T,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Reply<T> {
    Ok {
        body: T,
    },
    Err {
        code: String,
        retry: String,
        message: String,
    },
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Json(serde_json::Error),
    Empty,
    TooLarge(usize),
    Protocol(&'static str),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(_) => f.write_str("wire I/O failed"),
            Self::Json(_) => f.write_str("invalid wire JSON"),
            Self::Empty => f.write_str("empty wire frame"),
            Self::TooLarge(_) => f.write_str("wire frame exceeds limit"),
            Self::Protocol(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for Error {}

impl From<io::Error> for Error {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for Error {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, value: &T) -> Result<(), Error> {
    let mut body = Bounded::default();
    if let Err(error) = serde_json::to_writer(&mut body, value) {
        return if body.full {
            Err(Error::TooLarge(MAX_FRAME + 1))
        } else {
            Err(Error::Json(error))
        };
    }
    let body = body.bytes;
    writer.write_all(&(body.len() as u32).to_be_bytes())?;
    writer.write_all(&body)?;
    writer.flush()?;
    Ok(())
}

pub fn read_frame<R: Read, T: DeserializeOwned>(reader: &mut R) -> Result<T, Error> {
    let mut header = [0_u8; 4];
    reader.read_exact(&mut header)?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 {
        return Err(Error::Empty);
    }
    if len > MAX_FRAME {
        return Err(Error::TooLarge(len));
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body)?)
}

pub fn negotiate(
    hello: &Hello,
    service: &str,
    versions: &[u16],
    server_caps: &[&str],
) -> Result<Ready, Error> {
    if hello.t != "hello" {
        return Err(Error::Protocol("expected hello"));
    }
    if hello.svc != service {
        return Err(Error::Protocol("service mismatch"));
    }
    let version = hello
        .v
        .iter()
        .filter(|version| versions.contains(version))
        .max()
        .copied()
        .ok_or(Error::Protocol("unsupported protocol version"))?;
    Ok(Ready {
        t: "ready".into(),
        v: version,
        caps: hello
            .caps
            .iter()
            .filter(|offered| server_caps.iter().any(|cap| offered == cap))
            .cloned()
            .collect(),
    })
}

pub fn accept_ready(hello: &Hello, ready: &Ready, required_caps: &[&str]) -> Result<(), Error> {
    if ready.t != "ready" || !hello.v.contains(&ready.v) {
        return Err(Error::Protocol("invalid ready frame"));
    }
    if ready
        .caps
        .iter()
        .any(|cap| !hello.caps.iter().any(|offered| offered == cap))
    {
        return Err(Error::Protocol("ready frame has unoffered capability"));
    }
    if required_caps
        .iter()
        .any(|required| !ready.caps.iter().any(|offered| offered == required))
    {
        return Err(Error::Protocol("ready frame lacks capability"));
    }
    Ok(())
}

#[derive(Default)]
struct Bounded {
    bytes: Vec<u8>,
    full: bool,
}

impl Write for Bounded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > MAX_FRAME {
            self.full = true;
            return Err(io::Error::new(io::ErrorKind::OutOfMemory, "frame limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub mod peer {
    use std::{io, os::fd::AsRawFd, os::unix::net::UnixStream};

    pub fn verify_same_user(stream: &UnixStream) -> io::Result<()> {
        let mut uid = 0;
        let mut gid = 0;
        // SAFETY: getpeereid only writes to the two valid out-pointers for this live socket.
        let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: geteuid has no preconditions and returns the current process identity.
        let own_uid = unsafe { libc::geteuid() };
        if uid != own_uid {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "peer user mismatch",
            ));
        }
        Ok(())
    }
}
