mod app;
mod charts;
mod config;
mod device_scanner;
mod sensor;
mod timeseries;

use app::ZenSignal;
use iced::Theme;
use sensor::{start_data_collection, Handler, SensorUpdate};
use std::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::runtime::Runtime;
use tokio::sync::RwLock;

fn main() -> iced::Result {
    // Create a channel for communication between the data collection thread and the UI thread
    let (sender, receiver) = mpsc::channel::<SensorUpdate>();
    
    // Create a channel for connection commands
    let (connect_sender, connect_receiver) = mpsc::channel::<app::ConnectionCommand>();

    // Spawn a thread to handle connection requests
    std::thread::spawn(move || {
        let rt: Runtime = Runtime::new().unwrap();
        let mut stop_flag: Option<Arc<AtomicBool>> = None;
        
        // Wait for connection commands
        while let Ok(command) = connect_receiver.recv() {
            match command {
                app::ConnectionCommand::Connect(device_id) => {
                    println!("Main: Connecting to device: {}", device_id);
                    
                    // Create a new stop flag for this connection
                    let should_stop = Arc::new(AtomicBool::new(false));
                    stop_flag = Some(should_stop.clone());
                    
                    let sender_clone = sender.clone();
                    
                    // Spawn the connection task instead of blocking
                    rt.spawn(async move {
                        match arctic::PolarSensor::new(device_id.clone()).await {
                            Ok(sensor) => {
                                let polar = Arc::new(RwLock::new(sensor));
                                let handler = Handler::new(sender_clone.clone());
                                start_data_collection(polar, handler, should_stop).await;
                            }
                            Err(e) => {
                                println!("Failed to connect to device: {:?}", e);
                                let _ = sender_clone.send(SensorUpdate::ConnectionStatus(
                                    sensor::ConnectionStatus::Error(format!("{:?}", e))
                                ));
                            }
                        }
                    });
                }
                app::ConnectionCommand::Disconnect => {
                    println!("Main: Disconnect requested");
                    if let Some(flag) = &stop_flag {
                        println!("Main: Setting stop flag");
                        flag.store(true, Ordering::Relaxed);
                    }
                    stop_flag = None;
                }
            }
        }
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

