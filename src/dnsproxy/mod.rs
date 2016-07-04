extern crate regex;

use std::net::{IpAddr,UdpSocket};
use regex::Regex;

pub use self::proxy::Server;

mod proxy;

pub enum NamePattern {
    Regex(Regex),
    Literal(String)
}

pub struct ResourceRecord {
    pub rdata: IpAddr,
    pub name: NamePattern,
    pub ttl: u32
}

impl NamePattern {
    fn satisfies_query(&self, qname: &str) -> bool {
        match *self {
            NamePattern::Regex(ref r) => r.is_match(qname),
            NamePattern::Literal(ref l) => l == qname
        }
    }
}