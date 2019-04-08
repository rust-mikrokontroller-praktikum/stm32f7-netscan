use alloc::string::String;
use alloc::vec::Vec;
use alloc::boxed::Box;
use stm32f7_discovery::lcd::Color;
use stm32f7_discovery::lcd::FramebufferArgb8888;
use stm32f7_discovery::lcd::Layer;
use super::buttontext::ButtonText;
use super::scrollabletext::ScrollableText;
use super::uistates::UiStates;
use super::uielement::UiElement;
use super::fuielement::FUiElement;
use alloc::collections::btree_map::BTreeMap;

pub struct UiState{
    current_ui_state: UiStates
}

impl UiState {
    pub fn new() -> UiState{
        UiState{
            current_ui_state: UiStates::Initialization,
        }
    }

    pub fn get_ui_state(&mut self) -> UiStates{
        self.current_ui_state
    }

    pub fn change_ui_state(&mut self, layer: &mut Layer<FramebufferArgb8888>, draw_items: &mut Vec<String>, elements: &mut BTreeMap<String, FUiElement>, new_ui_state: UiStates){

        // Clear everything
        draw_items.clear();

        //Initialization
        elements.insert(String::from("INIT_ETHERNET"), Box::new(ButtonText::new(200, 111, 80, 50, String::from("ETH"))));

        //Address
        elements.insert(String::from("INIT_DHCP"), Box::new(ButtonText::new(110, 111, 80, 50, String::from("DHCP"))));
        elements.insert(String::from("INIT_LISTEN"), Box::new(ButtonText::new(200, 111, 80, 50, String::from("Listen"))));
        elements.insert(String::from("INIT_GLOBAL"), Box::new(ButtonText::new(290, 111, 80, 50, String::from("Global"))));

        // elements.insert(String::from("INIT_STATIC"), Box::new(ButtonText::new(245, 111, 80, 50, String::from("STATIC"))));

        //Start
        elements.insert(String::from("ScrollText"), Box::new(ScrollableText::new(5, 5, 300, 262, Vec::new())));

        elements.insert(String::from("ButtonScrollUp"), Box::new(ButtonText::new(310, 5, 80, 50, String::from("UP"))));

        elements.insert(String::from("ButtonScrollDown"), Box::new(ButtonText::new(310, 60, 80, 50, String::from("DOWN"))));

        elements.insert(String::from("ARP_SCAN"), Box::new(ButtonText::new(395, 5, 80, 50, String::from("ARP"))));

        elements.insert(String::from("ICMP"), Box::new(ButtonText::new(395, 60, 80, 50, String::from("ICMP"))));

        elements.insert(String::from("TCP_PROBE"), Box::new(ButtonText::new(395, 115, 80, 50, String::from("TCP PROBE"))));

        elements.insert(String::from("ButtonInfo"), Box::new(ButtonText::new(310, 217, 80, 50, String::from("INFO"))));

        //.set_background_color(Color{red: 255, green: 0, blue: 0, alpha: 255}
        let mut button_kill: FUiElement = Box::new(ButtonText::new(395, 217, 80, 50, String::from("KILL")));
        button_kill.set_background_color(Color{red: 255, green: 165, blue: 0, alpha: 255});
        elements.insert(String::from("ButtonKill"), button_kill);

        //elements.insert(String::from("ButtonHome"), Box::new(ButtonText::new(400, 222, 80, 50, String::from("HOME"))));

        if new_ui_state == UiStates::Initialization{
            draw_items.push(String::from("INIT_ETHERNET"));
        } else if new_ui_state == UiStates::Address{
            draw_items.push(String::from("INIT_DHCP"));
            // draw_items.push(String::from("INIT_STATIC"));
            draw_items.push(String::from("INIT_LISTEN"));
            draw_items.push(String::from("INIT_GLOBAL"));
        } else if new_ui_state == UiStates::Start{
            draw_items.push(String::from("ScrollText"));

            draw_items.push(String::from("ButtonScrollUp"));
            draw_items.push(String::from("ButtonScrollDown"));
            draw_items.push(String::from("ButtonInfo"));

            draw_items.push(String::from("ARP_SCAN"));
            draw_items.push(String::from("ICMP"));
            draw_items.push(String::from("TCP_PROBE"));
            draw_items.push(String::from("ButtonKill"));

        }


        //Clear and redraw
        layer.clear();

        for item in draw_items {
            elements.get_mut(item).unwrap().draw(layer);
        }

        self.current_ui_state = new_ui_state;
    }
}
