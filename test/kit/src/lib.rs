use lao_core_api::Fault;
use lao_wire::{Call, Hello, Ready, Reply, accept_ready, negotiate, read_frame, write_frame};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const SERVICE: &str = "probe";
pub const CAP: &str = "probe.check";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Check {
    pub value: u64,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Checked {
    pub value: u64,
}

pub trait Probe: Send + Sync {
    fn check(&self, request: Check) -> Result<Checked, Fault>;
}

#[derive(Debug, Default)]
pub struct Fake {
    calls: AtomicUsize,
}

impl Fake {
    pub fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl Probe for Fake {
    fn check(&self, request: Check) -> Result<Checked, Fault> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if request.value == 0 {
            return Err(Fault::unsupported());
        }
        Ok(Checked {
            value: request.value * 2,
        })
    }
}

pub fn linked(probe: &impl Probe, request: Check) -> Result<Checked, Fault> {
    probe.check(request)
}

#[cfg(unix)]
pub fn serve_once(
    mut stream: std::os::unix::net::UnixStream,
    probe: &impl Probe,
) -> Result<(), lao_wire::Error> {
    #[cfg(target_os = "macos")]
    lao_wire::peer::verify_same_user(&stream)?;

    let hello: Hello = read_frame(&mut stream)?;
    let ready = negotiate(&hello, SERVICE, &[0], &[CAP])?;
    if !ready.caps.iter().any(|cap| cap == CAP) {
        return Err(lao_wire::Error::Protocol("missing capability"));
    }
    write_frame(&mut stream, &ready)?;
    let call: Call<Check> = read_frame(&mut stream)?;
    if call.t != "call" {
        return Err(lao_wire::Error::Protocol("expected call"));
    }
    if call.op != "check" {
        return write_frame(
            &mut stream,
            &Reply::<Checked>::Err {
                code: "unsupported".into(),
                retry: "never".into(),
                message: "unknown operation".into(),
            },
        );
    }

    let reply = match probe.check(call.body) {
        Ok(body) => Reply::Ok { body },
        Err(error) => Reply::Err {
            code: error.code,
            retry: error.retry,
            message: error.message,
        },
    };
    write_frame(&mut stream, &reply)
}

#[cfg(unix)]
pub fn rpc(mut stream: std::os::unix::net::UnixStream, request: Check) -> Result<Checked, Fault> {
    #[cfg(target_os = "macos")]
    lao_wire::peer::verify_same_user(&stream).map_err(|error| wire_fault(error.into()))?;

    let hello = Hello {
        t: "hello".into(),
        svc: SERVICE.into(),
        v: vec![0],
        caps: vec![CAP.into()],
    };
    write_frame(&mut stream, &hello).map_err(wire_fault)?;
    let ready: Ready = read_frame(&mut stream).map_err(wire_fault)?;
    accept_ready(&hello, &ready, &[CAP]).map_err(wire_fault)?;
    let call = Call {
        t: "call".into(),
        op: "check".into(),
        body: request,
    };
    write_frame(&mut stream, &call).map_err(wire_fault)?;
    match read_frame(&mut stream).map_err(wire_fault)? {
        Reply::Ok { body } => Ok(body),
        Reply::Err {
            code,
            retry,
            message,
        } => Err(Fault {
            code,
            retry,
            message,
        }),
    }
}

#[cfg(unix)]
pub fn assert_conformance<P: Probe + 'static>(probe: std::sync::Arc<P>) {
    use std::{os::unix::net::UnixStream, sync::Arc, thread};

    let ok = Check {
        value: 7,
        note: Some("additive".into()),
    };
    let expected = linked(probe.as_ref(), ok.clone()).expect("linked success");
    let (client, server) = UnixStream::pair().expect("socket pair");
    let server_probe = Arc::clone(&probe);
    let handle = thread::spawn(move || serve_once(server, server_probe.as_ref()));
    assert_eq!(rpc(client, ok).expect("RPC success"), expected);
    handle
        .join()
        .expect("server thread")
        .expect("serve success");

    let rejected = Check {
        value: 0,
        note: None,
    };
    let expected = linked(probe.as_ref(), rejected.clone()).expect_err("linked error");
    let (client, server) = UnixStream::pair().expect("socket pair");
    let server_probe = Arc::clone(&probe);
    let handle = thread::spawn(move || serve_once(server, server_probe.as_ref()));
    assert_eq!(rpc(client, rejected).expect_err("RPC error"), expected);
    handle.join().expect("server thread").expect("serve error");
}

