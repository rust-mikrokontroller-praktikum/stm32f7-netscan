use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use stm32f7_discovery::lcd::Color;
use stm32f7_discovery::lcd::Framebuffer;
use stm32f7_discovery::lcd::Layer;

pub trait UiElement<T: Framebuffer>: Any {
    fn get_x_pos(&mut self) -> usize;
    fn get_y_pos(&mut self) -> usize;
    fn get_x_size(&mut self) -> usize;
    fn get_y_size(&mut self) -> usize;

    fn get_background_color(&mut self) -> Color;
    fn set_background_color(&mut self, color: Color);
    fn get_text_color(&mut self) -> Color;
    fn set_text_color(&mut self, color: Color);

    //fn run_touch_func(&mut self);

    fn draw(&mut self, layer: &mut Layer<T>);

    fn set_text(&mut self, _text: String) {
        println!("set_text called for unimplemented struct")
    }

    fn set_lines(&mut self, _lines: Vec<String>) {
        println!("set_lines called for unimplemented struct")
    }

    fn set_lines_no_scroll(&mut self, _lines: Vec<String>) {
        println!("set_lines_no_scroll called for unimplemented struct")
    }

    fn add_line(&mut self, _line: String) {
        println!("add_line called for unimplemented struct")
    }

    fn add_lines(&mut self, mut _lines: Vec<String>) {
        println!("add_lines called for unimplemented struct")
    }

    fn set_lines_start(&mut self, _lines_start: usize) {
        println!("set_lines_start called for unimplemented struct")
    }

    fn get_lines_start(&mut self) -> usize {
        println!("get_lines_start called for unimplemented struct");
        0
    }

    fn set_title(&mut self, _title: String) {
        println!("set_title called for unimplemented struct");
    }

    fn get_lines(&mut self) -> Vec<String> {
        println!("get_lines called for unimplemented struct");
        vec![]
    }
}
