#![no_std]
#![feature(ip_in_core)]

mod inner;

use crate::inner::{FromInner, IntoInner};
use core::ffi::c_void;
use core::future;
use core::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use core::task::Poll;

#[derive(Debug)]
pub struct Socket(core::ffi::c_int);

type IOError = core::ffi::c_int;

#[derive(Debug)]
pub struct TcpStream {
    inner: Socket,
}

#[derive(Debug)]
pub struct TcpListener {
    inner: Socket,
}

impl TcpListener {
    pub fn bind(addr: &SocketAddr) -> Result<TcpListener, IOError> {
        let sock = Socket::new(addr, esp_idf_sys::SOCK_STREAM as core::ffi::c_int)?;
        sock.set_nonblocking()?;
        sock.setsockopt(
            esp_idf_sys::SOL_SOCKET as core::ffi::c_int,
            esp_idf_sys::SO_REUSEADDR as core::ffi::c_int,
            1,
        )?;
        unsafe {
            let (addr, addr_len) = addr.into_inner();
            cvt(esp_idf_sys::lwip_bind(sock.0, addr.as_ptr(), addr_len))?;
            cvt(esp_idf_sys::lwip_listen(sock.0, 128))?;
        }
        Ok(TcpListener { inner: sock })
    }

    pub async fn accept(&self) -> Result<(TcpStream, SocketAddr), IOError> {
        let (stream, addr) = future::poll_fn(|_cx| self.inner.poll_accept()).await?;
        stream.inner.set_nonblocking()?;
        Ok((stream, addr))
    }
}

fn cvt(n: core::ffi::c_int) -> Result<core::ffi::c_int, IOError> {
    if n < 0 {
        Err(core::mem::replace(
            unsafe { &mut *esp_idf_sys::__errno() },
            0,
        ))
    } else {
        Ok(n)
    }
}

fn cvt_poll(n: core::ffi::c_int) -> Poll<Result<core::ffi::c_int, IOError>> {
    if n < 0 {
        let errno = core::mem::replace(unsafe { &mut *esp_idf_sys::__errno() }, 0);
        if errno == esp_idf_sys::EAGAIN as i32 || errno == esp_idf_sys::EWOULDBLOCK as i32 {
            Poll::Pending
        } else {
            Poll::Ready(Err(errno))
        }
    } else {
        Poll::Ready(Ok(n))
    }
}

impl Socket {
    pub fn new(addr: &SocketAddr, ty: core::ffi::c_int) -> Result<Socket, IOError> {
        let fam = match *addr {
            SocketAddr::V4(..) => esp_idf_sys::AF_INET,
            SocketAddr::V6(..) => esp_idf_sys::AF_INET6,
        } as core::ffi::c_int;
        Socket::new_raw(fam, ty)
    }

    pub fn new_raw(fam: core::ffi::c_int, ty: core::ffi::c_int) -> Result<Socket, IOError> {
        let fd = cvt(unsafe { esp_idf_sys::lwip_socket(fam, ty, 0) })?;
        let socket = Socket(fd);
        Ok(socket)
    }

    pub fn set_nonblocking(&self) -> Result<(), IOError> {
        let mut flag = cvt(unsafe {
            esp_idf_sys::lwip_fcntl(self.0, esp_idf_sys::F_GETFL as core::ffi::c_int, 0)
        })?;
        flag |= esp_idf_sys::O_NONBLOCK as i32;
        cvt(unsafe {
            esp_idf_sys::lwip_fcntl(self.0, esp_idf_sys::F_SETFL as core::ffi::c_int, flag)
        })?;
        Ok(())
    }

    pub fn poll_accept(&self) -> Poll<Result<(TcpStream, SocketAddr), IOError>> {
        let mut storage: esp_idf_sys::sockaddr_storage = unsafe { core::mem::zeroed() };
        let mut len = core::mem::size_of_val(&storage) as esp_idf_sys::socklen_t;
        cvt_poll(unsafe {
            esp_idf_sys::lwip_accept(
                self.0,
                &mut storage as *mut esp_idf_sys::sockaddr_storage as *mut esp_idf_sys::sockaddr,
                &mut len,
            )
        })?
        .map(|fd| {
            let sock = Socket(fd);
            let addr = sockaddr_to_addr(&storage, len as usize)?;
            Ok((TcpStream { inner: sock }, addr))
        })
    }

    pub fn poll_read(&self, buf: &mut [u8]) -> Poll<Result<i32, IOError>> {
        cvt_poll(unsafe {
            esp_idf_sys::lwip_read(self.0, buf.as_mut_ptr() as *mut c_void, buf.len())
        } as i32)
    }

    pub fn poll_write(&self, buf: &[u8]) -> Poll<Result<i32, IOError>> {
        cvt_poll(unsafe {
            esp_idf_sys::lwip_write(self.0, buf.as_ptr() as *const c_void, buf.len())
        } as i32)
    }

    pub fn setsockopt<T>(
        &self,
        level: core::ffi::c_int,
        option_name: core::ffi::c_int,
        option_value: T,
    ) -> Result<(), IOError> {
        unsafe {
            cvt(esp_idf_sys::lwip_setsockopt(
                self.0,
                level,
                option_name,
                &option_value as *const T as *const _,
                core::mem::size_of::<T>() as esp_idf_sys::socklen_t,
            ))?;
            Ok(())
        }
    }
}

impl Drop for Socket {
    fn drop(&mut self) {
        unsafe {
            esp_idf_sys::lwip_close(self.0);
        }
    }
}

impl TcpStream {
    pub async fn read(&self, buf: &mut [u8]) -> Result<i32, IOError> {
        future::poll_fn(|_cx| self.inner.poll_read(buf)).await
    }
    pub async fn write(&self, buf: &[u8]) -> Result<i32, IOError> {
        future::poll_fn(|_cx| self.inner.poll_write(buf)).await
    }
}

pub(crate) fn sockaddr_to_addr(
    storage: &esp_idf_sys::sockaddr_storage,
    len: usize,
) -> Result<SocketAddr, IOError> {
    match storage.ss_family as u32 {
        esp_idf_sys::AF_INET => {
            assert!(len >= core::mem::size_of::<esp_idf_sys::sockaddr_in>());
            Ok(SocketAddr::V4(SocketAddrV4::from_inner(unsafe {
                *(storage as *const _ as *const esp_idf_sys::sockaddr_in)
            })))
        }
        esp_idf_sys::AF_INET6 => {
            assert!(len >= core::mem::size_of::<esp_idf_sys::sockaddr_in6>());
            Ok(SocketAddr::V6(SocketAddrV6::from_inner(unsafe {
                *(storage as *const _ as *const esp_idf_sys::sockaddr_in6)
            })))
        }
        _ => Err(-1),
    }
}
