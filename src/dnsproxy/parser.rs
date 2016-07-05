extern crate regex;

use std::str;
use dnsproxy::{ResourceRecord,NamePattern};
use self::regex::Regex;

struct RecordOptions {
    ttl: u32
}

impl ResourceRecord {
    pub fn from_str(s: &str) -> Result<ResourceRecord, &'static str> {
        lazy_static!{
            static ref RECORD_PATTERN: Regex = Regex::new(r"^\s+(?P<ip>(?:[0-9]{1,3}\.){3}[0-9]{1,3})\s+(?P<re>[^\s]+)(?:\s+\[(?P<o>[^\]]+)\])?").unwrap();
        }

        let captures = try!(RECORD_PATTERN.captures(s).ok_or("did not match record pattern"));
        let addr = try!(captures["ip"].parse().map_err(|_| "invalid ip address"));
        let regexp = try!(Regex::new(&captures["re"]).map_err(|_| "malformed regular expression"));
        let options = try!(parse_options(&captures["o"]));

        return Ok(ResourceRecord { rdata: addr, name: NamePattern::Regex(regexp), ttl: options.ttl });
    }
}

fn parse_options(ops: &str) -> Result<RecordOptions, &'static str> {
    let mut ttl = 1800; // Default TTL

    for raw in ops.split(',') {
        let (opt, val) = raw.find('=')
            .and_then(|pos| Some((&raw[..pos], &raw[pos+1..])))
            .unwrap_or((raw, ""));
        
        match opt {
            "ttl" => ttl = try!(parse_ttl(val)),
            _ => println!("info: skipping unrecognized option '{}'", opt)
        }
    }

    return Ok(RecordOptions { ttl: ttl });
}

fn parse_ttl(value: &str) -> Result<u32, &'static str> {
    let tmp = value.trim();

    match tmp.is_empty() {
        true => Err("missing value for ttl"),
        false => u32::from_str_radix(tmp, 10).map_err(|_| "invalid ttl value")
    }
}