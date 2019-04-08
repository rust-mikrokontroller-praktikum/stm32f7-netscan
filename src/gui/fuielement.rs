use super::uielement::UiElement;
use alloc::boxed::Box;
use stm32f7_discovery::lcd::FramebufferArgb8888;

pub type FUiElement = Box<UiElement<FramebufferArgb8888>>;
