use dnsproxy::{ResourceRecord,NamePattern};
use std::net::{ToSocketAddrs,UdpSocket};

const DNS_HEADER_SIZE: usize = 12;

pub struct Server {
    records: Vec<ResourceRecord>,
    forwarder: ToSocketAddrs
}

struct Header {
    id: u16,
    flags: u16,
    qd_count: u16
}

struct Question {
    qname: String,
    qtype: u16,
    qclass: u16
}

impl Server {
    pub fn listen<A: ToSocketAddrs>(&self, addr: A) {
        let master = UdpSocket::bind(addr).unwrap();
        let slave = UdpSocket::bind("0.0.0.0:0").unwrap();
    }
}

fn to_u16(buffer: &[u8], offset: usize) -> u16 {
    return ((buffer[offset] as u16) << 8) | buffer[offset + 1] as u16;
}

fn parse_domain_label(input: &[u8]) -> Option<(usize, &str)> {
    let len = input[0] as usize;
    
    match len {
        0 => None,
        _ => Some((len + 1, str::from_utf8(&input[1..len + 1]).unwrap())),
    }   
}

fn parse_message(input: &[u8]) -> Option<(&[u8], Question)> {
    let mut labels = Vec::new();
    let mut consumed = DNS_HEADER_SIZE; 

    let header = Header {
        id: to_u16(input, 0),
        flags: to_u16(input, 2),
        qd_count: to_u16(input, 4)
    };

    // Not a valid question header, we don't want it
    if !header.is_valid_question() {
        return None;
    }

    while let Some((size, label)) = parse_domain_label(&input[consumed..]) {
        consumed += size;
        labels.push(label);
    }

    let question = Question {
        qname: labels.join("."),
        qtype: to_u16(input, consumed + 1),
        qclass: to_u16(input, consumed + 3)
    };

    return match question.qtype {
        // Can only handle A (0x01) or AAAA (0x1C) records at this time
        0x01 | 0x1C => Some((&input[..consumed+5], question)),
        _ => None
    }
}