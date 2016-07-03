extern crate regex;

use std::str;
use std::net::UdpSocket;
use std::net::IpAddr;
use regex::Regex;

struct DomainRecord {
    dest: IpAddr,
    domain: Regex
}

struct DnsQuestion {
    domain: String,
    qtype: u16,
    qclass: u16
}

struct DnsMessageHeader {
    id: u16,
    flags: u16,
    qd_count: u16,
    an_count: u16,
    ns_count: u16,
    ar_count: u16
}

impl DomainRecord {
    fn is_match(&self, domain: &str) -> bool {
        return self.domain.is_match(domain);
    }
}

fn mk_u16(buffer: &[u8], offset: usize) -> u16 {
    return ((buffer[offset] as u16) << 8) | buffer[offset + 1] as u16;
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
    let mut consumed = 12; 

    let header = DnsMessageHeader {
        id: mk_u16(input, 0),
        flags: mk_u16(input, 2),
        qd_count: mk_u16(input, 4),
        an_count: mk_u16(input, 6),
        ns_count: mk_u16(input, 8),
        ar_count: mk_u16(input, 10)
    };

    // No questions, we don't care about this message
    if header.qd_count == 0 {
        return None;
    }

    while let Some((size, label)) = parse_domain_label(&input[consumed..]) {
        consumed += size;
        labels.push(label);
    }

    let question = DnsQuestion {
        domain: labels.join("."),
        qtype: mk_u16(input, consumed + 1),
        qclass: mk_u16(input, consumed + 3)
    };

    return match question.qtype {
        0x01 | 0x1C => Some((&input[..consumed+5], question)),
        _ => None
    }
}

fn create_message(base: &[u8], record: &DomainRecord) -> Result<Vec<u8>, &'static str> {
    let rtype = 0x01; // Hardcode IPv4 for now
    let rlen = 0x04; // ^

    let header = [ 0x80, 0, 0, 1, 0, 1, 0, 0, 0, 0 ];
    let answer = [ 0xC0, 0x0C, 0, rtype, 0, 1, 0, 0, 0x07, 0xD0, 0, rlen ];
    let mut message = base.to_vec();

    &message[2..12].copy_from_slice(&header);

    // Add the answer after the question
    message.extend_from_slice(&answer);

    // Append the IP address
    if let IpAddr::V4(v4) = record.dest {
        message.extend_from_slice(&v4.octets());
        return Ok(message);
    }
    
    return Err("Supplied IP address protocol is not currently supported");
}

fn main() {
    let master = UdpSocket::bind("127.0.0.1:53").unwrap();
    let slave = UdpSocket::bind("0.0.0.0:0").unwrap();

    let records = vec![
        DomainRecord { dest: "192.168.56.103".parse().unwrap(), domain: Regex::new(".+\\.dev\\.io").unwrap() },
    ];

    loop {
        let mut buffer = [0; 512];
        let (len, client) = master.recv_from(&mut buffer).unwrap();
        println!("received!");

        if let Some((msg, question)) = parse_message(&buffer[..len]) {
            if let Some(record) = records.iter().find(|r| r.is_match(&question.domain)) {
                // Found a matching wildcard, forward it on
                println!("returning match");
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

