use super::buttontext::ButtonText;
use super::fuielement::FUiElement;
use super::scrollabletext::ScrollableText;
use super::uistates::UiStates;
use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;
use stm32f7_discovery::lcd::Color;
use stm32f7_discovery::lcd::FramebufferArgb8888;
use stm32f7_discovery::lcd::Layer;

pub struct UiState {
    current_ui_state: UiStates,
}

impl UiState {
    // Create a new UIState
    pub fn new() -> UiState {
        UiState {
            current_ui_state: UiStates::Initialization,
        }
    }

    // Return the current UIState
    pub fn get_ui_state(&mut self) -> UiStates {
        self.current_ui_state
    }

    // Change the current UIState
    pub fn change_ui_state(
        &mut self,
        layer: &mut Layer<FramebufferArgb8888>,
        draw_items: &mut Vec<String>,
        elements: &mut BTreeMap<String, FUiElement>,
        new_ui_state: UiStates,
    ) {
        // Clear everything
        draw_items.clear();

        //Initialization
        let mut ethernet_hint: FUiElement = Box::new(ScrollableText::new(
            50,
            50,
            400,
            50,
            vec![String::from(
                "Please connect your ethernet cable and press the button",
            )],
        ));
        let color_ethernet_hint = Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        };
        ethernet_hint.set_background_color(color_ethernet_hint);
        elements.insert(String::from("EthernetHint"), ethernet_hint);

        elements.insert(
            String::from("INIT_ETHERNET"),
            Box::new(ButtonText::new(200, 111, 80, 50, String::from("ETH"))),
        );

        //Address
        let mut address_hint: FUiElement = Box::new(ScrollableText::new(
            80,
            50,
            350,
            50,
            vec![String::from("Please select your network configuration")],
        ));
        let color_address_hint = Color {
            red: 0,
            green: 0,
            blue: 0,
            alpha: 0,
        };
        address_hint.set_background_color(color_address_hint);
        elements.insert(String::from("AddressHint"), address_hint);

        elements.insert(
            String::from("INIT_DHCP"),
            Box::new(ButtonText::new(30, 81, 110, 50, String::from("DHCP"))),
        );

        elements.insert(
            String::from("INIT_GLOBAL"),
            Box::new(ButtonText::new(310, 81, 110, 50, String::from("Global"))),
        );

        elements.insert(
            String::from("INIT_10_0_0_0"),
            Box::new(ButtonText::new(
                30,
                141,
                110,
                50,
                String::from("10.0.0.0/8"),
            )),
        );

        elements.insert(
            String::from("INIT_172_16_0_0"),
            Box::new(ButtonText::new(
                170,
                141,
                110,
                50,
                String::from("172.16.0.0/12"),
            )),
        );

        elements.insert(
            String::from("INIT_192_168_0_0"),
            Box::new(ButtonText::new(
                310,
                141,
                110,
                50,
                String::from("192.168.0.0/16"),
            )),
        );

        //Start
        elements.insert(
            String::from("ScrollText"),
            Box::new(ScrollableText::new(5, 1, 300, 270, Vec::new())),
        );

        elements.insert(
            String::from("ButtonScrollUp"),
            Box::new(ButtonText::new(310, 1, 80, 50, String::from("UP"))),
        );

        elements.insert(
            String::from("ButtonScrollDown"),
            Box::new(ButtonText::new(310, 56, 80, 50, String::from("DOWN"))),
        );

        elements.insert(
            String::from("TRAFFIC"),
            Box::new(ButtonText::new(310, 111, 80, 50, String::from("Traffic"))),
        );

        elements.insert(
            String::from("ButtonInfo"),
            Box::new(ButtonText::new(310, 165, 80, 50, String::from("INFO"))),
        );

        let mut button_kill_gateway: FUiElement =
            Box::new(ButtonText::new(310, 220, 80, 50, String::from("KILL GW")));
        button_kill_gateway.set_background_color(Color {
            red: 255,
            green: 255,
            blue: 0,
            alpha: 255,
        });
        elements.insert(String::from("ButtonKillGateway"), button_kill_gateway);

        elements.insert(
            String::from("ARP_SCAN"),
            Box::new(ButtonText::new(395, 1, 80, 50, String::from("ARP"))),
        );

        elements.insert(
            String::from("ICMP"),
            Box::new(ButtonText::new(395, 56, 80, 50, String::from("ICMP"))),
        );

        elements.insert(
            String::from("TCP_PROBE"),
            Box::new(ButtonText::new(395, 110, 80, 50, String::from("TCP PROBE"))),
        );

        elements.insert(
            String::from("UDP_PROBE"),
            Box::new(ButtonText::new(395, 165, 80, 50, String::from("UDP PROBE"))),
        );

        let mut button_kill_network: FUiElement =
            Box::new(ButtonText::new(395, 220, 80, 50, String::from("KILL NET")));
        button_kill_network.set_background_color(Color {
            red: 255,
            green: 255,
            blue: 0,
            alpha: 255,
        });
        elements.insert(String::from("ButtonKillNetwork"), button_kill_network);

        //elements.insert(String::from("ButtonHome"), Box::new(ButtonText::new(400, 222, 80, 50, String::from("HOME"))));

        if new_ui_state == UiStates::Initialization {
            draw_items.push(String::from("EthernetHint"));
            draw_items.push(String::from("INIT_ETHERNET"));
        } else if new_ui_state == UiStates::Address {
            draw_items.push(String::from("AddressHint"));
            draw_items.push(String::from("INIT_DHCP"));
            draw_items.push(String::from("INIT_GLOBAL"));

            draw_items.push(String::from("INIT_10_0_0_0"));
            draw_items.push(String::from("INIT_172_16_0_0"));
            draw_items.push(String::from("INIT_192_168_0_0"));
        } else if new_ui_state == UiStates::Start {
            draw_items.push(String::from("ScrollText"));

            draw_items.push(String::from("ButtonScrollUp"));
            draw_items.push(String::from("ButtonScrollDown"));
            draw_items.push(String::from("TRAFFIC"));
            draw_items.push(String::from("ButtonInfo"));
            draw_items.push(String::from("ButtonKillGateway"));

            draw_items.push(String::from("ARP_SCAN"));
            draw_items.push(String::from("ICMP"));
            draw_items.push(String::from("TCP_PROBE"));
            draw_items.push(String::from("UDP_PROBE"));
            draw_items.push(String::from("ButtonKillNetwork"));
        }

        //Clear and redraw
        layer.clear();

        for item in draw_items {
            elements.get_mut(item).unwrap().draw(layer);
        }

        self.current_ui_state = new_ui_state;
    }
}
