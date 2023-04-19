use std::{net::SocketAddr, collections::HashMap, sync::{Mutex, Arc}};

use tokio::{net::{TcpStream, UdpSocket}, io::AsyncReadExt};

use crate::packets;

type Db = Arc<Mutex<HashMap<String, Arc<UdpSocket>>>>;

pub async fn handle_tcp_connection_read(mut stream: TcpStream, to_port: u16) {
    // Create a common storage for port-mappings.
    // TODO: Preserve these mappings across connections.
    // TODO: Make this an LRU cache, set a limit and discard old port-mappings.
    let db: Db = Arc::new(Mutex::new(HashMap::new()));

    // Read packets from the TCP connection.
    let mut buf = [0; 1024];
    let mut packet_size = 0;
    let mut packet_data = Vec::new();
    while let Ok(size) = stream.read(&mut buf).await {
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
            handle_tcp_packet(db.clone(), packet_data[0..packet_size].to_vec(), to_port).await;
            // If the buffer is larger than the packet size, read the next packet.
            if packet_data.len() > packet_size {
                packet_data = packet_data[packet_size..].to_vec();
            } else {
                packet_data = Vec::new();
            }
            // Reset the packet size (queue a new packet read).
            packet_size = 0;
        }
    }
}

async fn handle_tcp_packet(db: Db, packet: Vec<u8>, to_port: u16) {
    // Forward received packets to the UDP receivers.
    let (addr, body) = match packets::decode_udp_packet(packet) {
        Ok((a, b)) => (a, b),
        Err(e) => {
            println!("Failed to decode packet: {}", e);
            return;
        }
    };
    // TODO: Some apps may want a well-known port, and not have their UDP port
    // changed to an ephemeral one. How do we handle this?
    let mut sockets = db.lock().unwrap();
    let socket = match sockets.get(&addr.to_string()) {
        Some(p) => p.clone(),
        None => {
            let socket = match UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    println!("Failed to bind UDP socket: {}", e);
                    return;
                }
            };
            sockets.insert(addr.to_string(), socket.clone());
            socket
        },
    };
    match socket.send_to(&body, SocketAddr::from(([127, 0, 0, 1], to_port))).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}
