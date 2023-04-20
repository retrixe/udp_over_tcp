mod client;
mod packets;
mod server;

use std::{env, sync::Arc};

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
    if mode == "server" {
        // Start a TCP server, forward received packets to UDP receivers.
        // Internally, 2-way communication is implemented where UDP reader threads are spawned.
        // Accept connections back only from localhost for security reasons.
        let listener = TcpListener::bind(format!("127.0.0.1:{}", from_port)).await.unwrap();
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            println!("Connection established!");
            server::handle_tcp_connection_read(stream, to_port).await;
        }
    } else {
        // Start a UDP server, forward received packets to the TCP connection.
        let socket = Arc::new(UdpSocket::bind(format!("0.0.0.0:{}", from_port)).await.unwrap());
        let stream = TcpStream::connect(format!("127.0.0.1:{}", to_port)).await.unwrap();
        let (mut read_stream, mut write_stream) = tokio::io::split(stream);

        // Spawn UDP read thread.
        let socket_r = socket.clone();
        tokio::spawn(async move {
            let mut buf = [0; 65535];
            loop {
                match socket_r.recv_from(&mut buf).await {
                    Ok((size, origin)) => {
                        client::handle_udp_packet(&mut write_stream, buf, size, origin).await;
                    },
                    Err(e) => println!("Couldn't recieve a datagram: {}", e)
                }
            }
        });

        // Begin reading from the TCP connection for data to send back.
        // UDP recv errors don't bring the app down, so no point threading this and using channels.
        client::handle_tcp_connection_read(&mut read_stream, socket).await;
    }
}
