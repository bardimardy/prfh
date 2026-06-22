use std::net::IpAddr;

use serde::{Deserialize, Serialize};

pub const TCP_PORT: u16 = 7777;
pub const DISCOVERY_PORT: u16 = 7778;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Announce {
    pub name: String,
    pub tcp_port: u16,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LobbyEntry {
    pub addr: IpAddr,
    pub name: String,
    pub tcp_port: u16,
}

/// Insert or update a lobby entry keyed by source IP (latest announce wins).
pub fn merge_announce(entries: &mut Vec<LobbyEntry>, addr: IpAddr, a: Announce) {
    if let Some(e) = entries.iter_mut().find(|e| e.addr == addr) {
        e.name = a.name;
        e.tcp_port = a.tcp_port;
    } else {
        entries.push(LobbyEntry { addr, name: a.name, tcp_port: a.tcp_port });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn announce_ron_roundtrip() {
        let a = Announce { name: "Hostspiel".into(), tcp_port: TCP_PORT };
        let s = ron::to_string(&a).unwrap();
        let back: Announce = ron::from_str(&s).unwrap();
        assert_eq!(a, back);
    }

    #[test]
    fn merge_dedups_by_ip() {
        let ip: IpAddr = Ipv4Addr::new(192, 168, 1, 5).into();
        let mut v = Vec::new();
        merge_announce(&mut v, ip, Announce { name: "A".into(), tcp_port: 7777 });
        merge_announce(&mut v, ip, Announce { name: "A2".into(), tcp_port: 7777 });
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].name, "A2");
    }

    #[test]
    fn merge_keeps_distinct_ips() {
        let mut v = Vec::new();
        merge_announce(&mut v, Ipv4Addr::new(192, 168, 1, 5).into(), Announce { name: "A".into(), tcp_port: 7777 });
        merge_announce(&mut v, Ipv4Addr::new(192, 168, 1, 6).into(), Announce { name: "B".into(), tcp_port: 7777 });
        assert_eq!(v.len(), 2);
    }
}
