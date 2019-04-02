use smoltcp::phy::Device;
use smoltcp::wire::*;
use super::cidr;

pub fn get_neighbors_v4(addr: EthernetAddress, cidr: Ipv4Cidr) {
    let mut arp_req = ArpRepr::EthernetIpv4 {
        operation: ArpOperation::Request,
        source_hardware_addr: addr,
        source_protocol_addr: Ipv4Address::new(0, 0, 0, 0),
        target_hardware_addr: EthernetAddress::BROADCAST,
        target_protocol_addr: Ipv4Address::new(0, 0, 0, 0)
    };
    if let ArpRepr::EthernetIpv4{ operation: _, source_hardware_addr: _, source_protocol_addr: _, target_hardware_addr: _, target_protocol_addr: ref mut y } = arp_req {
            *y = Ipv4Address::new(192, 168, 1, 1);
    }
}
