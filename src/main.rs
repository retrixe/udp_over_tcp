mod server;
mod client;

use std::{env, net::{TcpListener, TcpStream, UdpSocket}};

use client::handle_udp_packet;

use crate::server::handle_tcp_connection;

/*
TCP packet spec:
  - 4 bytes: packet body size (big endian)
  - 1 byte: IPv4 (4) or IPv6 (6)
  - 4 or 16 bytes: origin IP (big endian)
  - 2 bytes: origin port (big endian)
  - N bytes: packet data
*/

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 {
        println!("Usage: ./udp_over_tcp server|client <from_port> <to_port>");
        return;
    }
    let mode = args[1].clone();
    let from_port = args[2].clone().parse::<u16>().unwrap_or(0);
    let to_port = args[3].clone().parse::<u16>().unwrap_or(0);
    if (mode != "server" && mode != "client") || from_port < 1 || to_port < 1 {
        println!("Usage: ./udp_over_tcp server|client <from_port> <to_port>");
        return;
    }
    // TODO: Support two-way forwarding (currently only one-way)!
    if mode == "server" {
        // Start a TCP server, forward received packets to UDP receivers.
        let listener = TcpListener::bind(format!("127.0.0.1:{}", from_port)).unwrap();
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            println!("Connection established!");
            handle_tcp_connection(&stream, to_port);
        }
    } else {
        // Start a UDP server, forward received packets to the TCP connection.
        let listener = UdpSocket::bind(format!("127.0.0.1:{}", from_port)).unwrap();
        let stream = TcpStream::connect(format!("127.0.0.1:{}", to_port)).unwrap();
        let mut buf = [0; 1024];
        
        // TODO: Use tokio/async-std?
        loop {
            match listener.recv_from(&mut buf) {
                Ok((size, origin)) => {
                    handle_udp_packet(&stream, buf, size, origin);
                },
                Err(e) => println!("Couldn't recieve a datagram: {}", e)
            }
        }
    }
}