fn wire_fault(error: lao_wire::Error) -> Fault {
    Fault {
        code: "unavailable".into(),
        retry: "later".into(),
        message: error.to_string(),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{io::Write, os::unix::net::UnixStream, sync::Arc, thread};

    fn request() -> Check {
        Check {
            value: 7,
            note: Some("additive".into()),
        }
    }

    #[test]
    fn linked_and_rpc_conform() {
        let fake = Arc::new(Fake::default());
        assert_conformance(Arc::clone(&fake));
        assert_eq!(fake.calls(), 4);
    }

    #[test]
    fn additive_field_is_compatible() {
        #[derive(Serialize)]
        struct PreviousWrite {
            value: u64,
        }

        #[derive(Deserialize)]
        struct Previous {
            value: u64,
        }

        let encoded = serde_json::to_vec(&request()).expect("encode current request");
        let previous: Previous = serde_json::from_slice(&encoded).expect("decode previous request");
        assert_eq!(previous.value, 7);

        let old = serde_json::to_vec(&PreviousWrite { value: 8 }).expect("encode previous request");
        let current: Check = serde_json::from_slice(&old).expect("decode current request");
        assert_eq!(current.note, None);
    }

    #[test]
    fn unsupported_version_never_dispatches() {
        let fake = Fake::default();
        let hello = Hello {
            t: "hello".into(),
            svc: SERVICE.into(),
            v: vec![1],
            caps: vec![CAP.into()],
        };
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &hello).expect("encode hello");
        let error = serve_bytes(&bytes, &fake).expect_err("version must fail");
        assert!(error.to_string().contains("version"));
        assert_eq!(fake.calls(), 0);
    }

    #[test]
    fn missing_capability_never_dispatches() {
        let fake = Fake::default();
        let hello = Hello {
            t: "hello".into(),
            svc: SERVICE.into(),
            v: vec![0],
            caps: Vec::new(),
        };
        let mut bytes = Vec::new();
        write_frame(&mut bytes, &hello).expect("encode hello");
        let error = serve_bytes(&bytes, &fake).expect_err("capability must fail");
        assert!(error.to_string().contains("capability"));
        assert_eq!(fake.calls(), 0);
    }

    #[test]
    fn oversized_frame_fails_before_body_read() {
        let mut bytes = ((lao_wire::MAX_FRAME + 1) as u32).to_be_bytes().to_vec();
        bytes.write_all(b"{}").expect("append body");
        let error = read_frame::<_, Hello>(&mut bytes.as_slice()).expect_err("frame must fail");
        assert!(matches!(error, lao_wire::Error::TooLarge(_)));
    }

    #[test]
    fn malformed_frames_fail() {
        let zero_bytes = 0_u32.to_be_bytes();
        let mut zero = zero_bytes.as_slice();
        assert!(matches!(
            read_frame::<_, Hello>(&mut zero),
            Err(lao_wire::Error::Empty)
        ));

        let mut truncated = [0, 0, 0, 4, b'{'].as_slice();
        assert!(matches!(
            read_frame::<_, Hello>(&mut truncated),
            Err(lao_wire::Error::Io(_))
        ));

        let duplicate = br#"{"t":"hello","t":"hello","svc":"probe","v":[1]}"#;
        let mut framed = Vec::new();
        framed.extend_from_slice(&(duplicate.len() as u32).to_be_bytes());
        framed.extend_from_slice(duplicate);
        assert!(matches!(
            read_frame::<_, Hello>(&mut framed.as_slice()),
            Err(lao_wire::Error::Json(_))
        ));
    }

    #[test]
    fn unknown_operation_never_dispatches() {
        let fake = Arc::new(Fake::default());
        let (mut client, server) = UnixStream::pair().expect("socket pair");
        let server_fake = Arc::clone(&fake);
        let handle = thread::spawn(move || serve_once(server, server_fake.as_ref()));
        let hello = Hello {
            t: "hello".into(),
            svc: SERVICE.into(),
            v: vec![0],
            caps: vec![CAP.into()],
        };
        write_frame(&mut client, &hello).expect("write hello");
        let ready: Ready = read_frame(&mut client).expect("read ready");
        accept_ready(&hello, &ready, &[CAP]).expect("accept ready");
        let call = Call {
            t: "call".into(),
            op: "future".into(),
            body: request(),
        };
        write_frame(&mut client, &call).expect("write call");
        let reply: Reply<Checked> = read_frame(&mut client).expect("read error");
        assert!(
            matches!(reply, Reply::Err { code, retry, .. } if code == "unsupported" && retry == "never")
        );
        handle.join().expect("server thread").expect("serve call");
        assert_eq!(fake.calls(), 0);
    }

    #[test]
    fn invalid_ready_is_rejected() {
        let hello = Hello {
            t: "hello".into(),
            svc: SERVICE.into(),
            v: vec![0],
            caps: vec![CAP.into()],
        };
        for ready in [
            Ready {
                t: "other".into(),
                v: 0,
                caps: vec![CAP.into()],
            },
            Ready {
                t: "ready".into(),
                v: 1,
                caps: vec![CAP.into()],
            },
            Ready {
                t: "ready".into(),
                v: 0,
                caps: Vec::new(),
            },
            Ready {
                t: "ready".into(),
                v: 0,
                caps: vec![CAP.into(), "future".into()],
            },
        ] {
            assert!(accept_ready(&hello, &ready, &[CAP]).is_err());
        }
    }

    fn serve_bytes(bytes: &[u8], fake: &Fake) -> Result<(), lao_wire::Error> {
        let mut reader = bytes;
        let hello: Hello = read_frame(&mut reader)?;
        let ready = negotiate(&hello, SERVICE, &[0], &[CAP])?;
        if !ready.caps.iter().any(|cap| cap == CAP) {
            return Err(lao_wire::Error::Protocol("missing capability"));
        }
        let _ = fake;
        Ok(())
    }
}
