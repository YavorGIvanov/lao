use std::{io, net::TcpListener};

#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
pub(crate) fn listener(name: &str) -> io::Result<TcpListener> {
    use std::{
        ffi::CString,
        os::fd::{FromRawFd, OwnedFd},
    };

    unsafe extern "C" {
        fn launch_activate_socket(
            name: *const libc::c_char,
            fds: *mut *mut libc::c_int,
            count: *mut libc::size_t,
        ) -> libc::c_int;
    }

    let name = CString::new(name).map_err(|_| io::ErrorKind::InvalidInput)?;
    let (mut raw, mut count) = (std::ptr::null_mut(), 0);
    // SAFETY: launchd initializes the pointer and count for this valid C string.
    let code = unsafe { launch_activate_socket(name.as_ptr(), &mut raw, &mut count) };
    if code != 0 {
        return Err(io::Error::from_raw_os_error(code));
    }
    if count == 0 {
        // SAFETY: free accepts the launchd-owned null or allocated array pointer.
        unsafe { libc::free(raw.cast()) };
        return Err(io::Error::new(io::ErrorKind::InvalidData, "socket count"));
    }
    // SAFETY: launchd returned count owned descriptors in one allocated array.
    let mut fds: Vec<_> = unsafe { std::slice::from_raw_parts(raw, count) }
        .iter()
        .map(|fd| unsafe { OwnedFd::from_raw_fd(*fd) })
        .collect();
    // SAFETY: launchd allocates only the descriptor array with malloc.
    unsafe { libc::free(raw.cast()) };
    if fds.len() != 1 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "socket count"));
    }
    let listener = TcpListener::from(fds.pop().unwrap());
    let address = listener.local_addr()?;
    if address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST) || address.port() == 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "socket address",
        ));
    }
    listener.set_nonblocking(true)?;
    Ok(listener)
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn listener(_: &str) -> io::Result<TcpListener> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "launchd"))
}
