enum CidrType {
    Ipv4Cidr,
    Ipv6Cidr,
}

struct Ipv4Cidr {
    addr: Ipv4Address,
    size: u8,
}

struct Ipv4Address {
    addr: [u8; 4],
}

struct Ipv6Cidr {
    addr: Ipv6Address,
    size: u8,
}

struct Ipv6Address {
    addr: [u8; 16],
}

impl Iterator for CidrType {
    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            CidrType::Ipv4Cidr => self.next(),
            CidrType::Ipv6Cidr => self.next(),
        }
    }
}

impl Iterator for Ipv4Cidr {
    fn next(&mut self) -> Option<Self::Item> {
        self.addr.inc(self.size);
    }
}

impl Ipv4Cidr {
    fn max_size() -> u8 {
        return 32;
    }
}

impl Ipv4Address {
    fn inc(&mut self, mask: u8) -> bool {
        match self.get_incrementable_idx() {
            Some(idx) => {
                let free_bits = (idx * 8) as u8 + self.addr[idx].leading_zeros() as u8 - mask;
                if self.addr[idx] < 2 ^ free_bits {
                    if self.addr[idx] >= 254 {
                        self.addr[idx] = 0;
                        self.addr[idx - 1] += 1;
                    } else {
                        self.addr[idx] += 1
                    }
                    return true;
                } else {
                    return false;
                }
            },
            None => return false,
        }
    }

    fn get_incrementable_idx(&mut self) -> Option<usize> {
        for i in (0..3).rev() {
            if self.addr[i] < 254 {
                return Some(i);
            }
        }
        return None;
    }
}

impl Iterator for Ipv6Cidr {
    fn next(&mut self) -> Option<Self::Item> {
    }
}

impl Ipv6Cidr {
    fn max_size() -> u8 {
        return 128;
    }
}

impl Ipv6Address {
    fn inc(&mut self, mask: u8) -> bool {
        match self.get_incrementable_idx() {
            Some(idx) => {
                let free_bits = (idx * 8) as u8 + self.addr[idx].leading_zeros() as u8 - mask;
                if self.addr[idx] < 2 ^ free_bits {
                    if self.addr[idx] >= 254 {
                        self.addr[idx] = 0;
                        self.addr[idx - 1] += 1;
                    } else {
                        self.addr[idx] += 1
                    }
                    return true;
                } else {
                    return false;
                }
            },
            None => return false,
        }
    }

    fn get_incrementable_idx(&mut self) -> Option<usize> {
        for i in (0..15).rev() {
            if self.addr[i] < 254 {
                return Some(i);
            }
        }
        return None;
    }
}
