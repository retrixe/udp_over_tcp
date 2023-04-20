use std::{net::SocketAddr, collections::HashMap, sync::Arc};

use tokio::{net::{TcpStream, UdpSocket}, io::{AsyncReadExt, WriteHalf, AsyncWriteExt}, sync::Mutex};

use crate::packets;

type Db = Arc<Mutex<HashMap<String, Arc<UdpSocket>>>>;

pub async fn handle_tcp_connection_read(stream: TcpStream, to_port: u16) {
    // Create a common storage for port-mappings.
    // LOW-TODO: Preserve these mappings across connections.
    // LOW-TODO: Make this an LRU cache, set a limit and discard old port-mappings.
    let db: Db = Arc::new(Mutex::new(HashMap::new()));

    // Split the TCP stream into a reader and a writer.
    let (mut reader, writer) = tokio::io::split(stream);
    let writer_arc = Arc::new(Mutex::new(writer)); // Mutex prevents multiple writes.

    // Read packets from the TCP connection.
    let mut buf = [0; 1024];
    let mut packet_size = 0;
    let mut packet_data = Vec::new();
    // TODO: No error handling.
    while let Ok(size) = reader.read(&mut buf).await {
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
                db.clone(),
                packet_data[0..packet_size].to_vec(),
                to_port,
                writer_arc.clone()
            ));
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

async fn handle_tcp_packet(
    db: Db, packet: Vec<u8>, to_port: u16, tcp_writer: Arc<Mutex<WriteHalf<TcpStream>>>
) {
    // Forward received packets to the UDP receivers.
    let (addr, body) = match packets::decode_udp_packet(packet) {
        Ok((a, b)) => (a, b),
        Err(e) => {
            println!("Failed to decode packet: {}", e);
            return;
        }
    };
    // LOW-TODO: Some apps may want a well-known port, and not have their UDP port
    // changed to an ephemeral one. How do we handle this?
    // LOW-TODO: We could make this locking mechanism slightly more efficient.
    let mut sockets = db.lock().await;
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
            let socket_r = socket.clone();
            // LOW-TODO: Implement channels to get rid of these when we have an LRU.
            tokio::spawn(async move {
                let mut buf = [0; 1024];
                loop {
                    match socket_r.recv_from(&mut buf).await {
                        Ok((size, _)) => {
                            let packet = packets::encode_udp_packet(buf, size, addr);
                            match tcp_writer.lock().await.write_all(&packet).await {
                                Ok(_) => {},
                                Err(e) => println!("Failed to send packet to TCP sender: {}", e),
                            }
                        },
                        Err(e) => println!("Failed to receive packet from UDP sender: {}", e),
                    }
                }
            });
            sockets.insert(addr.to_string(), socket.clone());
            socket
        },
    };
    match socket.send_to(&body, SocketAddr::from(([127, 0, 0, 1], to_port))).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}
