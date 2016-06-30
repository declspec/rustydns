use std::net::UdpSocket;
use std::mem;
use std::str;

struct DnsMessageHeader {
    id: u16,
    flags1: u8,
    flags2: u8,
    qd_count: u16,
    an_count: u16,
    ns_count: u16,
    ar_count: u16
}

fn mk_u16(buffer: &[u8], offset: usize) -> u16 {
    return ((buffer[offset] as u16) << 8) | buffer[offset + 1] as u16;
}

fn parse_dns_header(buffer: &[u8]) -> DnsMessageHeader {
    return DnsMessageHeader {
        id: mk_u16(buffer, 0),
        flags1: buffer[2],
        flags2: buffer[3],
        qd_count: mk_u16(buffer, 4),
        an_count: mk_u16(buffer, 6),
        ns_count: mk_u16(buffer, 8),
        ar_count: mk_u16(buffer, 10)
    };
}

fn parse_domain_labels(buffer: &[u8]) -> Vec<String> {
    let mut labels = Vec::new();
    let mut offset = 0;

    loop {
        match parse_domain_label(&buffer[offset..]) {
            None => break,
            Some(len, label) => { offset += len; labels.push(
    }

    labels
}

fn parse_domain_label(buffer: &[u8]) -> Option<(u8, &str)> {
    let len = buffer[0];
    
    match len {
        0 => None,
        1...64 => Some((len, str::from_utf8(&buffer[1..]).unwrap())),
        _ => None
    }
}

fn main() {
    let master = UdpSocket::bind("127.0.0.1:53").unwrap();
    let slave = UdpSocket::bind("0.0.0.0:0").unwrap();

    loop {
        let mut buffer = [0; 512];
        let (len, client) = master.recv_from(&mut buffer).unwrap();
        let header = parse_dns_header(&buffer[..len]);

        println!("id: {}, qd_count: {}", header.id, header.qd_count);

        slave.send_to(&buffer[..len], "8.8.8.8:53").expect("Failed to forward data to the Google DNS server");
        let (len, _) = slave.recv_from(&mut buffer).expect("Did not receive a response from the Google DNS server");

        master.send_to(&buffer[..len], client).expect("Failed to send response to client");
    }
}

