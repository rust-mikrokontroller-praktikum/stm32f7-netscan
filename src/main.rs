#![warn(clippy::all)]
#![feature(alloc)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

mod gui;
mod network;

#[macro_use]
extern crate alloc;
extern crate alloc_cortex_m;
extern crate cortex_m;
extern crate cortex_m_rt as rt;
extern crate cortex_m_semihosting as sh;
#[macro_use]
extern crate stm32f7;
#[macro_use]
extern crate stm32f7_discovery;
extern crate smoltcp;

use gui::fuielement::FUiElement;
use gui::uistate::UiState;
use gui::uistates::UiStates;
use network::{Stringable, StringableVec};

use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::string::ToString;
use alloc::vec::Vec;
use alloc_cortex_m::CortexMHeap;
use core::alloc::Layout as AllocLayout;
use core::fmt::Write;
use core::panic::PanicInfo;
use cortex_m::{asm, interrupt, peripheral::NVIC};
use managed::ManagedSlice;
use rt::{entry, exception, ExceptionFrame};
use sh::hio::{self, HStdout};
use smoltcp::{
    dhcp::Dhcpv4Client,
    iface::{EthernetInterface, Route},
    socket::{Socket, SocketSet, UdpPacketMetadata, UdpSocketBuffer},
    time::{Duration, Instant},
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
};
use stm32f7::stm32f7x6::{CorePeripherals, Interrupt, Peripherals};
use stm32f7_discovery::{
    ethernet,
    gpio::{GpioPort, InputPin, OutputPin},
    init,
    lcd::{self, Color, Framebuffer, Layer},
    random::Rng,
    sd,
    system_clock::{self, Hz},
    touch,
};

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

const HEAP_SIZE: usize = 50 * 1024; // in bytes
const ETH_ADDR: EthernetAddress = EthernetAddress([0x00, 0x08, 0xdc, 0xab, 0xcd, 0xef]);

