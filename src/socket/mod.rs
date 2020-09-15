#[cfg(unix)]
#[path = "unix.rs"]
mod sys;
#[cfg(windows)]
#[path = "windows.rs"]
mod sys;

use std::io;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6, TcpStream};

#[derive(Debug)]
pub struct Domain(sys::c_int);

impl Domain {
    // Domain for IPv4 communication.
    pub fn ipv4() -> Self {
        Self(sys::AF_INET)
    }

    // Domain for IPv6 communication.
    pub fn ipv6() -> Self {
        Self(sys::AF_INET6)
    }
}

#[derive(Debug)]
pub struct Type(sys::c_int);

impl Type {
    // Used for protocols such as TCP.
    pub fn stream() -> Self {
        Type(sys::SOCK_STREAM)
    }
}

#[derive(Debug)]
pub struct Protocol(sys::c_int);

impl Protocol {
    // Protocol corresponding to `TCP`.
    pub fn tcp() -> Self {
        Self(sys::IPPROTO_TCP)
    }
}

// `Addr`s may be constructed directly to and from the standard library
// `SocketAddr`, `SocketAddrV4`, and `SocketAddrV6` types.
#[allow(missing_debug_implementations)]
pub struct Addr {
    storage: sys::sockaddr_storage,
    len: sys::socklen_t,
}

impl Addr {
    // Constructs a `Addr` from its raw components.
    unsafe fn from_raw_parts(addr: *const sys::sockaddr, len: sys::socklen_t) -> Self {
        use std::mem::MaybeUninit;

        let mut storage = MaybeUninit::<sys::sockaddr_storage>::uninit();
        std::ptr::copy_nonoverlapping(
            addr as *const _ as *const u8,
            &mut storage as *mut _ as *mut u8,
            len as usize,
        );

        Self {
            // This is safe as we written the address to `storage` above.
            storage: storage.assume_init(),
            len,
        }
    }

    // Returns the size of this address in bytes.
    pub fn len(&self) -> sys::socklen_t {
        self.len
    }

    // Returns a raw pointer to the address.
    pub fn as_ptr(&self) -> *const sys::sockaddr {
        &self.storage as *const _ as *const _
    }
}

impl From<SocketAddrV4> for Addr {
    fn from(addr: SocketAddrV4) -> Self {
        unsafe {
            Self::from_raw_parts(
                &addr as *const _ as *const _,
                std::mem::size_of::<SocketAddrV4>() as sys::socklen_t,
            )
        }
    }
}

impl From<SocketAddrV6> for Addr {
    fn from(addr: SocketAddrV6) -> Self {
        unsafe {
            Self::from_raw_parts(
                &addr as *const _ as *const _,
                std::mem::size_of::<SocketAddrV6>() as sys::socklen_t,
            )
        }
    }
}

impl From<SocketAddr> for Addr {
    fn from(addr: SocketAddr) -> Self {
        match addr {
            SocketAddr::V4(addr) => Self::from(addr),
            SocketAddr::V6(addr) => Self::from(addr),
        }
    }
}

#[derive(Debug)]
pub struct Socket(sys::Socket);

impl Socket {
    // This function corresponds to `socket(2)` and simply creates a new
    // socket and moves it into nonblocking mode.
    pub fn new(domain: Domain, type_: Type, protocol: Option<Protocol>) -> io::Result<Self> {
        let protocol = protocol.map(|p| p.0).unwrap_or(0);
        let socket = sys::Socket::new(domain.0, type_.0, protocol)?;
        Ok(Self(socket))
    }

    // Initiate a connection on this socket to the specified address.
    // This function directly corresponds to the connect(2) function on Windows
    // and Unix.
    // An error will be returned if `listen` or `connect` has already been
    // called on this builder.
    pub fn connect(&self, addr: impl Into<Addr>) -> io::Result<()> {
        self.0.connect(addr.into())
    }
}

impl From<Socket> for TcpStream {
    fn from(socket: Socket) -> Self {
        Self::from(socket.0)
    }
}

#[cfg(unix)]
impl From<Socket> for std::os::unix::net::UnixStream {
    fn from(socket: Socket) -> Self {
        Self::from(socket.0)
    }
}
