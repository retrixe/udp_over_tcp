use std::{env, net::{TcpListener, TcpStream, UdpSocket, SocketAddr}, io::{Write, Read}};

/*
  TCP packet spec:
    - 4 bytes: packet body size (big endian)
    - 1 byte: IPv4 (4) or IPv6 (6)
    - 4 or 16 bytes: origin IP (big endian)
    - 2 bytes: origin port (big endian)
    - N bytes: packet data
*/

fn main() {
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
        let listener = TcpListener::bind(format!("127.0.0.1:{}", from_port)).unwrap();
        for stream in listener.incoming() {
            let stream = stream.unwrap();
            println!("Connection established!");
            handle_tcp_connection(&stream, to_port);
        }
    } else {
        // Start a UDP server, forward received packets to the TCP connection.
        let listener = UdpSocket::bind(format!("127.0.0.1:{}", from_port)).unwrap();
        let stream = TcpStream::connect(format!("127.0.0.1:{}", to_port)).unwrap();
        let mut buf = [0; 1024];
        
        // TODO: Use tokio/async-std?
        loop {
            match listener.recv_from(&mut buf) {
                Ok((size, origin)) => {
                    handle_udp_packet(&stream, buf, size, origin);
                },
                Err(e) => println!("Couldn't recieve a datagram: {}", e)
            }
        }
    }
}

fn handle_tcp_connection(mut stream: &TcpStream, to_port: u16) {
    // Read packets from the TCP connection.
    let mut buf = [0; 1024];
    let mut packet_size = 0;
    let mut packet_data = Vec::new();
    while let Ok(size) = stream.read(&mut buf) {
        if size == 0 {
            break;
        }
        packet_data.append(&mut buf[0..size].to_vec());
        if packet_size == 0 && packet_data.len() >= 4 {
            packet_size = u32::from_be_bytes([packet_data[0], packet_data[1], packet_data[2], packet_data[3]]) as usize;
            packet_data = packet_data[4..].to_vec();
        }
        if packet_data.len() >= packet_size && packet_size > 0 {
            handle_tcp_packet(packet_data[0..packet_size].to_vec(), to_port);
            if packet_data.len() > packet_size {
                packet_data = packet_data[packet_size..].to_vec();
            } else {
                packet_data = Vec::new();
            }
            packet_size = 0;
        }
    }
}

fn handle_tcp_packet(packet: Vec<u8>, to_port: u16) {
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
    println!("Forwarding packet to UDP receiver: {}", addr);
    println!("Packet body: {:?}", body);
    // TODO: Replace 7040 with randomised port-mapping.
    let socket = match UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 7040))) {
        Ok(s) => s,
        Err(e) => {
            println!("Failed to bind UDP socket: {}", e);
            return;
        }
    };
    match socket.send_to(&body, SocketAddr::from(([127, 0, 0, 1], to_port))) {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}

fn handle_udp_packet(mut stream: &TcpStream, buf: [u8; 1024], size: usize, origin: SocketAddr) {
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
