#![warn(clippy::all)]

#![feature(alloc)]
#![feature(alloc_error_handler)]
#![no_std]
#![no_main]

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


use network::StringableVec;

use stm32f7_discovery::lcd::FramebufferArgb8888;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
// use pin_utils::pin_mut;
use alloc::vec::Vec;
use alloc::string::String;
use alloc::string::ToString;
use alloc_cortex_m::CortexMHeap;
use core::alloc::Layout as AllocLayout;
use core::any::Any;
use core::fmt::Write;
use core::panic::PanicInfo;
use cortex_m::{asm, interrupt, peripheral::NVIC};
use rt::{entry, exception, ExceptionFrame};
use managed::ManagedSlice;
use sh::hio::{self, HStdout};
use smoltcp::{
    dhcp::Dhcpv4Client,
    iface::{EthernetInterface, Route},
    socket::{
        Socket, SocketSet, TcpSocket, TcpSocketBuffer,
        UdpPacketMetadata, UdpSocket, UdpSocketBuffer,
    },
    time::{Duration, Instant},
    wire::{EthernetAddress, IpCidr, IpEndpoint, Ipv4Address},
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
    let mut sdmmc = peripherals.SDMMC1;
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
    let mut current_ui_state = UiState{current_ui_state: UiStates::Initialization};
    let mut draw_items = Vec::<String>::new();
    let mut element_map: BTreeMap<String, FUiElement> = BTreeMap::new();

    current_ui_state.change_ui_state(&mut layer_1, &mut draw_items, &mut element_map, UiStates::Initialization);


    // ethernet
    // let mut ethernet_interface: Option<EthernetInterface<'b, 'c, 'e, DeviceT>> = None;
    let mut ethernet_interface = None;
    let mut neighbors = network::arp::ArpResponses::new();
    let mut got_dhcp = false;

    let mut previous_button_state = pins.button.get();

    // Set the default Touch State
    let mut previous_touch_state = false;

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
            if !previous_touch_state{
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
                        if item_ref == "INIT_ETHERNET"{
                            new_ui_state = UiStates::Address;
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
                                    // layer_2.clear();
                                    Some(iface.into_interface())
                                },
                                Err((e, dma)) => {
                                    let scroll_text: &mut FUiElement =
                                        element_map.get_mut(&String::from("ScrollText")).unwrap();
                                    scroll_text.set_lines(vec!(format!("ethernet init failed: {:?}", e); 1));
                                    scroll_text.draw(&mut layer_1);
                                    ethernet_dma = Some(dma);
                                    None
                                },
                            };
                        } else if item_ref == "INIT_DHCP" && !got_dhcp {
                            new_ui_state = UiStates::Start;
                            let mut sockets = SocketSet::new(Vec::new());
                            let dhcp_rx_buffer = UdpSocketBuffer::new([UdpPacketMetadata::EMPTY; 1], vec![0; 1500]);
                            let dhcp_tx_buffer = UdpSocketBuffer::new([UdpPacketMetadata::EMPTY; 1], vec![0; 3000]);
                            let mut dhcp = Dhcpv4Client::new(&mut sockets, dhcp_rx_buffer, dhcp_tx_buffer,
                                Instant::from_millis(system_clock::ms() as i64)).expect("could not bind udp socket");
                            let start_timestamp = Instant::from_millis(system_clock::ms() as i64);
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            while !got_dhcp {
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
                                                poll_socket(&mut socket).expect("socket poll failed");
                                            }
                                        }
                                    }
                                }

                                let config = dhcp.poll(iface, &mut sockets, timestamp)
                                    .unwrap_or_else(|e| { println!("DHCP: {:?}", e); None});
                                if let Some(x) = config {
                                    match x.address {
                                        Some(addr) => iface.update_ip_addrs(|addrs| { *addrs = ManagedSlice::from(vec![addr.into(); 1]); }),
                                        None => println!("DHCP Response without address"),
                                    };
                                    match x.router {
                                        Some(gw) => { iface.routes_mut().add_default_ipv4_route(gw).unwrap(); },
                                        None => println!("DHCP Response without default route"),
                                    };
                                    println!("DHCP Success: got address");
                                    got_dhcp = true;
                                    break;
                                }
                                if !got_dhcp && timestamp - Duration::from_secs(5) > start_timestamp {
                                    println!("DHCP Failed: no valid response");
                                    break;
                                }
                            }
                        } else if item_ref == "INIT_STATIC" {
                            new_ui_state = UiStates::Start;
                            network::set_ip4_address(&mut ethernet_interface.as_mut().unwrap(), Ipv4Address::new(192, 168, 1, 1), 24);
                        } else if item_ref == "ButtonScrollUp" {
                            let scroll_text: &mut FUiElement = element_map.get_mut(&String::from("ScrollText")).unwrap();
                            let current_lines_start = scroll_text.get_lines_start();
                            
                            if current_lines_start > 0{
                                scroll_text.set_lines_start(current_lines_start - 1);
                                scroll_text.draw(&mut layer_1);
                            }
                        } else if item_ref == "ButtonScrollDown" {
                            let scroll_text: &mut FUiElement = element_map.get_mut(&String::from("ScrollText")).unwrap();
                            let current_lines_start = scroll_text.get_lines_start();
                            scroll_text.set_lines_start(current_lines_start + 1);
                            scroll_text.draw(&mut layer_1);
                        } else if item_ref == "ARP_SCAN" {
                            let scroll_text: &mut FUiElement = element_map
                                .get_mut(&String::from("ScrollText")).unwrap();
                            let iface = &mut ethernet_interface.as_mut().unwrap();
                            neighbors = match network::cidr::Ipv4Cidr::from_str("192.168.1.0/24") {
                                Ok(mut c) => {
                                    match network::arp::get_neighbors_v4(&mut iface.device, ETH_ADDR, &mut c) {
                                        Ok(neigh) => neigh,
                                        Err(x) => {
                                            panic!("{}", x);
                                        },
                                    }
                                },
                                Err(x) => {
                                    panic!("{}", x);
                                },
                            };
                            // scroll_text.set_title("Neighbors");
                            scroll_text.set_lines(neighbors.to_string_vec());
                            scroll_text.draw(&mut layer_1);
                            for neighbor in &neighbors {
                                iface.inner.neighbor_cache.fill(neighbor.0.into(), neighbor.1, Instant::from_millis(system_clock::ms() as i64));
                            }
                        } else if item_ref == "ICMP" {
                            let scroll_text: &mut FUiElement = element_map
                                .get_mut(&String::from("ScrollText")).unwrap();
                            let mut sockets = SocketSet::new(Vec::new());
                            if !neighbors.is_empty() {
                                let icmp_neighbors = network::icmp::scan_v4(&mut ethernet_interface.as_mut().unwrap(), &mut
                                                                            sockets, &mut rng, &neighbors);
                                scroll_text.set_lines(icmp_neighbors.to_string_vec());
                            } else {
                                scroll_text.set_lines(vec!(String::from("No valid neighbors to ping")));
                            }
                            // println!("Icmp Neighbors: {:?}", icmp_neighbors);
                            // scroll_text.set_title("ICMP Responses");
                            scroll_text.draw(&mut layer_1);
                        }
                    }
                }

                if new_ui_state != current_ui_state.get_ui_state(){
                    current_ui_state.change_ui_state(&mut layer_1, &mut draw_items, &mut element_map, new_ui_state);
                }
            }

            number_of_touches += 1;

        }

        if number_of_touches == 0{
            //println!("NO TOUCH");
            previous_touch_state = false;
        }

        // handle new ethernet packets
        // if let Ok((ref mut iface, ref mut prev_ip_addr)) = ethernet_interface {
        //     let timestamp = Instant::from_millis(system_clock::ms() as i64);
        //     match iface.poll(&mut sockets, timestamp) {
        //         Err(::smoltcp::Error::Exhausted) => {
        //             continue;
        //         }
        //         Err(::smoltcp::Error::Unrecognized) => print!("U"),
        //         Err(e) => println!("Network error: {:?}", e),
        //         Ok(socket_changed) => {
        //             if socket_changed {
        //                 for mut socket in sockets.iter_mut() {
        //                     poll_socket(&mut socket).expect("socket poll failed");
        //                 }
        //             }
        //         }
        //     }

        //     let config = dhcp.poll(iface, &mut sockets, timestamp)
        //         .unwrap_or_else(|e| { println!("DHCP: {:?}", e); None});
        //     let ip_addr = iface.ipv4_addr().unwrap();
        //     if ip_addr != *prev_ip_addr {
        //         println!("\nAssigned a new IPv4 address: {}", ip_addr);
        //         iface.routes_mut().update(|routes_map| {
        //             routes_map
        //                 .get(&IpCidr::new(Ipv4Address::UNSPECIFIED.into(), 0))
        //                 .map(|default_route| {
        //                     println!("Default gateway: {}", default_route.via_router);
        //                 });
        //         });
        //         for dns_server in config.iter().flat_map(|c| c.dns_servers.iter()).filter_map(|x| x.as_ref()) {
        //             println!("DNS servers: {}", dns_server);
        //         }

        //         // TODO delete old sockets

        //         // add new sockets
        //         let endpoint = IpEndpoint::new(ip_addr.into(), 15);

        //         let udp_rx_buffer =
        //             UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY; 3], vec![0u8; 256]);
        //         let udp_tx_buffer =
        //             UdpSocketBuffer::new(vec![UdpPacketMetadata::EMPTY; 1], vec![0u8; 128]);
        //         let mut example_udp_socket = UdpSocket::new(udp_rx_buffer, udp_tx_buffer);
        //         example_udp_socket.bind(endpoint).unwrap();
        //         sockets.add(example_udp_socket);

        //         let tcp_rx_buffer = TcpSocketBuffer::new(vec![0; ethernet::MTU]);
        //         let tcp_tx_buffer = TcpSocketBuffer::new(vec![0; ethernet::MTU]);
        //         let mut example_tcp_socket = TcpSocket::new(tcp_rx_buffer, tcp_tx_buffer);
        //         example_tcp_socket.listen(endpoint).unwrap();
        //         sockets.add(example_tcp_socket);

        //         *prev_ip_addr = ip_addr;
        //     }
        //     let mut timeout = dhcp.next_poll(timestamp);
        //     iface
        //         .poll_delay(&sockets, timestamp)
        //         .map(|sockets_timeout| timeout = sockets_timeout);
        //     // TODO await next interrupt
        // }
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




