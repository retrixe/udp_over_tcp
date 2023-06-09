use std::{net::SocketAddr, collections::HashMap, sync::Arc};

use tokio::{net::{TcpStream, UdpSocket, tcp::OwnedWriteHalf}, io::{AsyncReadExt, AsyncWriteExt}, sync::{Mutex, mpsc::{self, Sender}}};

use crate::packets;

type Db = Arc<Mutex<HashMap<String, Sender<(SocketAddr, Vec<u8>)>>>>;

pub async fn handle_tcp_connection_read(
    stream: TcpStream, to_port: u16, disable_port_remapping: bool
) {
    // Create a common storage for port-mappings.
    // TODO: Preserve these mappings across connections.
    // TODO: Make this an LRU cache, set a limit and discard old port-mappings.
    let db: Db = Arc::new(Mutex::new(HashMap::new()));

    // Split the TCP stream into a reader and a writer.
    let (mut reader, writer) = stream.into_split();
    // Arc allows multiple ownership, Mutex prevents multiple writes.
    let writer_arc = Arc::new(Mutex::new(writer));

    // Read packets from the TCP connection.
    let mut buf = [0; 65535];
    let mut packet_size = 0;
    let mut packet_data = Vec::new();
    loop {
        match reader.read(&mut buf).await {
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
                    tokio::spawn(handle_tcp_packet(
                        db.clone(), packet_data[0..packet_size].to_vec(), to_port,
                        writer_arc.clone(), disable_port_remapping
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
            },
            Err(err) => {
                println!("Failed to read from TCP connection, exiting: {}", err);
                break;
            }
        }
    }
}

async fn handle_tcp_packet(
    db: Db, packet: Vec<u8>, to_port: u16, tcp_writer: Arc<Mutex<OwnedWriteHalf>>,
    disable_port_remapping: bool
) {
    // Forward received packets to the UDP receivers.
    let (addr, body) = match packets::decode_udp_packet(packet) {
        Ok((a, b)) => (a, b),
        Err(e) => {
            println!("Failed to decode packet: {}", e);
            return;
        }
    };
    let mut sockets = db.lock().await;
    let socket = match sockets.get(&addr.to_string()) {
        Some(p) => p.clone(),
        None => { // Accept connections back only from localhost for security reasons.
            // If port remapping is disabled, listen on the same port as the client.
            let port = if disable_port_remapping { to_port } else { 0 };
            let socket = match UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port))).await {
                Ok(s) => Arc::new(s),
                Err(e) => {
                    println!("Failed to bind UDP socket on port {}: {}", port, e);
                    return;
                }
            };
            let socket_r = socket.clone();
            // TODO: Implement channels to get rid of these when we have an LRU.
            tokio::spawn(async move {
                let mut buf = [0; 65535];
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
            let (tx, mut rx) = mpsc::channel::<(SocketAddr, Vec<u8>)>(32);
            let socket_w = socket.clone();
            tokio::spawn(async move {
                loop {
                    let (addr, buf) = rx.recv().await.unwrap();
                    match socket_w.send_to(&buf, addr).await {
                        Ok(_) => {},
                        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
                    }
                }
            });
            sockets.insert(addr.to_string(), tx.clone());
            tx
        },
    };
    match socket.send((SocketAddr::from(([127, 0, 0, 1], to_port)), body)).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}
