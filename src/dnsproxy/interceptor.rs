extern crate regex;

use std::str;
use std::net::IpAddr;
use regex::Regex;
use dnsproxy::Interceptor;

const DNS_HEADER_SIZE: usize = 12;

pub enum NamePattern {
    Regex(Regex),
    Literal(String)
}

pub struct ResourceRecord {
    pub rdata: IpAddr,
    pub name: NamePattern,
    pub ttl: u32
}

pub struct DnsInterceptor {
    records: Vec<ResourceRecord>
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

impl NamePattern {
    fn satisfies_query(&self, qname: &str) -> bool {
        match *self {
            NamePattern::Regex(ref r) => r.is_match(qname),
            NamePattern::Literal(ref l) => l == qname
        }
    }
}

impl Header {
    fn is_valid_query(&self) -> bool {
        return self.qd_count > 0 // Has a question
            && (self.flags & 0x8000) == 0 // 'QR' flag is not set
            && (self.flags & 0x7800) == 0 // Regular query type
    }
}

impl Interceptor for DnsInterceptor {
    fn overwrite(&self, datagram: &[u8]) -> Option<&[u8]> {
        return parse_message(datagram).and_then(|(msg,question)| 
            self.records.iter().find(|r| r.name.satisfies_query(&question.qname))
                .and_then(|record| Some(create_message(msg, record)))
        );
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
    if !header.is_valid_query() {
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

fn create_message<'a>(base: &[u8], record: &ResourceRecord) -> &'a [u8] {
    let rtype = 0x01; // Hardcode IPv4 for now (A record)
    let rlen = 0x04; // ^

    // Convert the ttl into an array of bytes to push into the header
    let ttl = [ (record.ttl >> 24) as u8, (record.ttl >> 16) as u8, (record.ttl >> 8) as u8, record.ttl as u8 ]; 

    // Sub-header definition, sets the QR flag and sets the QD/AN counts to 1
    let header = [ 0x80, 0, 0, 1, 0, 1, 0, 0, 0, 0 ];
    // Answer is just an offset to the question (0xC0, 0x0C means the question starts at offset 12, straight after the header)
    // Followed by the type (u16), class (u16, "1" = IN), ttl (u32) and finally length (u16)
    let answer = [ 0xC0, 0x0C, 0, rtype, 0, 1, ttl[0], ttl[1], ttl[2], ttl[3], 0, rlen ];
    let mut message = base.to_vec();

    // Copy the new header bits into the message (don't overwrite the ID)
    &message[2..DNS_HEADER_SIZE].copy_from_slice(&header);
    message.extend_from_slice(&answer);

    // Append the IP address
    if let IpAddr::V4(v4) = record.rdata {
        message.extend_from_slice(&v4.octets());
    }
    
    return &message[..];
}