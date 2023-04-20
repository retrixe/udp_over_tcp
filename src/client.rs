use std::{net::SocketAddr, sync::Arc};

use tokio::{net::{UdpSocket, TcpStream}, io::{AsyncWriteExt, AsyncReadExt, ReadHalf, WriteHalf}};

use crate::packets;

pub async fn handle_udp_packet(
    stream: &mut WriteHalf<TcpStream>, buf: [u8; 65535], size: usize, origin: SocketAddr
) {
    // Forward received packets to the TCP connection.
    let packet = packets::encode_udp_packet(buf, size, origin);
    match stream.write_all(&packet).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to TCP server: {}", e),
    }
}

pub async fn handle_tcp_connection_read(stream: &mut ReadHalf<TcpStream>, socket: Arc<UdpSocket>) {
    // Read from the TCP connection, forward datagrams back to UDP clients.
    let mut buf = [0; 65535];
    let mut packet_size = 0;
    let mut packet_data = Vec::new();
    loop {
        match stream.read(&mut buf).await {
            Ok(size) => {
                if size == 0 {
                    break;
                }
                packet_data.append(&mut buf[0..size].to_vec());
                // If no packet read is queued, and the buffer is 4 bytes long, read the packet size.
                if packet_size == 0 && packet_data.len() >= 4 {
                    packet_size = u32::from_be_bytes([packet_data[0], packet_data[1], packet_data[2], packet_data[3]]) as usize;
                    packet_data = packet_data[4..].to_vec();
                }
                // If a packet read is queued, and the buffer is at least the packet size, read the packet.
                if packet_data.len() >= packet_size && packet_size > 0 {
                    // TODO: This is fine only if UdpSocket::send_to is thread-safe, else,
                    // to maximise throughput, we should use an actor.
                    tokio::spawn(handle_tcp_packet(
                        packet_data[0..packet_size].to_vec(), socket.clone()));
                    // If the buffer is larger than the packet size, read the next packet.
                    if packet_data.len() > packet_size {
                        packet_data = packet_data[packet_size..].to_vec();
                    } else {
                        packet_data = Vec::new();
                    }
                    // Reset the packet size (queue a new packet read).
                    packet_size = 0;
                }
            },
            Err(err) => {
                println!("Failed to read from TCP connection, exiting: {}", err);
                break;
            },
        }
    }
}

async fn handle_tcp_packet(packet: Vec<u8>, socket: Arc<UdpSocket>) {
    // Forward received packets to the UDP receivers.
    let (addr, body) = match packets::decode_udp_packet(packet) {
        Ok((a, b)) => (a, b),
        Err(e) => {
            println!("Failed to decode packet: {}", e);
            return;
        }
    };
    match socket.send_to(&body, addr).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}