#[entry]
fn main() -> ! {
    let core_peripherals = CorePeripherals::take().unwrap();
    let mut systick = core_peripherals.SYST;
    let mut nvic = core_peripherals.NVIC;

    let peripherals = Peripherals::take().unwrap();
    let mut rcc = peripherals.RCC;
    let mut pwr = peripherals.PWR;
    let mut flash = peripherals.FLASH;
    let mut fmc = peripherals.FMC;
    let mut ltdc = peripherals.LTDC;
    let mut sai_2 = peripherals.SAI2;
    let mut rng = peripherals.RNG;
    let mut syscfg = peripherals.SYSCFG;
    let mut ethernet_mac = peripherals.ETHERNET_MAC;
    let mut ethernet_dma = peripherals.ETHERNET_DMA;
    let mut ethernet_dma = Some(&mut ethernet_dma);

    init::init_system_clock_216mhz(&mut rcc, &mut pwr, &mut flash);
    init::enable_gpio_ports(&mut rcc);

    let gpio_a = GpioPort::new(peripherals.GPIOA);
    let gpio_b = GpioPort::new(peripherals.GPIOB);
    let gpio_c = GpioPort::new(peripherals.GPIOC);
    let gpio_d = GpioPort::new(peripherals.GPIOD);
    let gpio_e = GpioPort::new(peripherals.GPIOE);
    let gpio_f = GpioPort::new(peripherals.GPIOF);
    let gpio_g = GpioPort::new(peripherals.GPIOG);
    let gpio_h = GpioPort::new(peripherals.GPIOH);
    let gpio_i = GpioPort::new(peripherals.GPIOI);
    let gpio_j = GpioPort::new(peripherals.GPIOJ);
    let gpio_k = GpioPort::new(peripherals.GPIOK);
    let mut pins = init::pins(
        gpio_a, gpio_b, gpio_c, gpio_d, gpio_e, gpio_f, gpio_g, gpio_h, gpio_i, gpio_j, gpio_k,
    );

    // configures the system timer to trigger a SysTick exception every second
    init::init_systick(Hz(100), &mut systick, &rcc);
    systick.enable_interrupt();

    init::init_sdram(&mut rcc, &mut fmc);
    let mut lcd = init::init_lcd(&mut ltdc, &mut rcc);
    pins.display_enable.set(true);
    pins.backlight.set(true);

    let mut layer_1 = lcd.layer_1().unwrap();
    let mut layer_2 = lcd.layer_2().unwrap();

    layer_1.clear();
    layer_2.clear();

    lcd::init_stdout(lcd.layer_2().unwrap());

    //println!("Hello World");

    //layer_1.print_point_color_at(0,0, Color::from_hex(0xFFFFFF));

    // Initialize the allocator BEFORE you use it
    unsafe { ALLOCATOR.init(rt::heap_start() as usize, HEAP_SIZE) }

    let _xs = vec![1, 2, 3];

    let mut i2c_3 = init::init_i2c_3(peripherals.I2C3, &mut rcc);
    i2c_3.test_1();
    i2c_3.test_2();

    nvic.enable(Interrupt::EXTI0);

    init::init_sai_2(&mut sai_2, &mut rcc);
    init::init_wm8994(&mut i2c_3).expect("WM8994 init failed");
    // touch initialization should be done after audio initialization, because the touch
    // controller might not be ready yet
    touch::check_family_id(&mut i2c_3).unwrap();

    let mut rng = Rng::init(&mut rng, &mut rcc).expect("RNG init failed");
    // print!("Random numbers: ");
    // for _ in 0..4 {
    //     print!(
    //         "{} ",
    //         rng.poll_and_get()
    //             .expect("Failed to generate random number")
    //     );
    // }
    // println!("");

    // Initialise the Start UI
    let mut current_ui_state = UiState::new();
    let mut draw_items = Vec::<String>::new();
    let mut element_map: BTreeMap<String, FUiElement> = BTreeMap::new();

    current_ui_state.change_ui_state(
        &mut layer_1,
        &mut draw_items,
        &mut element_map,
        UiStates::Initialization,
    );

    // ethernet
    // let mut ethernet_interface: Option<EthernetInterface<'b, 'c, 'e, DeviceT>> = None;
    let mut ethernet_interface = None;
    let mut gateway = None;
    let mut neighbors = network::arp::ArpResponses::new();
    let mut traffic_stats = network::eth::StatsResponses::new();
    let mut got_dhcp = false;

    let mut previous_button_state = pins.button.get();

    // Set the default Touch State
    let mut previous_touch_state = false;

    let mut interval_count = usize::max_value() - 1;
    let mut attack_gateway_v4_active = false;
    let mut attack_network_v4_active = false;
    let mut traffic_stats_active = false;

    let mut dns_servers: [Option<Ipv4Address>; 3] = [None; 3];

    loop {
        // poll button state
        let current_button_state = pins.button.get();
        if current_button_state != previous_button_state {
            if current_button_state {
                pins.led.toggle();

                // trigger the `EXTI0` interrupt
                NVIC::pend(Interrupt::EXTI0);
            }

            previous_button_state = current_button_state;
        }

        let mut number_of_touches = 0;

        // poll for new touch data
        for touch in &touch::touches(&mut i2c_3).unwrap() {
            // layer_1.print_point_color_at(
            //     touch.x as usize,
            //     touch.y as usize,
            //     Color::from_hex(0xffff00),
            // );

            //println!("{}", draw_items.len());
            // let new_x_pos = (rng.poll_and_get().expect("Failed to generate random number")%350) as usize;
            // let new_y_pos = (rng.poll_and_get().expect("Failed to generate random number")%150) as usize;
            // println!("{}", new_x_pos);
            // println!("{}", new_y_pos);
            // draw_items.push(
            //     ButtonText{
            //         x_pos: new_x_pos,
            //         y_pos: new_y_pos,
            //         x_size: 50,
            //         y_size: 50,
            //         text: "Test",
            //         touch: test
            //     }
            // );

            // for item in &mut draw_items {
            //     item.draw(&mut layer_1);
            // }

            // TODO: Multitouch ist nicht mehr möglich
            // Möglicher Fix: Button finden, der gerade gedrückt wird und die Koordinaten ignorieren
            if !previous_touch_state {
                previous_touch_state = true;

                let touch_x = touch.x as usize;
                let touch_y = touch.y as usize;

                let mut new_ui_state = current_ui_state.get_ui_state();

                for item_ref in &mut draw_items {
                    let item = element_map.get_mut(item_ref).unwrap();
                    if touch_x >= item.get_x_pos()
                        && touch_x <= (item.get_x_pos() + item.get_x_size())
                        && touch_y >= item.get_y_pos()
                        && touch_y <= (item.get_y_pos() + item.get_y_size())
                    {
                        //println!("Touched Button");
                        if item_ref == "INIT_ETHERNET" {
                            let dma = ethernet_dma.take().unwrap();
                            let iface = ethernet::EthernetDevice::new(
                                Default::default(),
                                Default::default(),
                                &mut rcc,
                                &mut syscfg,
                                &mut ethernet_mac,
                                dma,
                                ETH_ADDR,
                            );
                            ethernet_interface = match iface {
                                Ok(iface) => {
                                    new_ui_state = UiStates::Address;
                                    layer_2.clear();
                                    Some(iface.into_interface())
                                }
                                Err((e, dma)) => {
                                    println!("ethernet init failed: {:?}", e);
                                    ethernet_dma = Some(dma);
                                    None
                                }
                            };
                        } else if item_ref == "INIT_DHCP" && !got_dhcp {
                            let mut sockets = SocketSet::new(Vec::new());
                            let dhcp_rx_buffer =
                                UdpSocketBuffer::new([UdpPacketMetadata::EMPTY; 1], vec![0; 1500]);
                            let dhcp_tx_buffer =
                                UdpSocketBuffer::new([UdpPacketMetadata::EMPTY; 1], vec![0; 3000]);
                            let mut dhcp = Dhcpv4Client::new(
                                &mut sockets,
                                dhcp_rx_buffer,
                                dhcp_tx_buffer,
                                Instant::from_millis(system_clock::ms() as i64),
                            )
                            .expect("could not bind udp socket");
                            let start_timestamp = Instant::from_millis(system_clock::ms() as i64);
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            println!("Requesting DHCP Address...");
                            while !got_dhcp
                                || dhcp.next_poll(Instant::from_millis(system_clock::ms() as i64))
                                    < Duration::from_secs(30)
                            {
                                let timestamp = Instant::from_millis(system_clock::ms() as i64);
                                match iface.poll(&mut sockets, timestamp) {
                                    Err(::smoltcp::Error::Exhausted) => {
                                        continue;
                                    }
                                    Err(::smoltcp::Error::Unrecognized) => print!("U"),
                                    Err(e) => println!("Network error: {:?}", e),
                                    Ok(socket_changed) => {
                                        if socket_changed {
                                            for mut socket in sockets.iter_mut() {
                                                poll_socket(&mut socket)
                                                    .expect("socket poll failed");
                                            }
                                        }
                                    }
                                }

                                let config = dhcp
                                    .poll(iface, &mut sockets, timestamp)
                                    .unwrap_or_else(|e| {
                                        println!("DHCP: {:?}", e);
                                        None
                                    });
                                if let Some(x) = config {
                                    match x.address {
                                        Some(addr) => iface.update_ip_addrs(|addrs| {
                                            *addrs = ManagedSlice::from(vec![addr.into(); 1]);
                                        }),
                                        None => println!("DHCP Response without address"),
                                    };
                                    match x.router {
                                        Some(gw) => {
                                            iface.routes_mut().add_default_ipv4_route(gw).unwrap();
                                            gateway = Some(gw);
                                        }
                                        None => println!("DHCP Response without default route"),
                                    };

                                    dns_servers = x.dns_servers;

                                    layer_2.clear();
                                    got_dhcp = true;
                                    new_ui_state = UiStates::Start;
                                }
                                if !got_dhcp && timestamp - Duration::from_secs(5) > start_timestamp
                                {
                                    println!("DHCP Failed: no valid response");
                                    break;
                                }
                            }
                        } else if item_ref == "INIT_LISTEN" {
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            println!("Listening for activity in the local network...");
                            match network::arp::listen(&mut iface.device, ETH_ADDR) {
                                Some(cidr) => {
                                    println!("Setting subnet to {}", cidr.to_string());
                                    for addr in cidr {
                                        let s_addr = network::cidr::to_ipv4_address(addr);
                                        match network::arp::request(
                                            &mut iface.device,
                                            ETH_ADDR,
                                            s_addr,
                                        ) {
                                            Ok(x) => {
                                                if x {
                                                    new_ui_state = UiStates::Start;
                                                    network::set_ip4_address(iface, s_addr, 0);
                                                    layer_2.clear();
                                                    break;
                                                }
                                            }
                                            Err(e) => println!("Error during ARP request: {}", e),
                                        }
                                    }
                                }
                                None => {
                                    println!(
                                        "No activity in local network, please try again later."
                                    );
                                }
                            }
                        } else if item_ref == "INIT_GLOBAL" {
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            let cidr = network::cidr::Ipv4Cidr::new(0x01_00_00_00, 0);
                            for addr in cidr {
                                let s_addr = network::cidr::to_ipv4_address(addr);
                                match network::arp::request(&mut iface.device, ETH_ADDR, s_addr) {
                                    Ok(x) => {
                                        if x {
                                            new_ui_state = UiStates::Start;
                                            network::set_ip4_address(iface, s_addr, 0);
                                            break;
                                        }
                                    }
                                    Err(e) => println!("Error during ARP request: {}", e),
                                }
                            }
                        } else if item_ref == "INIT_10_0_0_0" {
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            let cidr = network::cidr::Ipv4Cidr::from_str("10.0.0.0/8").unwrap();
                            for addr in cidr {
                                let s_addr = network::cidr::to_ipv4_address(addr);
                                match network::arp::request(&mut iface.device, ETH_ADDR, s_addr) {
                                    Ok(x) => {
                                        if x {
                                            new_ui_state = UiStates::Start;
                                            network::set_ip4_address(iface, s_addr, 8);
                                            break;
                                        }
                                    }
                                    Err(e) => println!("Error during ARP request: {}", e),
                                }
                            }
                        } else if item_ref == "INIT_172_16_0_0" {
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            let cidr = network::cidr::Ipv4Cidr::from_str("172.16.0.0/12").unwrap();
                            for addr in cidr {
                                let s_addr = network::cidr::to_ipv4_address(addr);
                                match network::arp::request(&mut iface.device, ETH_ADDR, s_addr) {
                                    Ok(x) => {
                                        if x {
                                            new_ui_state = UiStates::Start;
                                            network::set_ip4_address(iface, s_addr, 12);
                                            break;
                                        }
                                    }
                                    Err(e) => println!("Error during ARP request: {}", e),
                                }
                            }
                        } else if item_ref == "INIT_192_168_0_0" {
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            let cidr = network::cidr::Ipv4Cidr::from_str("192.168.0.0/16").unwrap();
                            for addr in cidr {
                                let s_addr = network::cidr::to_ipv4_address(addr);
                                match network::arp::request(&mut iface.device, ETH_ADDR, s_addr) {
                                    Ok(x) => {
                                        if x {
                                            new_ui_state = UiStates::Start;
                                            network::set_ip4_address(iface, s_addr, 16);
                                            break;
                                        }
                                    }
                                    Err(e) => println!("Error during ARP request: {}", e),
                                }
                            }
                        } else if item_ref == "ButtonScrollUp" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();
                            let current_lines_start = scroll_text.get_lines_start();

                            if current_lines_start > 0 {
                                scroll_text.set_lines_start(current_lines_start - 1);
                                scroll_text.draw(&mut layer_1);
                            }
                        } else if item_ref == "ButtonScrollDown" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();
                            let current_lines_start = scroll_text.get_lines_start();

                            if scroll_text.get_lines().len() > ((scroll_text.get_y_size() / 8) - 1)
                                && current_lines_start
                                    < scroll_text.get_lines().len()
                                        - ((scroll_text.get_y_size() / 8) - 1)
                            {
                                scroll_text.set_lines_start(current_lines_start + 1);
                                scroll_text.draw(&mut layer_1);
                            }
                        } else if item_ref == "TRAFFIC" {
                            let stats_button: &mut FUiElement =
                                element_map.get_mut(&String::from("TRAFFIC")).unwrap();
                            let color1 = Color {
                                red: 0,
                                green: 255,
                                blue: 255,
                                alpha: 255,
                            };
                            let color2 = Color {
                                red: 0,
                                green: 255,
                                blue: 0,
                                alpha: 255,
                            };
                            if !traffic_stats_active {
                                stats_button.set_background_color(color1);
                                traffic_stats_active = true;
                                traffic_stats.clear();
                            } else {
                                traffic_stats_active = false;
                                stats_button.set_background_color(color2);
                            }
                            stats_button.draw(&mut layer_1);
                        } else if item_ref == "ARP_SCAN" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();

                            scroll_text.set_title(String::from("ARP Scan"));

                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            if let IpCidr::Ipv4(cidr) = iface.ip_addrs()[0] {
                                scroll_text.add_line(String::from(
                                    "Scanning for neighbors via ARP solicitations...",
                                ));
                                scroll_text.draw(&mut layer_1);

                                neighbors = match network::arp::get_neighbors_v4(
                                    &mut iface.device,
                                    ETH_ADDR,
                                    &mut cidr.into(),
                                ) {
                                    Ok(neigh) => neigh,
                                    Err(x) => {
                                        panic!("{}", x);
                                    }
                                };

                                if neighbors.is_empty() {
                                    scroll_text.add_line(String::from("No neighbors found"));;
                                } else {
                                    scroll_text.set_lines(neighbors.to_string_vec());
                                }

                                scroll_text.draw(&mut layer_1);

                                for neighbor in &neighbors {
                                    iface.inner.neighbor_cache.fill(
                                        (*neighbor.0).into(),
                                        *neighbor.1,
                                        Instant::from_millis(system_clock::ms() as i64),
                                    );
                                }
                            } else {
                                scroll_text.add_line(String::from(
                                    "No valid Ipv4 Address found, can't find network to scan.",
                                ));
                            }
                        } else if item_ref == "ICMP" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();

                            scroll_text.set_title(String::from("ICMP Ping"));

                            if !neighbors.is_empty() {
                                let alive_neighbors = network::icmp::scan_v4(
                                    &mut ethernet_interface.as_mut().unwrap(),
                                    &mut rng,
                                    &neighbors,
                                );
                                if alive_neighbors.is_empty() {
                                    scroll_text
                                        .add_line(String::from("No neighbors responded to pings"));
                                } else {
                                    scroll_text.set_lines(alive_neighbors.to_string_vec());
                                }
                            } else {
                                scroll_text.set_lines(vec![String::from(
                                    "No valid neighbors to ping, try performing an ARP scan",
                                )]);
                            }
                            // println!("Icmp Neighbors: {:?}", icmp_neighbors);
                            // scroll_text.set_title("ICMP Responses");
                            scroll_text.draw(&mut layer_1);
                        } else if item_ref == "TCP_PROBE" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();

                            scroll_text.set_title(String::from("TCP Scan"));

                            if !neighbors.is_empty() {
                                scroll_text.set_lines(vec![String::from("Probing neighbors...")]);
                                scroll_text.draw(&mut layer_1);
                                let ports = network::tcp::probe_addresses(
                                    &mut ethernet_interface.as_mut().unwrap(),
                                    &neighbors,
                                );
                                scroll_text.set_lines(ports.to_string_vec());
                            } else {
                                scroll_text.add_line(String::from(
                                    "No neighbors to probe, perform an ARP scan first",
                                ));
                            }
                            scroll_text.draw(&mut layer_1);
                        } else if item_ref == "UDP_PROBE" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();

                            scroll_text.set_title(String::from("UDP Scan"));

                            if !neighbors.is_empty() {
                                scroll_text.set_lines(vec![String::from("Probing neighbors...")]);
                                scroll_text.draw(&mut layer_1);

                                let ports = network::udp::probe_addresses(
                                    &mut ethernet_interface.as_mut().unwrap(),
                                    &neighbors,
                                );
                                scroll_text.set_lines(ports.to_string_vec());
                            } else {
                                scroll_text.add_line(String::from(
                                    "No neighbors to probe, perform an ARP scan first",
                                ));
                            }
                            scroll_text.draw(&mut layer_1);
                        } else if item_ref == "ButtonInfo" {
                            let scroll_text: &mut FUiElement =
                                element_map.get_mut(&String::from("ScrollText")).unwrap();
                            let iface = &mut ethernet_interface.as_mut().unwrap();

                            scroll_text.set_title(String::from("Info"));
                            scroll_text.set_lines(vec![]);

                            scroll_text.add_line(format!("MAC: {}", ETH_ADDR.to_string()));

                            for addr in iface.ip_addrs() {
                                if let IpCidr::Ipv4(x) = addr {
                                    scroll_text.add_line(format!("IPv4: {}", x.address()));

                                    scroll_text.add_line(format!("Netmask: {}", x.netmask()));
                                }
                            }

                            iface.routes_mut().update(|routes_map| {
                                routes_map
                                    .get(&IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0))
                                    .map(|default_route| {
                                        scroll_text.add_line(format!(
                                            "Gateway: {}",
                                            default_route.via_router
                                        ));
                                    });
                            });

                            for dns_server in dns_servers.iter() {
                                if let Some(x) = dns_server {
                                    scroll_text.add_line(format!("DNS: {}", x));
                                }
                            }

                            scroll_text.draw(&mut layer_1);
                        } else if item_ref == "ButtonKillGateway" {
                            let button_kill_gateway: &mut FUiElement = element_map
                                .get_mut(&String::from("ButtonKillGateway"))
                                .unwrap();

                            if !attack_gateway_v4_active {
                                button_kill_gateway.set_background_color(Color {
                                    red: 255,
                                    green: 0,
                                    blue: 0,
                                    alpha: 255,
                                });

                                attack_gateway_v4_active = true;
                            } else {
                                button_kill_gateway.set_background_color(Color {
                                    red: 255,
                                    green: 255,
                                    blue: 0,
                                    alpha: 255,
                                });

                                attack_gateway_v4_active = false;
                            }

                            button_kill_gateway.draw(&mut layer_1);
                        } else if item_ref == "ButtonKillNetwork" {
                            let button_kill_network: &mut FUiElement = element_map
                                .get_mut(&String::from("ButtonKillNetwork"))
                                .unwrap();

                            if !attack_network_v4_active {
                                button_kill_network.set_background_color(Color {
                                    red: 255,
                                    green: 0,
                                    blue: 0,
                                    alpha: 255,
                                });

                                attack_network_v4_active = true;
                            } else {
                                button_kill_network.set_background_color(Color {
                                    red: 255,
                                    green: 255,
                                    blue: 0,
                                    alpha: 255,
                                });

                                attack_network_v4_active = false;
                            }

                            button_kill_network.draw(&mut layer_1);
                        }
                    }
                }

                if new_ui_state != current_ui_state.get_ui_state() {
                    current_ui_state.change_ui_state(
                        &mut layer_1,
                        &mut draw_items,
                        &mut element_map,
                        new_ui_state,
                    );
                }
            }

            number_of_touches += 1;
        }

        if number_of_touches == 0 {
            //println!("NO TOUCH");
            previous_touch_state = false;
        }

        if traffic_stats_active {
            let iface = ethernet_interface.as_mut().unwrap();
            let scroll_text: &mut FUiElement =
                element_map.get_mut(&String::from("ScrollText")).unwrap();
            match network::eth::listen(
                &mut traffic_stats,
                &mut iface.device,
                ETH_ADDR,
                &neighbors,
                gateway,
            ) {
                Ok(_) => scroll_text.set_lines(traffic_stats.to_string_vec()),
                Err(x) => scroll_text.add_line(format!("Error during processing: {}", x)),
            }
        }

        let ticks = system_clock::ticks();
        if ticks % 100 == 0 {
            interval_count += 1;
            if traffic_stats_active {
                {
                    let stats_button: &mut FUiElement =
                        element_map.get_mut(&String::from("TRAFFIC")).unwrap();
                    let color1 = Color {
                        red: 0,
                        green: 255,
                        blue: 255,
                        alpha: 255,
                    };
                    let color2 = Color {
                        red: 0,
                        green: 255,
                        blue: 0,
                        alpha: 255,
                    };

                    // Button Animation
                    if stats_button.get_background_color() == color1 {
                        stats_button.set_background_color(color2);
                    } else {
                        stats_button.set_background_color(color1);
                    }
                    stats_button.draw(&mut layer_1);
                }
                element_map
                    .get_mut(&String::from("ScrollText"))
                    .unwrap()
                    .draw(&mut layer_1);
            }
            if attack_gateway_v4_active {
                let button_kill_gateway: &mut FUiElement = element_map
                    .get_mut(&String::from("ButtonKillGateway"))
                    .unwrap();

                let color1 = Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                };
                let color2 = Color {
                    red: 255,
                    green: 165,
                    blue: 0,
                    alpha: 255,
                };

                // Button Animation
                if button_kill_gateway.get_background_color() == color2 {
                    button_kill_gateway.set_background_color(color1);
                } else {
                    button_kill_gateway.set_background_color(color2);
                }

                button_kill_gateway.draw(&mut layer_1);
            }

            if attack_network_v4_active {
                let button_kill_network: &mut FUiElement = element_map
                    .get_mut(&String::from("ButtonKillNetwork"))
                    .unwrap();

                let color1 = Color {
                    red: 255,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                };
                let color2 = Color {
                    red: 255,
                    green: 165,
                    blue: 0,
                    alpha: 255,
                };

                // Button Animation
                if button_kill_network.get_background_color() == color2 {
                    button_kill_network.set_background_color(color1);
                } else {
                    button_kill_network.set_background_color(color2);
                }

                button_kill_network.draw(&mut layer_1);
            }
        }

        if interval_count >= 10 {
            interval_count = 0;
            if attack_gateway_v4_active {
                network::arp::attack_gateway_v4_request(
                    &mut ethernet_interface.as_mut().unwrap(),
                    ETH_ADDR,
                );

                network::arp::attack_gateway_v4_reply(
                    &mut ethernet_interface.as_mut().unwrap(),
                    ETH_ADDR,
                );
            }

            if attack_network_v4_active {
                if !neighbors.is_empty() {
                    network::arp::attack_network_v4_request(
                        &mut ethernet_interface.as_mut().unwrap(),
                        ETH_ADDR,
                        &neighbors,
                    );

                    network::arp::attack_network_v4_reply(
                        &mut ethernet_interface.as_mut().unwrap(),
                        ETH_ADDR,
                        &neighbors,
                    );
                } else {
                    let scroll_text: &mut FUiElement =
                        element_map.get_mut(&String::from("ScrollText")).unwrap();
                    scroll_text.set_lines(vec![String::from("No valid neighbors to attack")]);
                    scroll_text.draw(&mut layer_1);
                    attack_network_v4_active = false;
                    let button_kill_network: &mut FUiElement = element_map
                        .get_mut(&String::from("ButtonKillNetwork"))
                        .unwrap();

                    button_kill_network.set_background_color(Color {
                        red: 255,
                        green: 255,
                        blue: 0,
                        alpha: 255,
                    });

                    button_kill_network.draw(&mut layer_1);
                }
            }
        }
    }
}

