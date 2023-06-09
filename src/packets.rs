use std::net::SocketAddr;

/*
TCP client->server packet spec:
  - 4 bytes: packet body size (big endian)
  - 1 byte: IPv4 (4) or IPv6 (6)
  - 4 or 16 bytes: origin IP (big endian)
  - 2 bytes: origin port (big endian)
  - N bytes: packet data

TCP server->client packet spec:
  - 4 bytes: packet body size (big endian)
  - 1 byte: IPv4 (4) or IPv6 (6)
  - 4 or 16 bytes: destination IP (big endian)
  - 2 bytes: destination port (big endian)
  - N bytes: packet data
*/

pub fn encode_udp_packet(buf: [u8; 65535], size: usize, addr: SocketAddr) -> Vec<u8> {
    let mut packet_data = [0; 1].to_vec();
    match addr {
        SocketAddr::V4(ip) => {
            packet_data[0] = 4;
            packet_data.append(&mut ip.ip().octets().to_vec());
        },
        SocketAddr::V6(ip) => {
            packet_data[0] = 6;
            packet_data.append(&mut ip.ip().octets().to_vec());
        },
    }
    packet_data.append(&mut addr.port().to_be_bytes().to_vec());
    packet_data.append(&mut buf[0..size].to_vec());
    let mut packet = (packet_data.len() as u32).to_be_bytes().to_vec();
    packet.append(&mut packet_data);
    return packet;
}

pub fn decode_udp_packet(packet: Vec<u8>) -> Result<(SocketAddr, Vec<u8>), String> {
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
        _ => return Err(format!("Invalid IP version: {}", ip_version)),
    };
    return Ok((addr, body));
}
