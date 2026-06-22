use std::net::{IpAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

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

/// Host: periodically broadcast an announce packet on the LAN.
pub fn spawn_announce(name: String, tcp_port: u16) {
    thread::spawn(move || {
        let socket = match UdpSocket::bind(("0.0.0.0", 0)) {
            Ok(s) => s,
            Err(_) => return,
        };
        if socket.set_broadcast(true).is_err() {
            return;
        }
        let payload = ron::to_string(&Announce { name, tcp_port }).unwrap();
        let target = ("255.255.255.255", DISCOVERY_PORT);
        loop {
            let _ = socket.send_to(payload.as_bytes(), target);
            thread::sleep(Duration::from_millis(1000));
        }
    });
}

/// Client: listen for announce packets for `timeout`, return deduped lobby.
pub fn discover(timeout: Duration) -> Vec<LobbyEntry> {
    let mut entries = Vec::new();
    let socket = match UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)) {
        Ok(s) => s,
        Err(_) => return entries,
    };
    let _ = socket.set_read_timeout(Some(Duration::from_millis(250)));
    let deadline = Instant::now() + timeout;
    let mut buf = [0u8; 512];
    while Instant::now() < deadline {
        match socket.recv_from(&mut buf) {
            Ok((n, src)) => {
                if let Ok(a) = ron::from_str::<Announce>(&String::from_utf8_lossy(&buf[..n])) {
                    merge_announce(&mut entries, src.ip(), a);
                }
            }
            Err(_) => continue,
        }
    }
    entries
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
