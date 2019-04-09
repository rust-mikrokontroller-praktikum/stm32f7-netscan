use super::uielement::UiElement;
use alloc::string::String;
use stm32f7_discovery::lcd::Color;
use stm32f7_discovery::lcd::Framebuffer;
use stm32f7_discovery::lcd::Layer;

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

impl ButtonText {
    pub fn new(
        x_pos: usize,
        y_pos: usize,
        x_size: usize,
        y_size: usize,
        text: String,
    ) -> ButtonText {
        ButtonText {
            x_pos: x_pos,
            y_pos: y_pos,
            x_size: x_size,
            y_size: y_size,
            text: text,
            background_color: Color {
                red: 0,
                green: 255,
                blue: 0,
                alpha: 255,
            },
            text_color: Color {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            },
        }
    }
}

impl<T: Framebuffer> UiElement<T> for ButtonText {
    fn get_x_pos(&mut self) -> usize {
        self.x_pos
    }

    fn get_y_pos(&mut self) -> usize {
        self.y_pos
    }

    fn get_x_size(&mut self) -> usize {
        self.x_size
    }

    fn get_y_size(&mut self) -> usize {
        self.y_size
    }

    fn set_text(&mut self, text: String) {
        self.text = text;
    }

    fn get_background_color(&mut self) -> Color {
        self.background_color
    }

    fn set_background_color(&mut self, color: Color) {
        self.background_color = color;
    }

    fn get_text_color(&mut self) -> Color {
        self.text_color
    }

    fn set_text_color(&mut self, color: Color) {
        self.text_color = color;
    }

    // fn run_touch_func(&mut self){
    //     (self.touch)()
    // }

    fn draw(&mut self, layer: &mut Layer<T>) {
        use font8x8::{self, UnicodeFonts};

        for x in self.x_pos..self.x_pos + self.x_size {
            for y in self.y_pos..self.y_pos + self.y_size {
                layer.print_point_color_at(x, y, self.background_color);
            }
        }

        let mut temp_x_pos = self.x_pos;
        let mut temp_y_pos = self.y_pos;

        for c in self.text.chars() {
            if c == '\n' {
                temp_y_pos += 8;
                temp_x_pos = self.x_pos;
                continue;
            }
            match c {
                ' '..='~' => {
                    let rendered = font8x8::BASIC_FONTS
                        .get(c)
                        .expect("character not found in basic font");
                    for (y, byte) in rendered.iter().enumerate() {
                        for (x, bit) in (0..8).enumerate() {
                            //TODO remove alpha
                            let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };
                            if alpha != 0 {
                                layer.print_point_color_at(
                                    temp_x_pos + x,
                                    temp_y_pos + y,
                                    self.text_color,
                                );
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
