use std::net::SocketAddr;

use tokio::{net::TcpStream, io::AsyncWriteExt};

use crate::packets;

pub async fn handle_udp_packet(stream: &mut TcpStream, buf: [u8; 1024], size: usize, origin: SocketAddr) {
    // Forward received packets to the TCP connection.
    let packet = packets::encode_client_udp_packet(buf, size, origin);
    match stream.write_all(&packet).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to TCP server: {}", e),
    }
}
