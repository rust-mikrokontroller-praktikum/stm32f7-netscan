use alloc::vec::Vec;
use alloc::string::{String, ToString};
use smoltcp::phy::{Device, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::*;
use stm32f7_discovery::{
    ethernet::EthernetDevice,
    system_clock,
};
use super::cidr;

#[derive(Debug)]
pub struct ArpResponse(Ipv4Address, EthernetAddress);

pub fn request(iface: &mut EthernetDevice, eth_addr: EthernetAddress, addr: Ipv4Address) -> Result<(), String> {
    let mut arp_req = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: eth_addr,
        source_protocol_addr: Ipv4Address::new(0, 0, 0, 0),
        target_hardware_addr: EthernetAddress::BROADCAST,
        target_protocol_addr: addr
    };

    let mut buffer = vec![0; arp_req.buffer_len()];
    let mut packet = ArpPacket::new_unchecked(&mut buffer);
    arp_req.emit(&mut packet);

    let tx_token = match iface.transmit() {
        Some(x) => x,
        None => return Err(String::from("No tx descriptor available")),
    };
    match dispatch_ethernet(eth_addr, tx_token, Instant::from_millis(system_clock::ms() as i64), arp_req.buffer_len(), |mut frame| {
        frame.set_dst_addr(EthernetAddress::BROADCAST);
        frame.set_ethertype(EthernetProtocol::Arp);

        let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
        arp_req.emit(&mut packet);
    }) {
        Ok(x) => Ok(x),
        Err(x) => return Err(x.to_string()),
    }
}

pub fn get_neighbors_v4(iface: &mut EthernetDevice, eth_addr: EthernetAddress, cidr: &mut cidr::Ipv4Cidr) -> Result<Vec<ArpResponse>, String> {
    let mut found_addrs = Vec::<ArpResponse>::new();
    let mut arp_req = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: eth_addr,
        source_protocol_addr: Ipv4Address::new(0, 0, 0, 0),
        target_hardware_addr: EthernetAddress::BROADCAST,
        target_protocol_addr: Ipv4Address::new(0, 0, 0, 0)
    };
    cidr.reset();
    for addr in cidr {
        if let ArpRepr::EthernetIpv4{ operation: _, source_hardware_addr: _, source_protocol_addr: _, target_hardware_addr: _, target_protocol_addr: ref mut y } = arp_req {
                *y = cidr::to_ipv4_address(addr);
        }
        let mut buffer = vec![0; arp_req.buffer_len()];
        let mut packet = ArpPacket::new_unchecked(&mut buffer);
        arp_req.emit(&mut packet);

        let tx_token = match iface.transmit() {
            Some(x) => x,
            None => return Err(String::from("No tx descriptor available")),
        };
        match dispatch_ethernet(eth_addr, tx_token, Instant::from_millis(system_clock::ms() as i64), arp_req.buffer_len(), |mut frame| {
            frame.set_dst_addr(EthernetAddress::BROADCAST);
            frame.set_ethertype(EthernetProtocol::Arp);

            let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
            arp_req.emit(&mut packet);
        }) {
            Ok(x) => x,
            Err(x) => return Err(x.to_string()),
        }

    }
    let mut tries = 0;
    loop {
        let (rx_token, _) = match iface.receive() {
            None => {
                if tries > 100 {
                    // println!("Didn't receive answers to ARP");
                    break;
                }
                tries += 1;
                // system_clock::wait_ms(100);
                continue
            },
            Some(tokens) => tokens,
        };
        match rx_token.consume(Instant::from_millis(system_clock::ms() as i64), |frame| {
            match process_arp(eth_addr, &frame) {
                Ok(x) => {
                    return Ok(x);
                },
                Err(x) => return Err(x),
            };}) {
            Ok(ArpRepr::EthernetIpv4{source_hardware_addr, source_protocol_addr, .. }) => found_addrs.push(ArpResponse(source_protocol_addr, source_hardware_addr)),
            Ok(_) => {},
            Err(e) => println!("{:?}", e),
        };
        }
    return Ok(found_addrs);
}

fn dispatch_ethernet<Tx, F>(eth_addr: EthernetAddress, tx_token: Tx, timestamp: Instant,
                            buffer_len: usize, f: F) -> Result<(), smoltcp::Error>
    where Tx: TxToken, F: FnOnce(EthernetFrame<&mut [u8]>)
{
    let tx_len = EthernetFrame::<&[u8]>::buffer_len(buffer_len);
    tx_token.consume(timestamp, tx_len, |tx_buffer| {
        debug_assert!(tx_buffer.as_ref().len() == tx_len);
        let mut frame = EthernetFrame::new_unchecked(tx_buffer.as_mut());
        frame.set_src_addr(eth_addr);

        f(frame);

        Ok(())
    })
}

fn process_arp<T: AsRef<[u8]>> (eth_addr: EthernetAddress, frame: &T) -> Result<ArpRepr, smoltcp::Error> {
    let eth_frame = EthernetFrame::new_checked(frame)?;

    // Ignore any packets not directed to our hardware address or any of the multicast groups.
    if !eth_frame.dst_addr().is_broadcast() &&
       !eth_frame.dst_addr().is_multicast() &&
       eth_frame.dst_addr() != eth_addr
    {
        return Err(smoltcp::Error::Dropped);
    }

    match eth_frame.ethertype() {
        EthernetProtocol::Arp => {
            let arp_packet = ArpPacket::new_checked(eth_frame.payload())?;
            let arp_repr = ArpRepr::parse(&arp_packet)?;

            match arp_repr {
                // Respond to ARP requests aimed at us, and fill the ARP cache from all ARP
                // requests and replies, to minimize the chance that we have to perform
                // an explicit ARP request.
                ArpRepr::EthernetIpv4 {
                    source_hardware_addr, source_protocol_addr, ..
                } => {
                    if source_protocol_addr.is_unicast() && source_hardware_addr.is_unicast() {
                        Ok(arp_repr)
                    } else {
                        Err(smoltcp::Error::Dropped)
                    }
                },
                _ => Err(smoltcp::Error::Unrecognized),
            }
        }
        _ => Err(smoltcp::Error::Unrecognized)
    }
}
