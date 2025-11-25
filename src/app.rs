//! # Application State and UI Module
//!
//! Main application logic using the Iced GUI framework. Manages application state,
//! processes user interactions, and orchestrates the UI layout.
//!
//! ## Architecture
//! Follows Elm architecture:
//! - `ZenSignal`: Application state
//! - `Message`: User interactions and events
//! - `update()`: State transitions
//! - `view()`: UI rendering
//! - `subscription()`: Time-based updates
//!
//! ## Key State
//! - Connection state (disconnected, scanning, connecting, connected)
//! - Available and selected Bluetooth devices
//! - Sensor data channels (ECG, HR, RR, HRV, ACC)
//! - Configuration settings
//!
//! ## Data Flow
//! Sensor data arrives via mpsc channel from connection thread.
//! Connection commands are sent via separate mpsc channel.
//! UI updates at 60Hz (16ms intervals) to process pending sensor data.
//!
//! ## Why Manual Disconnect Flag
//! Prevents autoconnect from immediately reconnecting after user explicitly
//! disconnects. Reset when autoconnect is re-enabled or manually connecting.

use crate::charts::{AccChartType, EcgChartType, HrChartType, HrvChartType, RrChartType};
use crate::config::Config;
use crate::connection::ConnectionCommand;
use crate::device_scanner::{scan_devices, BluetoothDevice};
use crate::error::ScanError;
use crate::sensor::SensorUpdate;
use crate::polar_data::Channels;
use crate::ui::styles;
use iced::widget::{button, checkbox, column, container, row, scrollable, text, vertical_space};
use iced::{Element, Length, Subscription, Task};
use plotters_iced::ChartWidget;
use std::sync::mpsc::Receiver;

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
    pub config: Config,
    manual_disconnect: bool, // Track if user manually disconnected
}

#[derive(Debug, Clone)]
pub enum Message {
    Tick,
    ScanDevices,
    DevicesScanned(Result<Vec<BluetoothDevice>, ScanError>),
    SelectDevice(BluetoothDevice),
    ConnectDevice,
    DisconnectDevice,
    ToggleAutoconnect(bool),
    ToggleSmoothStreaming(bool),
}

