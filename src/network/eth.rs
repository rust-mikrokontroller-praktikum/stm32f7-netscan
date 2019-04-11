use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::*;
use stm32f7_discovery::{ethernet::EthernetDevice, system_clock};

use super::arp::ArpResponses;

pub type StatsResponses = BTreeMap<Ipv4Address, (usize, usize, Instant)>;

impl super::StringableVec for StatsResponses {
    fn to_string_vec(&self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        let now_s = Instant::from_millis(system_clock::ms() as i64).secs();
        for i in self.iter() {
            ret.push(format!("{}:", i.0));
            let (count, bytes, ts) = i.1;
            ret.push(format!("    {} packets", count));
            ret.push(format!("    {} bytes", bytes));
            if now_s > ts.secs() {
                ret.push(format!("    {} bytes / second", (*bytes as i64) / (now_s - ts.secs())));
            }
        }
        ret
    }
}

pub fn listen(
    stats: &mut StatsResponses,
    iface: &mut EthernetDevice,
    eth_addr: EthernetAddress,
    neighbors: &ArpResponses,
    gw: Option<Ipv4Address>,
) -> Result<(), String> {
    let mut tries = 0;
    let caps = iface.capabilities();
    loop {
        // Try to receive all packets currently in the buffer
        let (rx_token, tx_token) = match iface.receive() {
            None => {
                if tries > 100 {
                    break;
                }
                tries += 1;
                continue;
            }
            Some(tokens) => tokens,
        };
        rx_token
            .consume(Instant::from_millis(system_clock::ms() as i64), |frame| {
                let timestamp = Instant::from_millis(system_clock::ms() as i64);
                // Parse the raw ethernet frame and return the info necessary for a
                // response/forward
                process_eth(gw, &neighbors, eth_addr, &frame, &caps).and_then(
                    |(x, (addr, bytes))| {
                        // Collect statistics on received packets
                        stats
                            .entry(addr)
                            .and_modify(|(count, total_bytes, _)| {
                                *count += 1;
                                *total_bytes += bytes
                            })
                            .or_insert((1, bytes, timestamp));
                        if let Some((ethertype, dst, payload, len)) = x {
                            // Dispatch returned packet
                            dispatch_ethernet(
                                eth_addr,
                                tx_token,
                                timestamp,
                                len,
                                |mut frame| {
                                    frame.set_dst_addr(dst);
                                    // frame.set_src_addr(eth_addr);
                                    frame.set_ethertype(ethertype);
                                    frame.payload_mut().copy_from_slice(&payload);
                                },
                            )
                        } else {
                            Ok(())
                        }
                    },
                )
            })
            .or_else(|x| Err(x.to_string()))?;
    }
    Ok(())
}

fn process_eth<'a, T: AsRef<[u8]>>(
    gw: Option<Ipv4Address>,
    neighbors: &ArpResponses,
    eth_addr: EthernetAddress,
    frame: &'a T,
    caps: &DeviceCapabilities,
) -> Result<
    (
        Option<(EthernetProtocol, EthernetAddress, Vec<u8>, usize)>,
        (Ipv4Address, usize),
    ),
    smoltcp::Error,
> {
    // Ignore any packets not directed to our hardware address or any of the multicast groups.
    let eth_frame = EthernetFrame::new_checked(frame)?;

    if !eth_frame.dst_addr().is_broadcast()
        && !eth_frame.dst_addr().is_multicast()
        && eth_frame.dst_addr() != eth_addr
    {
        return Err(smoltcp::Error::Dropped);
    }

    let packet_len = eth_frame.payload().len() + EthernetFrame::<&T>::header_len();

    match eth_frame.ethertype() {
        EthernetProtocol::Ipv4 => {
            let ipv4_packet = Ipv4Packet::new_checked(eth_frame.payload())?;
            let checksum_caps = caps.checksum.clone();
            let ipv4_repr = Ipv4Repr::parse(&ipv4_packet, &checksum_caps)?;
            let dst_addr = match neighbors.get(&ipv4_repr.dst_addr) {
                Some(x) => Some(x),
                None => {
                    if let Some(gw) = gw {
                        neighbors.get(&gw)
                    } else {
                        None
                    }
                }
            };

            if let Some(dst) = dst_addr {
                Ok((
                    Some((
                        EthernetProtocol::Ipv4,
                        *dst,
                        Vec::from(eth_frame.payload()),
                        eth_frame.payload().len(),
                    )),
                    (ipv4_repr.src_addr, packet_len),
                ))
            } else {
                Ok((None, (ipv4_repr.src_addr, packet_len)))
            }
        }
        EthernetProtocol::Arp => {
            let arp_packet = ArpPacket::new_checked(eth_frame.payload())?;
            let arp_repr = ArpRepr::parse(&arp_packet)?;

            if let ArpRepr::EthernetIpv4 {
                operation,
                source_hardware_addr,
                source_protocol_addr,
                target_protocol_addr,
                ..
            } = arp_repr
            {
                if operation == ArpOperation::Request {
                    // Respond to ARP request
                    let arp = ArpRepr::EthernetIpv4 {
                        operation: ArpOperation::Reply,
                        source_hardware_addr: eth_addr,
                        source_protocol_addr: target_protocol_addr,
                        target_hardware_addr: source_hardware_addr,
                        target_protocol_addr: source_protocol_addr,
                    };
                    let mut pack = ArpPacket::new_unchecked(vec![0; arp.buffer_len()]);
                    arp.emit(&mut pack);
                    Ok((
                        Some((
                            EthernetProtocol::Arp,
                            source_hardware_addr,
                            pack.into_inner(),
                            arp.buffer_len(),
                        )),
                        (source_protocol_addr, packet_len),
                    ))
                } else {
                    Ok((None, (source_protocol_addr, packet_len)))
                }
            } else {
                Err(::smoltcp::Error::Unrecognized)
            }
        }
        // Ignore non ipv4 traffic
        _ => Err(::smoltcp::Error::Unrecognized),
    }
}

pub fn dispatch_ethernet<Tx, F>(
    eth_addr: EthernetAddress,
    tx_token: Tx,
    timestamp: Instant,
    buffer_len: usize,
    f: F,
) -> Result<(), smoltcp::Error>
where
    Tx: TxToken,
    F: FnOnce(EthernetFrame<&mut [u8]>),
{
    let tx_len = EthernetFrame::<&[u8]>::buffer_len(buffer_len);
    tx_token.consume(timestamp, tx_len, |tx_buffer| {
        debug_assert!(tx_buffer.as_ref().len() == tx_len);
        let mut frame = EthernetFrame::new_unchecked(tx_buffer);
        frame.set_src_addr(eth_addr);

        f(frame);

        Ok(())
    })
}