trait UiElement<T: Framebuffer>: Any {
    fn get_x_pos(&mut self) -> usize;
    fn get_y_pos(&mut self) -> usize;
    fn get_x_size(&mut self) -> usize;
    fn get_y_size(&mut self) -> usize;

    fn set_background_color(&mut self, color: Color);
    fn set_text_color(&mut self, color: Color);

    //fn run_touch_func(&mut self);

    fn draw(&mut self, layer: &mut Layer<T>);

    fn set_text(&mut self, text: String){
        println!("set_text called for unimplemented struct")
    }

    fn set_lines(&mut self, lines: Vec<String>) {
        println!("set_lines called for unimplemented struct")
    }

    fn add_line(&mut self, line: String){
        println!("add_line called for unimplemented struct")
    }

    fn set_lines_start(&mut self, lines_start: usize) {
        println!("set_lines_start called for unimplemented struct")
    }

    fn get_lines_start(&mut self) -> usize{
        println!("get_lines_start called for unimplemented struct");
        0
    }

}


pub struct ButtonText {
    x_pos: usize,
    y_pos: usize,
    x_size: usize,
    y_size: usize,
    text: String,
    background_color: Color,
    text_color: Color,
    //touch: fn()
}

impl ButtonText{
    fn new(x_pos: usize, y_pos: usize, x_size: usize, y_size: usize, text: String) -> ButtonText{
        ButtonText{
            x_pos: x_pos,
            y_pos: y_pos,
            x_size: x_size,
            y_size: y_size,
            text: text,
            background_color:
                Color {
                    red: 0,
                    green: 255,
                    blue: 0,
                    alpha: 255,
                },
            text_color:
                Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                },
        }
    }

