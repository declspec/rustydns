use std::net::ToSocketAddrs;
use std::net::UdpSocket;
use std::io::Error;

pub trait Interceptor {
    fn intercept(&self, _: &[u8]) -> Option<Vec<u8>> { None }
}

pub struct DefaultInterceptor;
impl Interceptor for DefaultInterceptor { }

pub struct UdpRelay<A: ToSocketAddrs, I: Interceptor> {
    target: A,
    interceptor: I
}

impl<A: ToSocketAddrs, I: Interceptor> UdpRelay<A, I> {
    pub fn new(target: A, interceptor: I) -> UdpRelay<A, I> {
        return UdpRelay { target: target, interceptor: interceptor }
    }

    pub fn listen(&self, addr: A, capacity: usize) -> Result<(), Error> {
        let master = try!(UdpSocket::bind(addr));
        let slave = try!(UdpSocket::bind("0.0.0.0:0"));

        // Until I can find a better way around the borrow checker, create two buffers
        // (no way to tell the compiler that an immutable borrow has ended)
        let mut ibuffer = vec![0; capacity];
        let mut obuffer = vec![0; capacity];

        loop {
            let (len, client) = try!(master.recv_from(&mut ibuffer));
            let received = &ibuffer[..len]; 

            if let Some(vec) = self.interceptor.intercept(received) {
                try!(master.send_to(&vec, client));
            }
            else {
                try!(slave.send_to(received, &self.target));
                let (len, _) = try!(slave.recv_from(&mut obuffer));
                try!(master.send_to(&obuffer[..len], client));
            }
        }
    }
}
