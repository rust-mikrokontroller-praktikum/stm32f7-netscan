use alloc::string::{String, ToString};
use alloc::vec::Vec;
use byteorder::{ByteOrder, NetworkEndian};
use smoltcp::iface::EthernetInterface;
use smoltcp::phy::{Device, DeviceCapabilities};
use smoltcp::socket::*;
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{Icmpv4Packet, Icmpv4Repr, IpAddress, Ipv4Address};
use stm32f7_discovery::{ethernet::MTU, random, system_clock};

use super::arp::ArpResponses;

#[derive(Debug)]
pub struct IcmpResponse(pub Ipv4Address, pub Duration);
pub type IcmpResponses = Vec<IcmpResponse>;

impl super::StringableVec for IcmpResponses {
    fn to_string_vec(&self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        for i in self.iter() {
            ret.push(format!("{} ({})", i.0, i.1));
        }
        ret
    }
}

pub fn scan_v4<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    rng: &mut random::Rng,
    addrs: &ArpResponses,
) -> IcmpResponses
where
    DeviceT: for<'d> Device<'d>,
{
    let mut found_addrs = Vec::<IcmpResponse>::new();

    for addr in addrs {
        if let Some(x) = probe_v4(iface, rng, *addr.0) {
            found_addrs.push(IcmpResponse(*addr.0, x));
        }
    }

    found_addrs
}

pub fn probe_v4<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    rng: &mut random::Rng,
    addr: Ipv4Address,
) -> Option<Duration>
where
    DeviceT: for<'d> Device<'d>,
{
    let start = Instant::from_millis(system_clock::ms() as i64);
    let mut send_at = Instant::from_millis(0);
    let mut seq_no = 0;
    let mut echo_payload = [0xffu8; 40];
    let mut sockets = SocketSet::new(Vec::new());
    let gident = rng.poll_and_get().expect("RNG Failed") as u16;

    let rx_buffer = IcmpSocketBuffer::new([IcmpPacketMetadata::EMPTY; 1], vec![0; 1500]);
    let tx_buffer = IcmpSocketBuffer::new([IcmpPacketMetadata::EMPTY; 1], vec![0; 3000]);
    let icmp_socket = IcmpSocket::new(rx_buffer, tx_buffer);
    let icmp_handle = sockets.add(icmp_socket);

    match iface.poll(
        &mut sockets,
        Instant::from_millis(system_clock::ms() as i64),
    ) {
        Ok(_) => {}
        Err(_) => {}
    }

    loop {
        let timestamp = Instant::from_millis(system_clock::ms() as i64);
        match iface.poll(&mut sockets, timestamp) {
            Ok(_) => {}
            Err(_) => {}
        }

        {
            let timestamp = Instant::from_millis(system_clock::ms() as i64);
            let mut socket = sockets.get::<IcmpSocket>(icmp_handle);
            if !socket.is_open() {
                socket.bind(IcmpEndpoint::Ident(gident)).unwrap();
                send_at = timestamp;
            }

            let can_send = socket.can_send();
            // println!("can_send: {}, seq_no: {}, {}", can_send, seq_no, send_at <= timestamp);
            if can_send && seq_no < 4 as u16 && send_at <= timestamp {
                NetworkEndian::write_i64(&mut echo_payload, timestamp.total_millis());

                let icmp_repr = Icmpv4Repr::EchoRequest {
                    ident: gident,
                    seq_no,
                    data: &echo_payload,
                };

                let icmp_payload = socket
                    .send(icmp_repr.buffer_len(), IpAddress::from(addr))
                    .unwrap();

                let mut icmp_packet = Icmpv4Packet::new_unchecked(icmp_payload);
                icmp_repr.emit(&mut icmp_packet, &capabilities().checksum);
                seq_no += 1;
                send_at += Duration::from_millis(10);
                // println!("Sent ECHOREQ to {}", addr.to_string());
            }

            if socket.can_recv() {
                let (payload, _) = socket.recv().unwrap();

                let packet = Icmpv4Packet::new_checked(&payload).unwrap();
                let repr = Icmpv4Repr::parse(&packet, &capabilities().checksum).unwrap();

                if let Icmpv4Repr::EchoReply { ident, data, .. } = repr {
                    if ident == gident {
                        // println!("Found address: {}", addr);
                        return Some(
                            timestamp - Instant::from_millis(NetworkEndian::read_i64(data)),
                        );
                    }
                }
            }

            // println!("timestamp: {}, start: {}", timestamp, start);
            if seq_no == 4 as u16 && timestamp - Duration::from_millis(500) > start {
                return None;
            }
            system_clock::wait_ms(10);
        }
    }
}

fn capabilities() -> DeviceCapabilities {
    let mut capabilities = DeviceCapabilities::default();
    capabilities.max_transmission_unit = MTU;
    capabilities
}
