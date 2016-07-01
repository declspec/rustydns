extern crate regex;

use std::str;
use std::net::UdpSocket;
use std::net::IpAddr;
use regex::Regex;

struct DomainRecord {
    dest: IpAddr,
    domain: Regex
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

fn parse_message(input: &[u8]) -> Option<(&[u8], String)> {
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

    return Some((&input[consumed + 1..], labels.join(".")));
}

fn create_answer(base: &[u8], record: &DomainRecord) -> [u8] {

}

fn main() {
    let master = UdpSocket::bind("127.0.0.1:53").unwrap();
    let slave = UdpSocket::bind("0.0.0.0:0").unwrap();

    let records = vec![
        DomainRecord { dest: "192.168.56.102".parse().unwrap(), domain: Regex::new(".+\\.dev\\.io").unwrap() },
    ];

    loop {
        let mut buffer = [0; 512];
        let (len, client) = master.recv_from(&mut buffer).unwrap();
        println!("received!");

        if let Some((msg, domain)) = parse_message(&buffer[..len]) {
            if let Some(record) = records.iter().find(|r| r.is_match(&domain)) {
                // Found a matching wildcard, forward it on
                let response = create_answer(&msg, &record);
                master.send_to(&response, client).expect("Failed to send response to client");
                continue;
            }
        }

        slave.send_to(&buffer[..len], "8.8.8.8:53").expect("Failed to forward data to the Google DNS server");
        let (len, _) = slave.recv_from(&mut buffer).expect("Did not receive a response from the Google DNS server");
        master.send_to(&buffer[..len], client).expect("Failed to send response to client");

    }
}

