use std::net::SocketAddr;

use tokio::{net::{TcpStream, UdpSocket}, io::AsyncReadExt};

pub async fn handle_tcp_connection(mut stream: TcpStream, to_port: u16) {
    // Read packets from the TCP connection.
    let mut buf = [0; 1024];
    let mut packet_size = 0;
    let mut packet_data = Vec::new();
    while let Ok(size) = stream.read(&mut buf).await {
        if size == 0 {
            break;
        }
        packet_data.append(&mut buf[0..size].to_vec());
        if packet_size == 0 && packet_data.len() >= 4 {
            packet_size = u32::from_be_bytes([packet_data[0], packet_data[1], packet_data[2], packet_data[3]]) as usize;
            packet_data = packet_data[4..].to_vec();
        }
        if packet_data.len() >= packet_size && packet_size > 0 {
            handle_tcp_packet(packet_data[0..packet_size].to_vec(), to_port).await;
            if packet_data.len() > packet_size {
                packet_data = packet_data[packet_size..].to_vec();
            } else {
                packet_data = Vec::new();
            }
            packet_size = 0;
        }
    }
}

async fn handle_tcp_packet(packet: Vec<u8>, to_port: u16) {
    // Forward received packets to the UDP receivers.
    let ip_version = packet[0];
    let mut body = packet[1..].to_vec();
    let addr = match ip_version {
        4 => {
            let ip = [body[0], body[1], body[2], body[3]];
            let port = u16::from_be_bytes([body[4], body[5]]);
            body = body[6..].to_vec();
            SocketAddr::from(([ip[0], ip[1], ip[2], ip[3]], port))
        },
        6 => {
            let mut ip = [0; 16];
            for i in 0..16 {
                ip[i] = body[i];
            }
            let port = u16::from_be_bytes([body[16], body[17]]);
            body = body[18..].to_vec();
            SocketAddr::from((ip, port))
        },
        _ => {
            println!("Invalid IP version: {}", ip_version);
            return;
        }
    };
    println!("Forwarding packet to UDP receiver: {}", addr); // TODO: Remove this.
    println!("Packet body: {:?}", body);
    // TODO: Replace 7040 with randomised port-mapping.
    let socket = match UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 7040))).await {
        Ok(s) => s,
        Err(e) => {
            println!("Failed to bind UDP socket: {}", e);
            return;
        }
    };
    match socket.send_to(&body, SocketAddr::from(([127, 0, 0, 1], to_port))).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}
