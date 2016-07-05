#[macro_use]
extern crate lazy_static;
extern crate getopts;

mod dnsproxy;

use std::str;
use std::io::{BufRead,BufReader};
use std::fs::File;
use std::path::Path;
use std::net::SocketAddr;
use getopts::{Options,HasArg,Occur};
use dnsproxy::{UdpRelay,DnsInterceptor,ResourceRecord};

fn parse_dns_record(line: &str) -> Option<ResourceRecord> {
    let trimmed = line.trim();

    if !trimmed.starts_with("#@") {
        return None;
    }

    return match ResourceRecord::from_str(&trimmed[2..]) {
        Ok(record) => Some(record),
        Err(reason) => {
            println!("warning: skipping '{}' ({})", trimmed[2..].trim(), reason);
            None
        }
    }
}

fn parse_dns_records<P: AsRef<Path>>(path: P) -> Result<Vec<ResourceRecord>, &'static str> {
    return File::open(path)
        .map_err(|_| "failed to open host file, check that the file exists and that it is accessible")
        .and_then(|f| 
            Ok(BufReader::new(f).lines()
                .filter_map(|lv| lv.ok().and_then(|line| parse_dns_record(&line)))
                .collect())
        );
}

fn run(args: &[String]) -> Result<(), String> {
    let mut opts = Options::new();

    opts.opt("h", "hosts-file", "path to the hosts file which contains custom DNS records", "FILE", HasArg::Yes, Occur::Req)
        .opt("s", "server", "ip:port of the real DNS server to relay unfulfilled requests to", "IP:PORT", HasArg::Yes, Occur::Req)
        .optopt("a", "local", "ip:port to listen to on the local machine", "IP:PORT");
    
    let options = try!(opts.parse(args).map_err(|e| e.to_string()));
    let hostfile = options.opt_str("h").expect("missing FILE argument");
    let server = options.opt_str("s").expect("missing SERVER argument");

    let addr = try!(server.parse::<SocketAddr>().map_err(|_| "server is not a valid ip:port string".to_string()));
    let records = try!(parse_dns_records(Path::new(&hostfile)));
    
    println!("info: proxy running with {} custom dns record(s)", records.len());

    let relay = UdpRelay::new(addr, DnsInterceptor::new(records));
    let local = options.opt_str("a").unwrap_or("127.0.0.1:53".to_string());
    let local = try!(local.parse::<SocketAddr>().map_err(|_| "local address is not a valid ip:port string".to_string()));

    return relay.listen(local, 512).map_err(|e| e.to_string());
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    match run(&args[1..]) {
        Err(reason) => println!("error: {}", reason),
        _ => ()
    }
}

