mod client;
mod packets;
mod server;

use std::env;

use tokio::net::{TcpListener, TcpStream, UdpSocket};

#[tokio::main]
async fn main() {
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
        let listener = TcpListener::bind(format!("127.0.0.1:{}", from_port)).await.unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            println!("Connection established!");
            server::handle_tcp_connection_read(stream, to_port).await;
        }
    } else {
        // Start a UDP server, forward received packets to the TCP connection.
        let listener = UdpSocket::bind(format!("127.0.0.1:{}", from_port)).await.unwrap();
        let mut stream = TcpStream::connect(format!("127.0.0.1:{}", to_port)).await.unwrap();
        let mut buf = [0; 1024];

        loop {
            match listener.recv_from(&mut buf).await {
                Ok((size, origin)) => {
                    client::handle_udp_packet(&mut stream, buf, size, origin).await;
                },
                Err(e) => println!("Couldn't recieve a datagram: {}", e)
            }
        }
    }
}
