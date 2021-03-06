use alloc::vec::Vec;
use smoltcp::iface::EthernetInterface;
use smoltcp::phy::Device;
use smoltcp::socket::*;
use smoltcp::time::{Duration, Instant};
use stm32f7_discovery::system_clock;

use super::arp::ArpResponses;
use super::services::{Service, TCP_SERVICES};
use super::{PortScan, PortScans};

/// Starts a tcp port scan for every address in addrs
pub fn probe_addresses<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    addrs: &ArpResponses,
) -> PortScans
where
    DeviceT: for<'d> Device<'d>,
{
    let mut ports = Vec::<PortScan>::new();
    for addr in addrs {
        let mut serv = Vec::<&Service>::new();
        let mut local_port = 49152;
        let mut handles: [(bool, Option<(Instant, SocketHandle, &Service)>); 10] =
            [(false, None); 10];
        let mut socket_count = 0;

        let mut serv_iter = TCP_SERVICES.iter();
        let mut iter_done = false;
        while !iter_done {
            let mut sockets = SocketSet::new(Vec::new());
            // Limit amount of sockets to open simultaneously to prevent OOM
            for i in 0..10 {
                let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; 64]);
                let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; 128]);
                let tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);

                let tcp_handle = sockets.add(tcp_socket);
                let port = match serv_iter.next() {
                    Some(x) => x,
                    None => {
                        iter_done = true;
                        break;
                    }
                };
                {
                    let mut socket = sockets.get::<TcpSocket>(tcp_handle);
                    socket.connect((*addr.0, port.0), local_port).unwrap();
                    local_port += 1;
                }
                handles[i] = (
                    false,
                    Some((
                        Instant::from_millis(system_clock::ms() as i64),
                        tcp_handle,
                        port,
                    )),
                );
                socket_count += 1;
            }
            // Poll sockets until connection is established or they time out
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
                        let (stamp, handle, port) = x;
                        let mut socket = sockets.get::<TcpSocket>(*handle);
                        if socket.state() == TcpState::Established {
                            serv.push(port);
                            if socket.can_send() {
                                socket.close();
                            } else {
                                socket.abort();
                            }
                            socket_count -= 1;
                            *done = true;
                            *opt = None;
                        } else if timestamp - Duration::from_millis(100) > *stamp {
                            socket.abort();
                            socket_count -= 1;
                            *done = true;
                            *opt = None;
                        }
                    }
                }
            }
            // sockets.prune();
        }
        ports.push(PortScan(*addr.0, serv));
    }
    ports
}
