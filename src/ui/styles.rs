//! # UI Styling Module
//!
//! Centralized styling utilities for consistent UI appearance across components.
//! Extracts complex button and widget styling logic for reusability.

use iced::widget::button;
use iced::{Background, Border, Color};

/// Style for device list buttons based on selection state
pub fn device_button_style(is_selected: bool) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_theme: &iced::Theme, status: button::Status| {
        match status {
            button::Status::Active => {
                if is_selected {
                    // Selected: Teal/Cyan background for visual feedback
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.2, 0.6, 0.7))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.3, 0.7, 0.8),
                            width: 2.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                } else {
                    // Unselected: Neutral gray background
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.4, 0.4, 0.4))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.5, 0.5, 0.5),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                }
            }
            button::Status::Hovered => {
                if is_selected {
                    // Hover on selected: Brighter teal
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.3, 0.7, 0.8))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.4, 0.8, 0.9),
                            width: 2.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                } else {
                    // Hover on unselected: Lighter gray
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.5, 0.5, 0.5))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.6, 0.6, 0.6),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                }
            }
            button::Status::Pressed => {
                if is_selected {
                    // Pressed on selected: Darker teal
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.15, 0.5, 0.6))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.2, 0.6, 0.7),
                            width: 2.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                } else {
                    // Pressed on unselected: Darker gray
                    button::Style {
                        background: Some(Background::Color(Color::from_rgb(0.35, 0.35, 0.35))),
                        text_color: Color::WHITE,
                        border: Border {
                            color: Color::from_rgb(0.45, 0.45, 0.45),
                            width: 1.0,
                            radius: 4.0.into(),
                        },
                        ..Default::default()
                    }
                }
            }
            button::Status::Disabled => {
                // Disabled state: Muted appearance
                button::Style {
                    background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))),
                    text_color: Color::from_rgb(0.6, 0.6, 0.6),
                    border: Border {
                        color: Color::from_rgb(0.4, 0.4, 0.4),
                        width: 1.0,
                        radius: 4.0.into(),
                    },
                    ..Default::default()
                }
            }
        }
    }
}

/// Style for connect button (green theme)
pub fn connect_button_style() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    |_theme: &iced::Theme, status: button::Status| match status {
        button::Status::Active => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.2, 0.7, 0.2))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.3, 0.8, 0.3),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.3, 0.8, 0.3))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.4, 0.9, 0.4),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.15, 0.6, 0.15))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.2, 0.7, 0.2),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        },
        _ => button::Style::default(),
    }
}

/// Style for disconnect button (red theme)
pub fn disconnect_button_style() -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    |_theme: &iced::Theme, status: button::Status| match status {
        button::Status::Active => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.8, 0.2, 0.2))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.9, 0.3, 0.3),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        },
        button::Status::Hovered => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.9, 0.3, 0.3))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(1.0, 0.4, 0.4),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        },
        button::Status::Pressed => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.7, 0.15, 0.15))),
            text_color: Color::WHITE,
            border: Border {
                color: Color::from_rgb(0.8, 0.2, 0.2),
                width: 1.0,
                radius: 4.0.into(),
            },
            ..Default::default()
        },
        _ => button::Style::default(),
    }
}
