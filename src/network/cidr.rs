use alloc::string::String;
use byteorder::{ByteOrder, NetworkEndian};
use smoltcp::wire::Ipv4Address;

use super::Stringable;

// pub enum Cidr {
//     Ipv4Cidr,
//     Ipv6Cidr,
// }

// pub enum IpAddr {
//     Ipv4Addr,
//     Ipv6Addr,
// }

type Ipv4Addr = u32;
// type Ipv6Addr = u128;

impl Stringable for Ipv4Addr {
    fn to_string(&self) -> String {
        let mut octets: [u8; 4] = [0; 4];
        for offset in (0..=3).rev() {
            octets[3 - offset] = ((self & (0xFF << (offset * 8))) >> (offset * 8)) as u8;
        }
        format!("{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
    }
}

pub struct Ipv4Cidr {
    first_addr: Ipv4Addr,
    last_addr: Ipv4Addr,
    pub addr: Ipv4Addr,
    pub netmask: u8,
}

impl Stringable for Ipv4Cidr {
    fn to_string(&self) -> String {
        format!("{}/{}", self.addr.to_string(), self.netmask)
    }
}

// pub struct Ipv6Cidr {
//     first_addr: Ipv6Addr,
//     last_addr: Ipv6Addr,
//     addr: Ipv6Addr,
//     netmask: u8,
// }

// impl Iterator for Cidr {
//     type Item = Cidr;
//     fn next(&mut self) -> Option<Self::Item> {
//         match *self {
//             Cidr::Ipv4Cidr => self.next(),
//             // Cidr::Ipv6Cidr => self.next(),
//         }
//     }
// }

// impl Cidr {
// }

impl Iterator for Ipv4Cidr {
    type Item = Ipv4Addr;
    fn next(&mut self) -> Option<Self::Item> {
        if self.addr < self.last_addr {
            self.addr += 1;
            Some(self.addr)
        } else {
            None
        }
    }
}

/// Convert smoltcp Ipv4Cidr representation to our own representation
impl From<smoltcp::wire::Ipv4Cidr> for Ipv4Cidr {
    fn from(t: smoltcp::wire::Ipv4Cidr) -> Self {
        let mask = NetworkEndian::read_u32(t.netmask().as_bytes());
        // println!("mask: {:x}", mask);
        // let mask: Ipv4Addr = (0xFFFFFFFF << (32 - netmask)) & 0xFFFFFFFF;
        let addr = NetworkEndian::read_u32(t.address().as_bytes());
        // println!("addr: {:x}", addr);
        let res = Ipv4Cidr {
            first_addr: addr & mask,
            last_addr: (addr & mask) | !mask,
            addr,
            netmask: t.prefix_len(),
        };
        // println!("first_addr: {}, last_addr: {}", res.first_addr.to_string(), res.last_addr.to_string());
        res
    }
}

impl Ipv4Cidr {
    fn max_size() -> u8 {
        32
    }

    /// Create new Ipv4Cidr from address and netmask
    pub fn new(first: Ipv4Addr, netmask: u8) -> Self {
        let mask: Ipv4Addr = (0xFF_FF_FF_FF as u32)
            .checked_shl((32 - netmask).into())
            .unwrap_or(0);
        Ipv4Cidr {
            first_addr: first,
            last_addr: first | !mask,
            addr: first,
            netmask,
        }
    }

    /// Parse Ipv4Cidr from string in x.x.x.x/y form
    pub fn from_str(s: &str) -> Result<Self, &'static str> {
        let (addr_str, mask_str) = match split_ip_netmask(s) {
            Some(parts) => parts,
            None => return Err("Ipv4Cidr Parse Failure"),
        };
        let mut shift = 24;
        let mut addr: Ipv4Addr = 0;
        for octet in addr_str.split('.') {
            let a = match octet.parse::<u8>() {
                Ok(a) => a,
                Err(_) => return Err("Ipv4Address Parse Failure"),
            };
            addr |= (Ipv4Addr::from(a)) << shift;
            shift -= 8;
        }
        let netmask = match mask_str.parse::<u8>() {
            Ok(a) => {
                if a > Ipv4Cidr::max_size() {
                    return Err("Ipv4 Netmask too large");
                }
                a
            }
            Err(_) => return Err("Ipv4 Netmask Parse Failure"),
        };
        // let mask: Ipv4Addr = (0xFFFFFFFF << (32 - netmask)) & 0xFFFFFFFF;
        let mask: Ipv4Addr = (0xFF_FF_FF_FF as u32)
            .checked_shl((32 - netmask).into())
            .unwrap_or(0);
        // println!("mask: {:x}", mask);
        let res = Ipv4Cidr {
            first_addr: addr & mask,
            last_addr: (addr & mask) | !mask,
            addr,
            netmask,
        };
        // println!("first_addr: {}, last_addr: {}", res.first_addr.to_string(), res.last_addr.to_string());
        Ok(res)
    }

    pub fn reset(&mut self) {
        self.addr = self.first_addr;
    }
}

// impl Iterator for Ipv6Cidr {
//     type Item = Ipv6Addr;
//     fn next(&mut self) -> Option<Self::Item> {
//         if self.addr < self.last_addr {
//             self.addr += 1;
//             Some(self.addr)
//         } else {
//             None
//         }
//     }
// }

/// Convert our Ipv4Addr into a smoltcp Ipv4Address struct
pub fn to_ipv4_address(addr: Ipv4Addr) -> Ipv4Address {
    let mut octets: [u8; 4] = [0; 4];
    for offset in (0..=3).rev() {
        octets[3 - offset] = ((addr & (0xFF << (offset * 8))) >> (offset * 8)) as u8;
    }
    Ipv4Address::new(octets[0], octets[1], octets[2], octets[3])
}

fn split_ip_netmask(input: &str) -> Option<(&str, &str)> {
    let delimiter = match input.find('/') {
        Some(pos) => pos,
        None => return None,
    };
    let (ip, mask) = input.split_at(delimiter);
    let mask = &mask[1..];

    if ip.is_empty() || mask.is_empty() {
        None
    } else {
        Some((ip, mask))
    }
}
