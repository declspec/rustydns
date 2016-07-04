
use std::net::{ToSocketAddrs,UdpSocket};

pub use self::relay::{UdpRelay,Interceptor};
pub use self::interceptor::DnsInterceptor;

mod relay;
mod interceptor;
