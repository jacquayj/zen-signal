// Hide console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod charts;
mod config;
mod connection;
mod device_scanner;
mod error;
mod polar_data;
mod recorder;
mod sensor;
mod timeseries;
mod ui;
mod visualization;

use app::ZenSignal;
use connection::ConnectionManager;
use iced::Theme;
use sensor::SensorUpdate;
use std::sync::mpsc;

fn main() -> iced::Result {
    // On Windows, force wgpu to use Vulkan backend instead of DirectX 12
    // This prevents "INVALID_SUBRESOURCE_STATE" errors with D3D12 resource state transitions
    #[cfg(target_os = "windows")]
    std::env::set_var("WGPU_BACKEND", "vulkan");
    
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    // Create a channel for communication between the data collection thread and the UI thread
    let (sender, receiver) = mpsc::channel::<SensorUpdate>();
    
    // Create the connection manager
    let (manager, connect_sender) = ConnectionManager::new(sender);

    // Spawn a thread to handle connection requests
    std::thread::spawn(move || {
        manager.run();
    });

    iced::application(
        "ZenSignal: Polar H10 Signal Viewer & Stress Monitor",
        ZenSignal::update,
        ZenSignal::view,
    )
    .subscription(ZenSignal::subscription)
    .theme(|_| Theme::Light)
    .window_size((1200.0, 800.0))
    .run_with(|| ZenSignal::new(receiver, connect_sender))
}

