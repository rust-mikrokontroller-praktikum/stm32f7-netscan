use alloc::string::String;
use alloc::vec::Vec;
use managed::ManagedSlice;
use smoltcp::iface::EthernetInterface;
use smoltcp::phy::Device;
use smoltcp::wire::{IpAddress, IpCidr, Ipv4Address};

use services::Service;

pub mod arp;
pub mod cidr;
pub mod eth;
pub mod icmp;
pub mod services;
pub mod tcp;
pub mod udp;

pub trait StringableVec {
    fn to_string_vec(&self) -> Vec<String>;
}

pub trait Stringable {
    fn to_string(&self) -> String;
}

#[derive(Debug)]
pub struct PortScan(pub Ipv4Address, pub Vec<&'static Service>);
pub type PortScans = Vec<PortScan>;

impl super::StringableVec for Vec<&Service> {
    fn to_string_vec(&self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        for i in self.iter() {
            ret.push(format!("    {} ({})", i.0, i.1));
        }
        ret
    }
}

impl super::StringableVec for PortScans {
    fn to_string_vec(&self) -> Vec<String> {
        let mut ret: Vec<String> = Vec::new();
        for i in self.iter() {
            ret.push(format!("{}:", i.0));
            if i.1.is_empty() {
                ret.push(String::from("    No open ports found"));
            } else {
                ret.extend(i.1.to_string_vec());
            }
        }
        ret
    }
}
pub fn set_ip4_address<'b, 'c, 'e, DeviceT>(
    iface: &mut EthernetInterface<'b, 'c, 'e, DeviceT>,
    addr: Ipv4Address,
    netmask: u8,
) where
    DeviceT: for<'d> Device<'d>,
{
    iface.update_ip_addrs(|addrs| {
        let addr = IpAddress::from(addr);
        *addrs = ManagedSlice::from(vec![IpCidr::new(addr, netmask); 1]);
    });
}