impl ZenSignal {
    pub fn new(
        receiver: Receiver<SensorUpdate>,
        connect_sender: std::sync::mpsc::Sender<ConnectionCommand>,
    ) -> (Self, Task<Message>) {
        let config = Config::load().unwrap_or_else(|e| {
            log::error!("Failed to load config: {}, using defaults", e);
            Config::default()
        });
        let should_autoconnect = config.enable_autoconnect;
        
        (
            ZenSignal {
                channels: Channels::new(),
                receiver,
                connection_state: if should_autoconnect { ConnectionState::Scanning } else { ConnectionState::Disconnected },
                available_devices: Vec::new(),
                selected_device: None,
                connect_sender,
                config,
                manual_disconnect: false,
            },
            if should_autoconnect {
                Task::perform(scan_devices(), Message::DevicesScanned)
            } else {
                Task::none()
            },
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
                                            log::error!("Connection error: {}", e);
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
                                    log::info!("Updating sample rates: ECG={} Hz, ACC={} Hz", ecg_rate, acc_rate);
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
                        // Auto-connect to first Polar device if enabled and not manually disconnected
                        if self.config.enable_autoconnect && !self.manual_disconnect && !devices.is_empty() {
                            if let Some(polar_device) = devices.iter().find(|d| d.name.to_lowercase().contains("polar")) {
                                self.selected_device = Some(polar_device.clone());
                                self.connection_state = ConnectionState::Connecting;
                                if let Err(e) = self.connect_sender.send(ConnectionCommand::Connect(polar_device.id.clone())) {
                                    log::error!("Failed to send autoconnect request: {}", e);
                                    self.connection_state = ConnectionState::Disconnected;
                                }
                            }
                        }
                        
                        self.available_devices = devices;
                    }
                    Err(e) => {
                        log::error!("Error scanning devices: {}", e);
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
                        log::error!("Failed to send connection request: {}", e);
                        self.connection_state = ConnectionState::Disconnected;
                    }
                }
                Task::none()
            }
            Message::DisconnectDevice => {
                log::info!("Sending disconnect command");
                self.manual_disconnect = true; // Mark as manual disconnect
                if let Err(e) = self.connect_sender.send(ConnectionCommand::Disconnect) {
                    log::error!("Failed to send disconnect request: {}", e);
                } else {
                    log::debug!("Disconnect command sent successfully");
                }
                // State will be updated when we receive ConnectionStatus::Disconnected
                Task::none()
            }
            Message::ToggleAutoconnect(enabled) => {
                self.config.enable_autoconnect = enabled;
                if let Err(e) = self.config.save() {
                    log::error!("Failed to save config: {}", e);
                }
                
                // If enabling autoconnect, reset manual disconnect flag and scan
                if enabled && self.connection_state == ConnectionState::Disconnected {
                    self.manual_disconnect = false; // Reset manual disconnect flag
                    self.connection_state = ConnectionState::Scanning;
                    self.available_devices.clear();
                    return Task::perform(scan_devices(), Message::DevicesScanned);
                }
                
                Task::none()
            }
            Message::ToggleSmoothStreaming(enabled) => {
                self.config.smooth_data_streaming = enabled;
                if let Err(e) = self.config.save() {
                    log::error!("Failed to save config: {}", e);
                }
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
            let message = if self.connection_state == ConnectionState::Scanning {
                "Scanning for devices..."
            } else {
                "No devices found. Click 'Scan for Devices' to start."
            };
            text(message).into()
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
                    .style(styles::device_button_style(is_selected))
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
                    .style(styles::disconnect_button_style())
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
                        .style(styles::connect_button_style())
                } else {
                    button(text("Select a device"))
                        .padding(10)
                        .width(Length::Fill)
                }
            }
        };

        let smooth_streaming_checkbox = checkbox(
            "Smooth and Delay Streaming Data",
            self.config.smooth_data_streaming
        )
        .on_toggle(Message::ToggleSmoothStreaming);

        let autoconnect_checkbox = checkbox(
            "Enable Autoconnect",
            self.config.enable_autoconnect
        )
        .on_toggle(Message::ToggleAutoconnect);

        let sidebar_content = column![
            title,
            scan_button,
            device_list,
            connect_button,
            vertical_space(), // Push checkboxes to bottom
            smooth_streaming_checkbox,
            autoconnect_checkbox,
        ]
        .padding(20)
        .spacing(10)
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

        // Calculate RMSSD from last 30 seconds of RR data
        use crate::timeseries::PointSliceExt;
        const THIRTY_SECONDS_NS: u64 = 30_000_000_000;
        let recent_rr = self.channels.rr.last_duration(THIRTY_SECONDS_NS);
        let rmssd = if recent_rr.len() >= 2 {
            recent_rr.rmssd()
        } else {
            0.0
        };

        let ecg_chart = ChartWidget::new(EcgChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let hr_chart = ChartWidget::new(HrChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let rr_chart = ChartWidget::new(RrChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let hrv_chart = ChartWidget::new(HrvChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let acc_chart = ChartWidget::new(AccChartType { state: self })
            .width(Length::Fill)
            .height(Length::Fill);

        let stats = column![
            text(format!("Heart Rate: {} bpm", hr)).size(24),
            text(format!("RMSSD: {:.2} ms", rmssd)).size(20)
        ]
            .spacing(10)
            .width(Length::FillPortion(1));

        let plots = column![ecg_chart, hr_chart, rr_chart, hrv_chart, acc_chart]
            .width(Length::FillPortion(3))
            .spacing(10);

        let content = row![plots, stats].spacing(20).padding(20);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}
