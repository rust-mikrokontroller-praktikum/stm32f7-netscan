use stm32f7_discovery::ethernet::EthernetDevice;
use smoltcp::phy::Device;
use smoltcp::time::Instant;
use smoltcp::wire::*;
use super::cidr;

pub fn get_neighbors_v4(iface: &mut EthernetDevice, addr: EthernetAddress, cidr: &mut cidr::Ipv4Cidr) {
    let mut arp_req = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: addr,
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

        let tx_token = iface.transmit();
        dispatch_ethernet(tx_token, Instant::now(), arp_req.buffer_len(), |mut frame| {
            frame.set_dst_addr(arp_req.target_hardware_addr);
            frame.set_ethertype(EthernetProtocol::Arp);

            let mut packet = ArpPacket::new_unchecked(frame.payload_mut());
            arp_req.emit(&mut packet);
        });
    }
}

fn dispatch_ethernet<Tx, F>(&mut self, tx_token: Tx, timestamp: Instant,
                            buffer_len: usize, f: F) -> Result<()>
    where Tx: TxToken, F: FnOnce(EthernetFrame<&mut [u8]>)
{
    let tx_len = EthernetFrame::<&[u8]>::buffer_len(buffer_len);
    tx_token.consume(timestamp, tx_len, |tx_buffer| {
        debug_assert!(tx_buffer.as_ref().len() == tx_len);
        let mut frame = EthernetFrame::new_unchecked(tx_buffer.as_mut());
        frame.set_src_addr(self.ethernet_addr);

        f(frame);

        Ok(())
    })
}
