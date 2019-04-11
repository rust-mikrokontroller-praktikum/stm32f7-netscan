use super::uielement::UiElement;
use alloc::string::String;
use alloc::vec::Vec;
use stm32f7_discovery::lcd::Color;
use stm32f7_discovery::lcd::Framebuffer;
use stm32f7_discovery::lcd::Layer;

// A scrollable Text Box
pub struct ScrollableText {
    x_pos: usize,
    y_pos: usize,
    x_size: usize,
    y_size: usize,
    title: String,
    lines: Vec<String>,
    lines_start: usize,
    autoscroll: bool,
    background_color: Color,
    text_color: Color,
}

impl ScrollableText {
    pub fn new(
        x_pos: usize,
        y_pos: usize,
        x_size: usize,
        y_size: usize,
        lines: Vec<String>,
    ) -> ScrollableText {
        ScrollableText {
            x_pos: x_pos,
            y_pos: y_pos,
            x_size: x_size,
            y_size: y_size,
            title: String::from(""),
            lines: lines,
            lines_start: 0,
            autoscroll: false,
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

impl<T: Framebuffer> UiElement<T> for ScrollableText {
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
        self.lines = vec![text];
    }

    fn set_lines(&mut self, lines: Vec<String>) {
        self.lines_start = 0;
        self.lines = lines;
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

    fn add_line(&mut self, line: String) {
        self.lines.push(line);

        if self.autoscroll && self.lines.len() > ((self.y_size / 8) - 1) {
            self.lines_start += 1;
        }
    }

    fn add_lines(&mut self, mut lines: Vec<String>) {
        self.lines.append(&mut lines);

        if self.autoscroll && self.lines.len() > ((self.y_size / 8) - 1) {
            self.lines_start += lines.len();
        }
    }

    // Set the first line that is drawn
    fn set_lines_start(&mut self, lines_start: usize) {
        if lines_start > self.lines.len() - (self.y_size / 8 - 1) {
            println!("lines_start > lines.len - lines_show");
        } else {
            self.lines_start = lines_start;
        }
    }

    fn get_lines(&mut self) -> Vec<String> {
        self.lines.clone()
    }

    fn get_lines_start(&mut self) -> usize {
        self.lines_start
    }

    fn set_title(&mut self, title: String) {
        self.title = title;
    }

    // fn run_touch_func(&mut self){
    // }

    // Draws the element on the given layer
    fn draw(&mut self, layer: &mut Layer<T>) {
        use font8x8::{self, UnicodeFonts};

        // Draw the background
        for x in self.x_pos..self.x_pos + self.x_size {
            for y in self.y_pos..self.y_pos + self.y_size {
                layer.print_point_color_at(x, y, self.background_color);
            }
        }

        let mut temp_x_pos = self.x_pos;
        let mut temp_y_pos = self.y_pos;
        let mut count_lines_start = 0;
        let mut count_lines_show = 0;

        //println!("Number of lines {}", lines_split.len());

        // Draw the title of the box
        for c in self.title.chars() {
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
                            if alpha != 0 {
                                layer.print_point_color_at(temp_x_pos + x, temp_y_pos + y, color);
                            }
                        }
                    }
                }
                _ => panic!("unprintable character"),
            }
            temp_x_pos += 8;
        }

        temp_x_pos = self.x_pos;
        temp_y_pos += 8;

        // Draw the lines
        for line in self.lines.iter() {
            if count_lines_start < self.lines_start {
                // Skip lines until the start line is reached
                //println!("Skip line");
            } else if count_lines_show >= ((self.y_size / 8) - 1) {
                // No more free lines
                //println!("End line");
                break;
            } else {
                // Draw the characters
                for c in line.chars() {
                    // New line
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
                                    let alpha = if *byte & (1 << bit) == 0 { 0 } else { 255 };
                                    let mut color = self.text_color;
                                    color.alpha = alpha;
                                    if alpha != 0 {
                                        layer.print_point_color_at(
                                            temp_x_pos + x,
                                            temp_y_pos + y,
                                            color,
                                        );
                                    }
                                }
                            }
                        }
                        _ => panic!("unprintable character"),
                    }
                    temp_x_pos += 8;

                    // New line if the line is full
                    if temp_x_pos >= (self.x_pos + self.x_size - 8) {
                        temp_y_pos += 8;
                        temp_x_pos = self.x_pos;

                        count_lines_show += 1;
                    }
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
