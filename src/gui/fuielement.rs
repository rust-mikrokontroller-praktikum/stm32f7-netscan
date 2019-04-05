use alloc::boxed::Box;
use stm32f7_discovery::lcd::FramebufferArgb8888;
use super::uielement::UiElement;

pub type FUiElement = Box<UiElement<FramebufferArgb8888>>;
