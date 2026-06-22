use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};

use prfh::net::protocol::{decode_line, encode_line, ClientMsg, ServerMsg};
use prfh::net::server::{spawn_listener, HostEvent, HostState};

#[test]
fn client_hello_gets_welcome() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let rx = spawn_listener(listener);

    let mut host = HostState::new("Host".into());

    // Client connects and says hello.
    let mut client = TcpStream::connect(addr).unwrap();
    client
        .write_all(encode_line(&ClientMsg::Hello { name: "Bob".into() }).as_bytes())
        .unwrap();

    // Host receives Hello, assigns a player, replies Welcome.
    let event = rx.recv().unwrap();
    match event {
        HostEvent::Hello { name, mut write, .. } => {
            assert_eq!(name, "Bob");
            let outcome = host.add_player(name).unwrap();
            write
                .write_all(encode_line(&outcome.welcome).as_bytes())
                .unwrap();
        }
        _ => panic!("expected Hello"),
    }

    // Client reads the Welcome line.
    let mut reader = BufReader::new(client);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let msg: ServerMsg = decode_line(&line).unwrap();
    match msg {
        ServerMsg::Welcome { your_id, .. } => assert_eq!(your_id, 1),
        _ => panic!("expected Welcome"),
    }
}
