use std::{net::{TcpStream, SocketAddr}, io::{Write}};

pub fn handle_udp_packet(mut stream: &TcpStream, buf: [u8; 1024], size: usize, origin: SocketAddr) {
    // Forward received packets to the TCP connection.
    let mut packet_data = [0; 1].to_vec();
    match origin {
        SocketAddr::V4(ip) => {
            packet_data[0] = 4;
            packet_data.append(&mut ip.ip().octets().to_vec());
        },
        SocketAddr::V6(ip) => {
            packet_data[0] = 6;
            packet_data.append(&mut ip.ip().octets().to_vec());
        },
    }
    packet_data.append(&mut origin.port().to_be_bytes().to_vec());
    packet_data.append(&mut buf[0..size].to_vec());

    let mut packet = (packet_data.len() as u32).to_be_bytes().to_vec();
    packet.append(&mut packet_data);
    match stream.write_all(&packet) {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to TCP server: {}", e),
    }
}
