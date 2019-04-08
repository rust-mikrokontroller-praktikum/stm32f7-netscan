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

    fn set_background_color(&mut self, color: Color);
    fn set_text_color(&mut self, color: Color);

    //fn run_touch_func(&mut self);

    fn draw(&mut self, layer: &mut Layer<T>);

    fn set_text(&mut self, text: String) {
        println!("set_text called for unimplemented struct")
    }

    fn set_lines(&mut self, lines: Vec<String>) {
        println!("set_lines called for unimplemented struct")
    }

    fn add_line(&mut self, line: String) {
        println!("add_line called for unimplemented struct")
    }

    fn set_lines_start(&mut self, lines_start: usize) {
        println!("set_lines_start called for unimplemented struct")
    }

    fn get_lines_start(&mut self) -> usize {
        println!("get_lines_start called for unimplemented struct");
        0
    }

    fn set_title(&mut self, title: String) {
        println!("set_title called for unimplemented struct");
    }
}
