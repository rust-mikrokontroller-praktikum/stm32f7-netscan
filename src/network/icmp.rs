use alloc::vec::Vec;
use alloc::string::ToString;
use byteorder::{ByteOrder, NetworkEndian};
use managed::ManagedSlice;
use smoltcp::iface::EthernetInterface;
use smoltcp::phy::{Device, DeviceCapabilities};
use smoltcp::socket::*;
use smoltcp::time::{Duration, Instant};
use smoltcp::wire::{
    IpAddress, Ipv4Address, Icmpv4Repr, Icmpv4Packet,
    IpCidr,
};
use stm32f7_discovery::{
    ethernet::MTU,
    random, system_clock,
};

use super::cidr;

pub fn scan_v4<'b, 'c, 'e, DeviceT>(iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>, sockets: &mut SocketSet, rng: &mut random::Rng, cidr: &mut cidr::Ipv4Cidr) -> Vec<Ipv4Address> 
    where DeviceT: for<'d> Device<'d> {
    let mut found_addrs = Vec::<Ipv4Address>::new();
    let rx_buffer = IcmpSocketBuffer::new([IcmpPacketMetadata::EMPTY; 1], vec![0; 1500]);
    let tx_buffer = IcmpSocketBuffer::new([IcmpPacketMetadata::EMPTY; 1], vec![0; 3000]);
    let icmp_socket = IcmpSocket::new(rx_buffer, tx_buffer);

    let icmp_handle = sockets.add(icmp_socket);

    match iface.poll(sockets, Instant::from_millis(system_clock::ms() as i64)) {
        Ok(_) => {},
        Err(e) => {
            panic!("poll error: {}", e);
        },
    }

    iface.update_ip_addrs(|addrs| {
        let addr = IpAddress::from(cidr::to_ipv4_address(cidr.addr));
        *addrs = ManagedSlice::from(vec![IpCidr::new(addr, cidr.netmask); 1]);
    });

    cidr.reset();
    for addr in cidr {
        let address = cidr::to_ipv4_address(addr);
        if probe_v4(iface, rng, sockets, icmp_handle, address) {
            found_addrs.push(address);
        }
    }

    iface.update_ip_addrs(|addrs| {
        *addrs = ManagedSlice::from([IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0)]);
    });
    found_addrs
}

pub fn probe_v4<'b, 'c, 'e, DeviceT>(iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>, rng: &mut random::Rng, sockets: &mut SocketSet, handle: SocketHandle, addr: Ipv4Address) -> bool 
    where DeviceT: for<'d> Device<'d> {
    let start = Instant::from_millis(system_clock::ms() as i64);
    let mut send_at = Instant::from_millis(0);
    let mut seq_no = 0;
    let mut echo_payload = [0xffu8; 40];
    // let mut waiting_queue = HashMap::new();
    let gident = rng.poll_and_get().expect("RNG Failed") as u16;

    loop {
        let timestamp = Instant::from_millis(system_clock::ms() as i64);
        match iface.poll(sockets, timestamp) {
            Ok(_) => {},
            Err(e) => {
                panic!("poll error: {}", e);
            },
        }

        {
            let timestamp = Instant::from_millis(system_clock::ms() as i64);
            let mut socket = sockets.get::<IcmpSocket>(handle);
            if !socket.is_open(){
                socket.bind(IcmpEndpoint::Ident(gident)).unwrap();
                send_at = timestamp;
            }

            let can_send = socket.can_send();
            println!("can_send: {}, seq_no: {}, {}", can_send, seq_no, send_at <= timestamp);
            if can_send && seq_no < 4 as u16 && send_at <= timestamp {
                NetworkEndian::write_i64(&mut echo_payload, timestamp.total_millis());

                let icmp_repr = Icmpv4Repr::EchoRequest {
                    ident: gident,
                    seq_no: seq_no,
                    data: &echo_payload,
                };
                
                let icmp_payload = socket.send(icmp_repr.buffer_len(), IpAddress::from(addr)).unwrap();

                let mut icmp_packet = Icmpv4Packet::new_unchecked(icmp_payload);
                icmp_repr.emit(&mut icmp_packet, &capabilities().checksum);
                seq_no += 1;
                send_at += Duration::from_millis(10);
                println!("Sent ECHOREQ to {}", addr.to_string());
            }

            if socket.can_recv() {
                let (payload, _) = socket.recv().unwrap();

                let packet = Icmpv4Packet::new_checked(&payload).unwrap();
                let repr = Icmpv4Repr::parse(&packet, &capabilities().checksum).unwrap();

                if let Icmpv4Repr::EchoReply {ident, .. } = repr {
                    if ident == gident {
                        return true;
                    }
                }
            }

            if seq_no == 4 as u16 && timestamp - Duration::from_millis(50) > start {
                return false;
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