//     fn newline(&mut self) {
//         self.y_pos += 8;
//         self.x_pos = 0;
//         if self.y_pos >= HEIGHT {
//             self.y_pos = 0;
//             self.layer.clear();
//         }
//     }
}

impl<T: Framebuffer> UiElement<T> for ButtonText {
    fn get_x_pos(&mut self) -> usize{
        self.x_pos
    }
    
    fn get_y_pos(&mut self) -> usize{
        self.y_pos
    }
    
    fn get_x_size(&mut self) -> usize{
        self.x_size
    }
    
    fn get_y_size(&mut self) -> usize{
        self.y_size
    }
    
    fn set_text(&mut self, text: String){
        self.text = text;
    }

    fn set_background_color(&mut self, color: Color){
        self.background_color = color;
    }

    fn set_text_color(&mut self, color: Color){
        self.text_color = color;
    }

    // fn run_touch_func(&mut self){
    //     (self.touch)()
    // }
    
    fn draw(&mut self, layer: &mut Layer<T>) {
        use font8x8::{self, UnicodeFonts};

        for x in self.x_pos..self.x_pos+self.x_size {
            for y in self.y_pos..self.y_pos+self.y_size {
                layer.print_point_color_at(x, y, self.background_color);
            }
            
        }

        let mut temp_x_pos = self.x_pos;

        for c in self.text.chars() {
            // if c == '\n' {
            //     self.newline();
            //     continue;
            // }
            match c {
                ' '..='~' => {
                    let rendered = font8x8::BASIC_FONTS
                        .get(c)
                        .expect("character not found in basic font");
                    for (y, byte) in rendered.iter().enumerate() {
                        for (x, bit) in (0..8).enumerate() {
                            //TODO remove alpha
                            let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };
                            if alpha != 0{
                                layer.print_point_color_at(temp_x_pos + x, self.y_pos + y, self.text_color);
                            }
                        }
                    }
                }
                _ => panic!("unprintable character"),
            }
            temp_x_pos += 8;
        }
    }
}

