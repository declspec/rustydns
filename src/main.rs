#[macro_use]
extern crate lazy_static;
extern crate regex;

mod dnsproxy;

use std::str;
use std::net::{IpAddr,UdpSocket};
use std::io::{BufRead,BufReader};
use std::fs::File;
use std::path::Path;
use regex::Regex;

// Constants
const DNS_HEADER_SIZE: usize = 0x0C;
const DEFAULT_TTL: u32 = 1800; // half an hour

struct DnsRecord {
    dest: IpAddr,
    domain: Regex,
    ttl: u32
}

struct DnsQuestion {
    domain: String,
    qtype: u16,
    qclass: u16
}

struct DnsMessageHeader {
    id: u16,
    flags: u16,
    qd_count: u16
}

impl DnsRecord {
    fn is_match(&self, domain: &str) -> bool {
        return self.domain.is_match(domain);
    }
}

impl DnsMessageHeader {
    fn is_valid_question(&self) -> bool {
        return self.qd_count > 0 // Has a question
            && (self.flags & 0x8000) == 0 // 'QR' flag is not set
            && (self.flags & 0x7800) == 0 // Regular query type
    }
}

fn to_u16(buffer: &[u8], offset: usize) -> u16 {
    return ((buffer[offset] as u16) << 8) | buffer[offset + 1] as u16;
}

fn parse_dns_record(line: &str) -> Option<DnsRecord> {
    lazy_static!{
        static ref RECORD_PATTERN: Regex = Regex::new(r"^#@\s+(?P<ip>(?:[0-9]{1,3}\.){3}[0-9]{1,3})\s+(?P<re>[^\s]+)(?:\s+\[(?P<o>[^\]]+)\])?").unwrap();
    }

    let trimmed = line.trim();
    let result = RECORD_PATTERN.captures(trimmed)
        .ok_or("shat meself")
        .and_then(|caps| caps["ip"].parse()
            .map_err(|_| "invalid IPv4 address provided")
            .and_then(|ip| Regex::new(&caps["re"])
                .map_err(|_| "malformed regular expression")
                .and_then(|re| Ok(DnsRecord{ dest: ip, domain: re, ttl: DEFAULT_TTL }))
            )
        );

    match result {
        Ok(record) => return Some(record),
        Err(reason) => println!("warning: skipping '{}' ({})", trimmed, reason)
    }

    return None;
}

fn parse_dns_records<R: BufRead>(reader: R) -> Vec<DnsRecord> {
    return reader.lines().filter_map(|res|
        res.ok().and_then(|line| parse_dns_record(&line))
    ).collect();
}

fn read_dns_records<P: AsRef<Path>>(path: P) -> Option<Vec<DnsRecord>> {
    return File::open(path).ok()
        .and_then(|f| Some(parse_dns_records(BufReader::new(f))));
}

fn parse_domain_label(input: &[u8]) -> Option<(usize, &str)> {
    let len = input[0] as usize;
    
    match len {
        0 => None,
        _ => Some((len + 1, str::from_utf8(&input[1..len + 1]).unwrap())),
    }   
}

fn parse_message(input: &[u8]) -> Option<(&[u8], DnsQuestion)> {
    let mut labels = Vec::new();
    let mut consumed = DNS_HEADER_SIZE; 

    let header = DnsMessageHeader {
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

    let question = DnsQuestion {
        domain: labels.join("."),
        qtype: to_u16(input, consumed + 1),
        qclass: to_u16(input, consumed + 3)
    };

    return match question.qtype {
        // Can only handle A (0x01) or AAAA (0x1C) records at this time
        0x01 | 0x1C => Some((&input[..consumed+5], question)),
        _ => None
    }
}

fn create_message(base: &[u8], record: &DnsRecord) -> Result<Vec<u8>, &'static str> {
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
    if let IpAddr::V4(v4) = record.dest {
        message.extend_from_slice(&v4.octets());
        return Ok(message);
    }
    
    return Err("Supplied IP address protocol is not currently supported");
}

fn main() {
    let records = read_dns_records("/etc/hosts").expect("failed to read records from hosts file");
    println!("configuring {} dns records", records.len());

    let master = UdpSocket::bind("127.0.0.1:53").unwrap();
    let slave = UdpSocket::bind("0.0.0.0:0").unwrap();

    loop {
        let mut buffer = [0; 512];
        let (len, client) = master.recv_from(&mut buffer).unwrap();

        if let Some((msg, question)) = parse_message(&buffer[..len]) {
            if let Some(record) = records.iter().find(|r| r.is_match(&question.domain)) {
                // Found a matching wildcard, forward it on
                let response = create_message(&msg, &record).unwrap();
                master.send_to(&response, client).expect("Failed to send response to client");
                continue;
            }
        }

        slave.send_to(&buffer[..len], "8.8.8.8:53").expect("Failed to forward data to the Google DNS server");
        let (len, _) = slave.recv_from(&mut buffer).expect("Did not receive a response from the Google DNS server");
        master.send_to(&buffer[..len], client).expect("Failed to send response to client");
    }
}

