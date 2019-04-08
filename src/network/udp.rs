use alloc::vec::Vec;
use smoltcp::iface::EthernetInterface;
use smoltcp::phy::Device;
use smoltcp::socket::*;
use smoltcp::wire::{IpEndpoint, Ipv4Address};
use smoltcp::time::{Duration, Instant};
use stm32f7_discovery::system_clock;

use super::{PortScan, PortScans};
use super::arp::ArpResponses;
use super::services::{Service, UDP_SERVICES};

pub fn probe_addresses<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    addrs: &ArpResponses,
) -> PortScans
where
    DeviceT: for<'d> Device<'d>,
{
    let mut ports = Vec::<PortScan>::new();
    let me = iface.ipv4_address().unwrap();
    for addr in addrs {
        let mut serv = Vec::<&Service>::new();
        let mut local_port = 49152;
        let mut handles: [(bool, Option<(Instant, SocketHandle, Ipv4Address, &Service)>); 10] =
            [(false, None); 10];
        let mut socket_count = 0;

        let mut serv_iter = UDP_SERVICES.iter();
        let mut iter_done = false;
        while !iter_done {
            let mut sockets = SocketSet::new(Vec::new());
            for i in 0..10 {
                let udp_rx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0; 64]);
                let udp_tx_buffer = UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY], vec![0; 128]);
                let udp_socket = UdpSocket::new(udp_rx_buffer, udp_tx_buffer);

                let udp_handle = sockets.add(udp_socket);
                let port = match serv_iter.next() {
                    Some(x) => x,
                    None => {
                        iter_done = true;
                        break;
                    }
                };
                {
                    let mut socket = sockets.get::<UdpSocket>(udp_handle);
                    socket.bind(IpEndpoint::new(me.into(), local_port)).unwrap();
                    local_port += 1;
                }
                handles[i] = (
                    false,
                    Some((
                        Instant::from_millis(system_clock::ms() as i64),
                        udp_handle,
                        addr.0,
                        port,
                    )),
                );
                socket_count += 1;
            }
            while socket_count > 0 {
                let timestamp = Instant::from_millis(system_clock::ms() as i64);
                match iface.poll(&mut sockets, timestamp) {
                    Ok(_) => {}
                    Err(_) => {}
                }
                for (done, opt) in handles.iter_mut() {
                    if *done {
                        continue;
                    }
                    if let Some(x) = opt {
                        let (stamp, handle, addr, port) = x;
                        let mut socket = sockets.get::<UdpSocket>(*handle);
                        if socket.can_send() {
                            socket.send_slice(b"", IpEndpoint::new((*addr).into(), port.0)).unwrap();
                        }
                        if socket.can_recv() {
                            serv.push(port);
                            socket_count -= 1;
                            *done = true;
                            *opt = None;
                        } else if timestamp - Duration::from_millis(100) > *stamp {
                            socket_count -= 1;
                            *done = true;
                            *opt = None;
                        }
                    }
                }
            }
        }
        ports.push(PortScan(addr.0, serv));
    }
    ports
}