fn poll_socket(socket: &mut Socket) -> Result<(), smoltcp::Error> {
    match socket {
        &mut Socket::Udp(ref mut socket) => match socket.endpoint().port {
            15 => loop {
                let reply;
                match socket.recv() {
                    Ok((data, remote_endpoint)) => {
                        let mut data = Vec::from(data);
                        let len = data.len() - 1;
                        data[..len].reverse();
                        reply = (data, remote_endpoint);
                    }
                    Err(smoltcp::Error::Exhausted) => break,
                    Err(err) => return Err(err),
                }
                socket.send_slice(&reply.0, reply.1)?;
            },
            _ => {}
        },
        &mut Socket::Tcp(ref mut socket) => match socket.local_endpoint().port {
            15 => {
                if !socket.may_recv() {
                    return Ok(());
                }
                let reply = socket.recv(|data| {
                    if data.len() > 0 {
                        let mut reply = Vec::from("tcp: ");
                        let start_index = reply.len();
                        reply.extend_from_slice(data);
                        reply[start_index..(start_index + data.len() - 1)].reverse();
                        (data.len(), Some(reply))
                    } else {
                        (data.len(), None)
                    }
                })?;
                if let Some(reply) = reply {
                    assert_eq!(socket.send_slice(&reply)?, reply.len());
                }
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

interrupt!(EXTI0, exti0, state: Option<HStdout> = None);

fn exti0(_state: &mut Option<HStdout>) {
    println!("Interrupt fired! This means that the button was pressed.");
}

#[exception]
fn SysTick() {
    system_clock::tick();
    // print a `.` every 500ms
    // if system_clock::ticks() % 50 == 0 && lcd::stdout::is_initialized() {
    //     print!(".");
    // }
}

#[exception]
fn HardFault(ef: &ExceptionFrame) -> ! {
    panic!("HardFault at {:#?}", ef);
}

// define what happens in an Out Of Memory (OOM) condition
#[alloc_error_handler]
fn rust_oom(_: AllocLayout) -> ! {
    panic!("out of memory");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    interrupt::disable();

    if lcd::stdout::is_initialized() {
        println!("{}", info);
    }

    if let Ok(mut hstdout) = hio::hstdout() {
        let _ = writeln!(hstdout, "{}", info);
    }

    // OK to fire a breakpoint here because we know the microcontroller is connected to a debugger
    asm::bkpt();

    loop {}
}
