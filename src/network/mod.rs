use alloc::vec::Vec;
use alloc::string::String;
use managed::ManagedSlice;
use smoltcp::phy::Device;
use smoltcp::iface::EthernetInterface;
use smoltcp::wire::{ IpAddress, Ipv4Address, IpCidr };

pub mod arp;
pub mod cidr;
pub mod icmp;

pub trait StringableVec {
    fn to_string_vec(&self) -> Vec<String>;
}

pub fn set_ip4_address<'b, 'c, 'e, DeviceT>(iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>, addr: Ipv4Address, netmask: u8) 
    where DeviceT: for<'d> Device<'d> {
    iface.update_ip_addrs(|addrs| {
        let addr = IpAddress::from(addr);
        *addrs = ManagedSlice::from(vec![IpCidr::new(addr, netmask); 1]);
    });
}