pub struct ScrollableText {
    x_pos: usize,
    y_pos: usize,
    x_size: usize,
    y_size: usize,
    lines_show: usize,
    lines: Vec<String>,
    lines_start: usize,
    background_color: Color,
    text_color: Color,
}

impl ScrollableText {
    fn new(x_pos: usize, y_pos: usize, x_size: usize, y_size: usize, lines: Vec<String>) -> ScrollableText{
        ScrollableText{
            x_pos: x_pos,
            y_pos: y_pos,
            x_size: x_size,
            y_size: y_size,
            // TODO: y_size / font_height
            lines_show: 10,
            lines: lines,
            lines_start: 0,
            background_color:
                Color {
                    red: 0,
                    green: 255,
                    blue: 0,
                    alpha: 255,
                },
            text_color:
                Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                },
        }
    }
}

impl<T: Framebuffer> UiElement<T> for ScrollableText {
    fn get_x_pos(&mut self) -> usize{
        self.x_pos
    }
    
    fn get_y_pos(&mut self) -> usize{
        self.y_pos
    }
    
    fn get_x_size(&mut self) -> usize{
        self.x_size
    }
    
    fn get_y_size(&mut self) -> usize{
        self.y_size
    }

    fn set_text(&mut self, text: String){
        self.lines = vec!(text);
    }

    fn set_lines(&mut self, lines: Vec<String>){
        self.lines = lines;
    }

    fn set_background_color(&mut self, color: Color){
        self.background_color = color;
    }

    fn set_text_color(&mut self, color: Color){
        self.text_color = color;
    }

    fn add_line(&mut self, line: String){
        self.lines.push(line);
    }

    fn set_lines_start(&mut self, lines_start: usize) {
        if lines_start < 0{
            println!("lines_start < 0");
        } else {
            self.lines_start = lines_start;
        }
    }

    fn get_lines_start(&mut self) -> usize{
        self.lines_start
    }
    
    // fn run_touch_func(&mut self){
    // }

