use std::{net::SocketAddr};

use tokio::{net::{tcp::{OwnedReadHalf, OwnedWriteHalf}}, io::{AsyncWriteExt, AsyncReadExt}, sync::mpsc::Sender};

use crate::packets;

type UdpWriteChannel = Sender<(SocketAddr, Vec<u8>)>;

pub async fn handle_udp_packet(
    stream: &mut OwnedWriteHalf, buf: [u8; 65535], size: usize, origin: SocketAddr
) {
    // Forward received packets to the TCP connection.
    let packet = packets::encode_udp_packet(buf, size, origin);
    match stream.write_all(&packet).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to TCP server: {}", e),
    }
}

pub async fn handle_tcp_connection_read(stream: &mut OwnedReadHalf, write_tx: UdpWriteChannel) {
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
                    // Parse the received TCP packet and send it to the UDP write thread.
                    tokio::spawn(handle_tcp_packet(
                        packet_data[0..packet_size].to_vec(), write_tx.clone()));
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

async fn handle_tcp_packet(packet: Vec<u8>, write_channel: UdpWriteChannel) {
    // Forward received packets to the UDP receivers.
    let (addr, body) = match packets::decode_udp_packet(packet) {
        Ok((a, b)) => (a, b),
        Err(e) => {
            println!("Failed to decode packet: {}", e);
            return;
        }
    };
    match write_channel.send((addr, body)).await {
        Ok(_) => {},
        Err(e) => println!("Failed to send packet to UDP receiver: {}", e),
    }
}
