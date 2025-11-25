use crate::charts::{AccChartType, EcgChartType, HrChartType, RrChartType};
use crate::device_scanner::{scan_devices, BluetoothDevice};
use crate::sensor::SensorUpdate;
use crate::timeseries::Channels;
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length, Subscription, Task};
use plotters_iced::ChartWidget;
use std::sync::mpsc::Receiver;

#[derive(Debug, Clone)]
pub enum ConnectionCommand {
    Connect(String),
    Disconnect,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Scanning,
    Connecting,
    Connected,
}

// Iced Application State
pub struct ZenSignal {
    pub channels: Channels,
    receiver: Receiver<SensorUpdate>,
    pub connection_state: ConnectionState,
    pub available_devices: Vec<BluetoothDevice>,
    pub selected_device: Option<BluetoothDevice>,
    connect_sender: std::sync::mpsc::Sender<ConnectionCommand>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    ScanDevices,
    DevicesScanned(Result<Vec<BluetoothDevice>, String>),
    SelectDevice(BluetoothDevice),
    ConnectDevice,
    DisconnectDevice,
}

impl ZenSignal {
    pub fn new(
        receiver: Receiver<SensorUpdate>,
        connect_sender: std::sync::mpsc::Sender<ConnectionCommand>,
    ) -> (Self, Task<Message>) {
        (
            ZenSignal {
                channels: Channels::new(),
                receiver,
                connection_state: ConnectionState::Disconnected,
                available_devices: Vec::new(),
                selected_device: None,
                connect_sender,
            },
            Task::none(),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                // Process all pending messages without blocking
                loop {
                    match self.receiver.try_recv() {
                        Ok(update) => {
                            match update {
                                SensorUpdate::ConnectionStatus(status) => {
                                    use crate::sensor::ConnectionStatus;
                                    match status {
                                        ConnectionStatus::Connecting => {
                                            self.connection_state = ConnectionState::Connecting;
                                        }
                                        ConnectionStatus::Connected => {
                                            self.connection_state = ConnectionState::Connected;
                                        }
                                        ConnectionStatus::Disconnected => {
                                            self.connection_state = ConnectionState::Disconnected;
                                            self.channels = Channels::new();
                                        }
                                        ConnectionStatus::Error(e) => {
                                            println!("Connection error: {}", e);
                                            self.connection_state = ConnectionState::Disconnected;
                                        }
                                    }
                                }
                                SensorUpdate::HeartRate(hr) => {
                                    self.channels.handle_heart_rate(hr);
                                }
                                SensorUpdate::MeasurementData(data) => {
                                    self.channels.handle_measurement_data(data);
                                }
                                SensorUpdate::SampleRateConfig { ecg_rate, acc_rate } => {
                                    println!("Updating sample rates: ECG={} Hz, ACC={} Hz", ecg_rate, acc_rate);
                                    self.channels.set_ecg_sample_rate(ecg_rate);
                                    self.channels.set_acc_sample_rate(acc_rate);
                                }
                            }
                        }
                        Err(_) => {
                            // No more messages
                            break;
                        }
                    }
                }
                Task::none()
            }
            Message::ScanDevices => {
                self.connection_state = ConnectionState::Scanning;
                self.available_devices.clear();
                Task::perform(scan_devices(), Message::DevicesScanned)
            }
            Message::DevicesScanned(result) => {
                self.connection_state = ConnectionState::Disconnected;
                match result {
                    Ok(devices) => {
                        self.available_devices = devices;
                    }
                    Err(e) => {
                        println!("Error scanning devices: {}", e);
                    }
                }
                Task::none()
            }
            Message::SelectDevice(device) => {
                self.selected_device = Some(device);
                Task::none()
            }
            Message::ConnectDevice => {
                if let Some(device) = &self.selected_device {
                    self.connection_state = ConnectionState::Connecting;
                    if let Err(e) = self.connect_sender.send(ConnectionCommand::Connect(device.id.clone())) {
                        println!("Failed to send connection request: {}", e);
                        self.connection_state = ConnectionState::Disconnected;
                    }
                }
                Task::none()
            }
            Message::DisconnectDevice => {
                println!("UI: Sending disconnect command");
                if let Err(e) = self.connect_sender.send(ConnectionCommand::Disconnect) {
                    println!("Failed to send disconnect request: {}", e);
                } else {
                    println!("UI: Disconnect command sent successfully");
                }
                // State will be updated when we receive ConnectionStatus::Disconnected
                Task::none()
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        iced::time::every(std::time::Duration::from_millis(16)).map(|_| Message::Tick)
    }

    pub fn view(&'_ self) -> Element<'_, Message> {
        let sidebar = self.create_sidebar();
        
        let main_content = if self.connection_state == ConnectionState::Connected {
            self.create_main_view()
        } else {
            self.create_disconnected_view()
        };

        let content = row![sidebar, main_content].spacing(0);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn create_sidebar(&self) -> Element<'_, Message> {
        let title = text("Bluetooth Devices").size(20);

        let scan_button = button(
            text(if self.connection_state == ConnectionState::Scanning {
                "Scanning..."
            } else {
                "Scan for Devices"
            })
        )
        .on_press_maybe(
            if self.connection_state == ConnectionState::Disconnected {
                Some(Message::ScanDevices)
            } else {
                None
            }
        )
        .padding(10);

        let device_list: Element<'_, Message> = if self.available_devices.is_empty() {
            text("No devices found. Click 'Scan for Devices' to start.").into()
        } else {
            let is_connected = self.connection_state == ConnectionState::Connected;
            let devices = self.available_devices.iter().map(|device| {
                let is_selected = self.selected_device.as_ref() == Some(device);
                let device_text = format!("{}\n{}", device.name, device.id);
                
                button(text(device_text).size(14))
                    .on_press_maybe(
                        if is_connected {
                            None
                        } else {
                            Some(Message::SelectDevice(device.clone()))
                        }
                    )
                    .width(Length::Fill)
                    .padding(10)
                    .style(move |_theme: &iced::Theme, status| {
                        match status {
                            button::Status::Active => {
                                if is_selected {
                                    // Selected: Teal/Cyan background
                                    button::Style {
                                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.2, 0.6, 0.7))),
                                        text_color: iced::Color::WHITE,
                                        border: iced::Border {
                                            color: iced::Color::from_rgb(0.3, 0.7, 0.8),
                                            width: 2.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                } else {
                                    // Unselected: Gray background
                                    button::Style {
                                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.4, 0.4, 0.4))),
                                        text_color: iced::Color::WHITE,
                                        border: iced::Border {
                                            color: iced::Color::from_rgb(0.5, 0.5, 0.5),
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                }
                            }
                            button::Status::Hovered => {
                                if is_selected {
                                    button::Style {
                                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.3, 0.7, 0.8))),
                                        text_color: iced::Color::WHITE,
                                        border: iced::Border {
                                            color: iced::Color::from_rgb(0.4, 0.8, 0.9),
                                            width: 2.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                } else {
                                    button::Style {
                                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.5, 0.5, 0.5))),
                                        text_color: iced::Color::WHITE,
                                        border: iced::Border {
                                            color: iced::Color::from_rgb(0.6, 0.6, 0.6),
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                }
                            }
                            button::Status::Pressed => {
                                if is_selected {
                                    button::Style {
                                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.15, 0.5, 0.6))),
                                        text_color: iced::Color::WHITE,
                                        border: iced::Border {
                                            color: iced::Color::from_rgb(0.2, 0.6, 0.7),
                                            width: 2.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                } else {
                                    button::Style {
                                        background: Some(iced::Background::Color(iced::Color::from_rgb(0.35, 0.35, 0.35))),
                                        text_color: iced::Color::WHITE,
                                        border: iced::Border {
                                            color: iced::Color::from_rgb(0.45, 0.45, 0.45),
                                            width: 1.0,
                                            radius: 4.0.into(),
                                        },
                                        ..Default::default()
                                    }
                                }
                            }
                            button::Status::Disabled => {
                                button::Style {
                                    background: Some(iced::Background::Color(iced::Color::from_rgb(0.3, 0.3, 0.3))),
                                    text_color: iced::Color::from_rgb(0.6, 0.6, 0.6),
                                    border: iced::Border {
                                        color: iced::Color::from_rgb(0.4, 0.4, 0.4),
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                }
                            }
                        }
                    })
                    .into()
            });

            scrollable(column(devices).spacing(5)).into()
        };

        let connect_button = match self.connection_state {
            ConnectionState::Connected => {
                button(text("Disconnect"))
                    .on_press(Message::DisconnectDevice)
                    .padding(10)
                    .width(Length::Fill)
                    .style(|theme: &iced::Theme, status| {
                        match status {
                            button::Status::Active => button::Style {
                                background: Some(iced::Background::Color(iced::Color::from_rgb(0.8, 0.2, 0.2))),
                                text_color: iced::Color::WHITE,
                                border: iced::Border {
                                    color: iced::Color::from_rgb(0.9, 0.3, 0.3),
                                    width: 1.0,
                                    radius: 4.0.into(),
                                },
                                ..Default::default()
                            },
                            button::Status::Hovered => button::Style {
                                background: Some(iced::Background::Color(iced::Color::from_rgb(0.9, 0.3, 0.3))),
                                text_color: iced::Color::WHITE,
                                border: iced::Border {
                                    color: iced::Color::from_rgb(1.0, 0.4, 0.4),
                                    width: 1.0,
                                    radius: 4.0.into(),
                                },
                                ..Default::default()
                            },
                            button::Status::Pressed => button::Style {
                                background: Some(iced::Background::Color(iced::Color::from_rgb(0.7, 0.15, 0.15))),
                                text_color: iced::Color::WHITE,
                                border: iced::Border {
                                    color: iced::Color::from_rgb(0.8, 0.2, 0.2),
                                    width: 1.0,
                                    radius: 4.0.into(),
                                },
                                ..Default::default()
                            },
                            _ => button::primary(theme, status),
                        }
                    })
            }
            ConnectionState::Connecting => {
                button(text("Connecting..."))
                    .padding(10)
                    .width(Length::Fill)
            }
            _ => {
                if let Some(_) = &self.selected_device {
                    button(text("Connect"))
                        .on_press(Message::ConnectDevice)
                        .padding(10)
                        .width(Length::Fill)
                        .style(|theme: &iced::Theme, status| {
                            match status {
                                button::Status::Active => button::Style {
                                    background: Some(iced::Background::Color(iced::Color::from_rgb(0.2, 0.7, 0.2))),
                                    text_color: iced::Color::WHITE,
                                    border: iced::Border {
                                        color: iced::Color::from_rgb(0.3, 0.8, 0.3),
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                },
                                button::Status::Hovered => button::Style {
                                    background: Some(iced::Background::Color(iced::Color::from_rgb(0.3, 0.8, 0.3))),
                                    text_color: iced::Color::WHITE,
                                    border: iced::Border {
                                        color: iced::Color::from_rgb(0.4, 0.9, 0.4),
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                },
                                button::Status::Pressed => button::Style {
                                    background: Some(iced::Background::Color(iced::Color::from_rgb(0.15, 0.6, 0.15))),
                                    text_color: iced::Color::WHITE,
                                    border: iced::Border {
                                        color: iced::Color::from_rgb(0.2, 0.7, 0.2),
                                        width: 1.0,
                                        radius: 4.0.into(),
                                    },
                                    ..Default::default()
                                },
                                _ => button::primary(theme, status),
                            }
                        })
                } else {
                    button(text("Select a device"))
                        .padding(10)
                        .width(Length::Fill)
                }
            }
        };

        let sidebar_content = column![title, scan_button, device_list, connect_button]
            .spacing(10)
            .padding(20)
            .width(300);

        container(sidebar_content)
            .style(container::bordered_box)
            .width(Length::Fixed(300.0))
            .height(Length::Fill)
            .into()
    }

    fn create_disconnected_view(&self) -> Element<'_, Message> {
        let message = match self.connection_state {
            ConnectionState::Scanning => "Scanning for devices...",
            ConnectionState::Connecting => "Connecting to device...",
            _ => "Select a Polar device from the sidebar to begin",
        };

        container(
            column![text(message).size(24)]
                .width(Length::Fill)
                .align_x(iced::alignment::Horizontal::Center)
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center(Length::Fill)
        .into()
    }

    fn create_main_view(&self) -> Element<'_, Message> {
        let hr_series = &self.channels.hr;
        let last_hr_point = hr_series.last_points(1);
        let hr = last_hr_point.last().map(|point| point.value).unwrap_or(0);

        let ecg_chart = ChartWidget::new(EcgChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let hr_chart = ChartWidget::new(HrChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let rr_chart = ChartWidget::new(RrChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let acc_chart = ChartWidget::new(AccChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let stats = column![text(format!("Heart Rate: {}", hr)).size(24)]
            .width(Length::FillPortion(1));

        let plots = column![ecg_chart, hr_chart, rr_chart, acc_chart]
            .width(Length::FillPortion(3))
            .spacing(10);

        let content = row![plots, stats].spacing(20).padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