    fn draw(&mut self, layer: &mut Layer<T>) {
        use font8x8::{self, UnicodeFonts};

        for x in self.x_pos..self.x_pos+self.x_size {
            for y in self.y_pos..self.y_pos+self.y_size {
                layer.print_point_color_at(x, y, self.background_color);
            }
            
        }

        let mut temp_x_pos = self.x_pos;
        let mut temp_y_pos = self.y_pos;
        let mut count_lines_start = 0;
        let mut count_lines_show = 0;

        //println!("Number of lines {}", lines_split.len());

        for line in self.lines.iter(){
            if count_lines_start < self.lines_start{
                //println!("Skip line");
            } else if count_lines_show >= self.lines_show{
                //println!("End line");
                break;
            } else {
                for c in line.chars() {
                    match c {
                        ' '..='~' => {
                            let rendered = font8x8::BASIC_FONTS
                                .get(c)
                                .expect("character not found in basic font");
                            for (y, byte) in rendered.iter().enumerate() {
                                for (x, bit) in (0..8).enumerate() {
                                    let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };
                                    let mut color = self.text_color;
                                    color.alpha = alpha;
                                    if alpha != 0{
                                        layer.print_point_color_at(temp_x_pos + x, temp_y_pos + y, color);
                                    }
                                }
                            }
                        }
                        _ => panic!("unprintable character"),
                    }
                    temp_x_pos += 8;
                }
                count_lines_show += 1;

                //New line inside the box
                temp_x_pos = self.x_pos;
                temp_y_pos += 8;
            }
            count_lines_start += 1;
        }

        
    }
}

#[derive(Copy, Clone, PartialEq)]
enum UiStates{
    Initialization,
    Address,
    Start
}

struct UiState{
    current_ui_state: UiStates
}

type FUiElement = Box<UiElement<FramebufferArgb8888>>;

impl UiState {
    fn get_ui_state(&mut self) -> UiStates{
        self.current_ui_state
    }

    fn change_ui_state(&mut self, layer: &mut Layer<FramebufferArgb8888>, draw_items: &mut Vec<String>, elements: &mut BTreeMap<String, FUiElement>, new_ui_state: UiStates){

        // Clear everything
        draw_items.clear();

        //Initialization
        elements.insert(String::from("INIT_ETHERNET"), Box::new(ButtonText::new(200, 111, 80, 50, String::from("ETH"))));

        //Address
        elements.insert(String::from("INIT_DHCP"), Box::new(ButtonText::new(155, 111, 80, 50, String::from("DHCP"))));

        elements.insert(String::from("INIT_STATIC"), Box::new(ButtonText::new(245, 111, 80, 50, String::from("STATIC"))));

        //Start
        elements.insert(String::from("ScrollText"), Box::new(ScrollableText::new(5, 5, 300, 262, Vec::new())));

        elements.insert(String::from("ButtonScrollUp"), Box::new(ButtonText::new(310, 5, 80, 50, String::from("UP"))));

        elements.insert(String::from("ButtonScrollDown"), Box::new(ButtonText::new(310, 60, 80, 50, String::from("DOWN"))));
        
        elements.insert(String::from("ARP_SCAN"), Box::new(ButtonText::new(395, 5, 80, 50, String::from("ARP"))));

        elements.insert(String::from("ICMP"), Box::new(ButtonText::new(395, 60, 80, 50, String::from("ICMP"))));

        //elements.insert(String::from("ButtonHome"), Box::new(ButtonText::new(400, 222, 80, 50, String::from("HOME"))));

        if new_ui_state == UiStates::Initialization{
            draw_items.push(String::from("INIT_ETHERNET"));
        } else if new_ui_state == UiStates::Address{
            draw_items.push(String::from("INIT_DHCP"));
            draw_items.push(String::from("INIT_STATIC"));
        } else if new_ui_state == UiStates::Start{
            draw_items.push(String::from("ScrollText"));

            draw_items.push(String::from("ButtonScrollUp"));
            draw_items.push(String::from("ButtonScrollDown"));

            draw_items.push(String::from("ARP_SCAN"));
            draw_items.push(String::from("ICMP"));
        }


        //Clear and redraw
        layer.clear();

        for item in draw_items {
            elements.get_mut(item).unwrap().draw(layer);
        }

        self.current_ui_state = new_ui_state;
    }
}
