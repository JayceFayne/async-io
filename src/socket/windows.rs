use std::io;
use std::net::{TcpStream, UdpSocket};
use std::os::windows::prelude::{AsRawSocket, FromRawSocket, IntoRawSocket, RawSocket};
use std::ptr;
use std::sync::Once;
pub use winapi::ctypes::c_int;
use winapi::ctypes::c_ulong;
use winapi::shared::minwindef::DWORD;
use winapi::shared::ntdef::HANDLE;
use winapi::um::handleapi::SetHandleInformation;
use winapi::um::winsock2 as sock;

use super::Addr;

// Used in `Addr`
pub use winapi::shared::ws2def::{SOCKADDR as sockaddr, SOCKADDR_STORAGE as sockaddr_storage};
pub use winapi::um::ws2tcpip::socklen_t;
// Used in `Domain`.
pub use winapi::shared::ws2def::{AF_INET, AF_INET6};
// Used in `Type`.
pub use winapi::shared::ws2def::SOCK_STREAM;
// Used in `Protocol`.
pub const IPPROTO_TCP: c_int = winapi::shared::ws2def::IPPROTO_TCP as c_int;

const HANDLE_FLAG_INHERIT: DWORD = 0x00000001;
const WSA_FLAG_OVERLAPPED: DWORD = 0x01;

fn init() {
    static INIT: Once = Once::new();

    INIT.call_once(|| {
        // Initialize winsock through the standard library by just creating a
        // dummy socket. Whether this is successful or not we drop the result as
        // libstd will be sure to have initialized winsock.
        let _ = UdpSocket::bind("127.0.0.1:34254");
    });
}

fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { sock::WSAGetLastError() })
}

#[derive(Debug)]
pub struct Socket {
    socket: sock::SOCKET,
}

impl Socket {
    pub fn new(family: c_int, ty: c_int, protocol: c_int) -> io::Result<Socket> {
        init();
        unsafe {
            let socket = match sock::WSASocketW(
                family,
                ty,
                protocol,
                ptr::null_mut(),
                0,
                WSA_FLAG_OVERLAPPED,
            ) {
                sock::INVALID_SOCKET => return Err(last_error()),
                socket => socket,
            };
            // Set no inherit.
            if SetHandleInformation(socket as HANDLE, HANDLE_FLAG_INHERIT, 0) == 0 {
                return Err(io::Error::last_os_error());
            }
            // Put socket into nonblocking mode.
            let mut nonblocking = true as c_ulong;
            if sock::ioctlsocket(socket, sock::FIONBIO as c_int, &mut nonblocking) != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Socket::from_raw_socket(socket as RawSocket))
        }
    }

    pub fn connect(&self, addr: Addr) -> io::Result<()> {
        unsafe {
            if sock::connect(self.socket, addr.as_ptr(), addr.len()) == 0 {
                Ok(())
            } else {
                Err(last_error())
            }
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            let _ = sock::closesocket(self.socket);
        }
    }
}

impl FromRawSocket for Socket {
    unsafe fn from_raw_socket(socket: RawSocket) -> Self {
        Self {
            socket: socket as sock::SOCKET,
        }
    }
}

impl AsRawSocket for Socket {
    fn as_raw_socket(&self) -> RawSocket {
        self.socket as RawSocket
    }
}

impl IntoRawSocket for Socket {
    fn into_raw_socket(self) -> RawSocket {
        let socket = self.socket;
        std::mem::forget(self);
        socket as RawSocket
    }
}

impl From<Socket> for TcpStream {
    fn from(socket: Socket) -> Self {
        unsafe { Self::from_raw_socket(socket.into_raw_socket()) }
    }
}
