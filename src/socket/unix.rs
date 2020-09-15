pub use libc::c_int;
use std::io;
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::os::unix::prelude::{AsRawFd, FromRawFd, IntoRawFd};

use super::{Addr, Domain};

// Used in `Addr`
pub use libc::{sockaddr, sockaddr_storage, socklen_t};
// Used in `Domain`.
pub use libc::{AF_INET, AF_INET6};
// Used in `Type`.
pub use libc::SOCK_STREAM;
// Used in `Protocol`.
pub use libc::IPPROTO_TCP;

impl Domain {
    // Domain for Unix socket communication.
    pub fn unix() -> Self {
        Self(libc::AF_UNIX)
    }
}

impl Addr {
    // Constructs a `Addr` with the family `AF_UNIX` and the provided path.
    // Returns an error if the path is longer than `SUN_LEN`.
    pub fn unix<P>(path: P) -> io::Result<Self>
    where
        P: AsRef<std::path::Path>,
    {
        use libc::{c_char, sockaddr_un, AF_UNIX};
        use std::cmp::Ordering;
        use std::mem::zeroed;
        use std::os::unix::ffi::OsStrExt;

        unsafe {
            let mut addr = zeroed::<sockaddr_un>();
            addr.sun_family = AF_UNIX as libc::sa_family_t;

            let bytes = path.as_ref().as_os_str().as_bytes();

            match (bytes.get(0), bytes.len().cmp(&addr.sun_path.len())) {
                // Abstract paths don't need a null terminator
                (Some(&0), Ordering::Greater) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path must be no longer than SUN_LEN",
                    ));
                }
                (Some(&0), _) => {}
                (_, Ordering::Greater) | (_, Ordering::Equal) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "path must be shorter than SUN_LEN",
                    ));
                }
                _ => {}
            }

            for (dst, src) in addr.sun_path.iter_mut().zip(bytes) {
                *dst = *src as c_char;
            }
            // null byte for pathname is already there since we zeroed up front

            let base = &addr as *const _ as usize;
            let path = &addr.sun_path as *const _ as usize;
            let sun_path_offset = path - base;

            let mut len = sun_path_offset + bytes.len();
            match bytes.get(0) {
                Some(&0) | None => {}
                Some(_) => len += 1,
            }
            Ok(Self::from_raw_parts(
                &addr as *const _ as *const _,
                len as socklen_t,
            ))
        }
    }
}

#[derive(Debug)]
pub struct Socket(c_int);

impl Socket {
    #[cfg(target_os = "linux")]
    pub fn new(family: c_int, ty: c_int, protocol: c_int) -> io::Result<Self> {
        unsafe {
            // On linux we pass the SOCK_CLOEXEC flag to atomically
            // create the socket and set it as CLOEXEC.
            let fd = libc::socket(family, ty | libc::SOCK_CLOEXEC, protocol).error()?;

            // Put socket into nonblocking mode.
            let flags = libc::fcntl(fd, libc::F_GETFL).error()? | libc::O_NONBLOCK;
            libc::fcntl(fd, libc::F_SETFL, flags).error()?;
            Ok(Self::from_raw_fd(fd))
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn new(family: c_int, ty: c_int, protocol: c_int) -> io::Result<Self> {
        unsafe {
            let fd = libc::socket(family, ty, protocol).error()?;
            // Set close-on-exec flag.
            let flags = libc::fcntl(fd, libc::F_GETFD).error()? | libc::FD_CLOEXEC;
            libc::fcntl(fd, libc::F_SETFD, flags).error()?;

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            {
                let payload = &1i32 as *const i32 as *const libc::c_void;
                libc::setsockopt(
                    fd,
                    libc::SOL_SOCKET,
                    libc::SO_NOSIGPIPE,
                    payload,
                    std::mem::size_of::<i32>() as libc::socklen_t,
                )
                .error()?;
            }

            // Put socket into nonblocking mode.
            let flags = libc::fcntl(fd, libc::F_GETFL).error()? | libc::O_NONBLOCK;
            libc::fcntl(fd, libc::F_SETFL, flags).error()?;
            Ok(Self::from_raw_fd(fd))
        }
    }

    pub fn connect(&self, addr: Addr) -> io::Result<()> {
        unsafe { libc::connect(self.0, addr.as_ptr(), addr.len()) }
            .error()
            .map(drop)
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            let _ = libc::close(self.0);
        }
    }
}

impl FromRawFd for Socket {
    unsafe fn from_raw_fd(fd: c_int) -> Self {
        Self(fd)
    }
}

impl AsRawFd for Socket {
    fn as_raw_fd(&self) -> c_int {
        self.0
    }
}

impl IntoRawFd for Socket {
    fn into_raw_fd(self) -> c_int {
        let fd = self.0;
        std::mem::forget(self);
        fd
    }
}

impl From<Socket> for TcpStream {
    fn from(socket: Socket) -> Self {
        unsafe { Self::from_raw_fd(socket.into_raw_fd()) }
    }
}

impl From<Socket> for UnixStream {
    fn from(socket: Socket) -> Self {
        unsafe { Self::from_raw_fd(socket.into_raw_fd()) }
    }
}

trait ToError: Sized {
    fn error(self) -> io::Result<Self>;
}

macro_rules! impl_is_error {
    ($($t:ident)*) => ($(impl ToError for $t {
        fn error(self) -> io::Result<Self> {
            if self == -1 {
                Err(io::Error::last_os_error())
            } else {
                Ok(self)
            }

        }
    })*)
}

impl_is_error! { i8 i16 i32 i64 isize }
