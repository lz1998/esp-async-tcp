use core::net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};

/// A trait for extracting representations from std types
#[doc(hidden)]
pub trait IntoInner<Inner> {
    fn into_inner(self) -> Inner;
}

/// A trait for creating std types from internal representations
#[doc(hidden)]
pub trait FromInner<Inner> {
    fn from_inner(inner: Inner) -> Self;
}

impl FromInner<esp_idf_sys::sockaddr_in> for SocketAddrV4 {
    fn from_inner(addr: esp_idf_sys::sockaddr_in) -> SocketAddrV4 {
        SocketAddrV4::new(
            Ipv4Addr::from_inner(addr.sin_addr),
            u16::from_be(addr.sin_port),
        )
    }
}

impl FromInner<esp_idf_sys::sockaddr_in6> for SocketAddrV6 {
    fn from_inner(addr: esp_idf_sys::sockaddr_in6) -> SocketAddrV6 {
        SocketAddrV6::new(
            Ipv6Addr::from_inner(addr.sin6_addr),
            u16::from_be(addr.sin6_port),
            addr.sin6_flowinfo,
            addr.sin6_scope_id,
        )
    }
}

impl FromInner<esp_idf_sys::in_addr> for Ipv4Addr {
    fn from_inner(addr: esp_idf_sys::in_addr) -> Ipv4Addr {
        Ipv4Addr::from(addr.s_addr.to_ne_bytes())
    }
}

impl FromInner<esp_idf_sys::in6_addr> for Ipv6Addr {
    #[inline]
    fn from_inner(addr: esp_idf_sys::in6_addr) -> Ipv6Addr {
        Ipv6Addr::from(unsafe { addr.un.u8_addr })
    }
}

/// A type with the same memory layout as `c::sockaddr`. Used in converting Rust level
/// SocketAddr* types into their system representation. The benefit of this specific
/// type over using `c::sockaddr_storage` is that this type is exactly as large as it
/// needs to be and not a lot larger. And it can be initialized more cleanly from Rust.
#[repr(C)]
pub(crate) union SocketAddrCRepr {
    v4: esp_idf_sys::sockaddr_in,
    v6: esp_idf_sys::sockaddr_in6,
}

impl SocketAddrCRepr {
    pub fn as_ptr(&self) -> *const esp_idf_sys::sockaddr {
        self as *const _ as *const esp_idf_sys::sockaddr
    }
}

impl<'a> IntoInner<(SocketAddrCRepr, esp_idf_sys::socklen_t)> for &'a SocketAddr {
    fn into_inner(self) -> (SocketAddrCRepr, esp_idf_sys::socklen_t) {
        match *self {
            SocketAddr::V4(ref a) => {
                let sockaddr = SocketAddrCRepr { v4: a.into_inner() };
                (
                    sockaddr,
                    core::mem::size_of::<esp_idf_sys::sockaddr_in>() as esp_idf_sys::socklen_t,
                )
            }
            SocketAddr::V6(ref a) => {
                let sockaddr = SocketAddrCRepr { v6: a.into_inner() };
                (
                    sockaddr,
                    core::mem::size_of::<esp_idf_sys::sockaddr_in6>() as esp_idf_sys::socklen_t,
                )
            }
        }
    }
}

impl IntoInner<esp_idf_sys::sockaddr_in> for SocketAddrV4 {
    fn into_inner(self) -> esp_idf_sys::sockaddr_in {
        esp_idf_sys::sockaddr_in {
            sin_family: esp_idf_sys::AF_INET as esp_idf_sys::sa_family_t,
            sin_port: self.port().to_be(),
            sin_addr: self.ip().into_inner(),
            ..unsafe { core::mem::zeroed() }
        }
    }
}

impl IntoInner<esp_idf_sys::sockaddr_in6> for SocketAddrV6 {
    fn into_inner(self) -> esp_idf_sys::sockaddr_in6 {
        esp_idf_sys::sockaddr_in6 {
            sin6_family: esp_idf_sys::AF_INET6 as esp_idf_sys::sa_family_t,
            sin6_port: self.port().to_be(),
            sin6_addr: self.ip().into_inner(),
            sin6_flowinfo: self.flowinfo(),
            sin6_scope_id: self.scope_id(),
            ..unsafe { core::mem::zeroed() }
        }
    }
}

impl IntoInner<esp_idf_sys::in_addr> for Ipv4Addr {
    #[inline]
    fn into_inner(self) -> esp_idf_sys::in_addr {
        // `s_addr` is stored as BE on all machines and the array is in BE order.
        // So the native endian conversion method is used so that it's never swapped.
        esp_idf_sys::in_addr {
            s_addr: u32::from_ne_bytes(self.octets()),
        }
    }
}

impl IntoInner<esp_idf_sys::in6_addr> for Ipv6Addr {
    fn into_inner(self) -> esp_idf_sys::in6_addr {
        esp_idf_sys::in6_addr {
            un: esp_idf_sys::in6_addr__bindgen_ty_1 {
                u8_addr: self.octets(),
            },
        }
    }
}
