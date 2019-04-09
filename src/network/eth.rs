use super::cidr;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use smoltcp::phy::{Device, DeviceCapabilities, RxToken, TxToken};
use smoltcp::time::Instant;
use smoltcp::wire::*;
use stm32f7_discovery::{ethernet::EthernetDevice, system_clock};

use super::arp::ArpResponses;

pub type StatsResponses = BTreeMap<Ipv4Address, usize>;

impl super::StringableVec for StatsResponses {
    fn to_string_vec(&self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        for i in self.iter() {
            ret.push(format!("{} ({})", i.0, i.1));
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
            process_eth(gw, &neighbors, eth_addr, &frame, &caps)
        }) {
            Ok((x, addr)) => {
                *stats.entry(addr).or_insert(1) += 1;
                if let Some((src, dst, payload, len)) = x {
                    let tx_token = match iface.transmit() {
                        Some(x) => x,
                        None => return Err(String::from("No tx descriptor available")),
                    };
                    match dispatch_ethernet(
                        eth_addr,
                        tx_token,
                        Instant::from_millis(system_clock::ms() as i64),
                        len,
                        |mut frame| {
                            frame.set_dst_addr(dst);
                            frame.set_src_addr(src);
                            frame.set_ethertype(EthernetProtocol::Ipv4);
                            frame.payload_mut().copy_from_slice(&payload);
                        },
                    ) {
                        Ok(_) => {}
                        Err(x) => return Err(x.to_string()),
                    };
                }
            }
            Err(::smoltcp::Error::Unrecognized) => {}
            Err(x) => {}
        };
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
        Option<(EthernetAddress, EthernetAddress, Vec<u8>, usize)>,
        Ipv4Address,
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
                Ok(((
                    Some((
                        eth_frame.src_addr(),
                        *dst,
                        Vec::from(eth_frame.payload()),
                        eth_frame.payload().len(),
                    )),
                    ipv4_repr.src_addr,
                )))
            } else {
                Ok((None, ipv4_repr.src_addr))
            }
        }
        e => Err(::smoltcp::Error::Unrecognized),
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
        let mut frame = EthernetFrame::new_unchecked(tx_buffer.as_mut());
        frame.set_src_addr(eth_addr);

        f(frame);

        Ok(())
    })
}
