use arctic::{self, PolarSensor};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::RwLock;

// Enable clone and debug for the SensorUpdate enum
#[derive(Debug)]
pub enum SensorUpdate {
    HeartRate(arctic::HeartRate),
    MeasurementData(arctic::PmdRead),
    ConnectionStatus(ConnectionStatus),
}

#[derive(Debug, Clone)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Disconnected,
    Error(String),
}

#[derive(Clone)]
pub struct Handler {
    sender: Sender<SensorUpdate>,
}

impl Handler {
    pub fn new(sender: Sender<SensorUpdate>) -> Self {
        Self { sender }
    }
}

// Send data to UI thread
#[arctic::async_trait]
impl arctic::EventHandler for Handler {
    async fn heart_rate_update(&self, _ctx: &arctic::PolarSensor, heartrate: arctic::HeartRate) {
        if let Err(why) = self.sender.send(SensorUpdate::HeartRate(heartrate)) {
            println!("Could not send heart rate data: {:?}", why);
        }
    }

    async fn measurement_update(&self, _ctx: &arctic::PolarSensor, data: arctic::PmdRead) {
        if let Err(why) = self.sender.send(SensorUpdate::MeasurementData(data)) {
            println!("Could not send heart rate data: {:?}", why);
        }
    }
}

pub async fn start_data_collection(
    polar: Arc<RwLock<PolarSensor>>,
    handler: Handler,
    should_stop: Arc<AtomicBool>,
) {
    println!("Attempting connection");
    let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Connecting));

    {
        let mut polar = polar.write().await;

        print!("Connecting");
        while !polar.is_connected().await {
            if should_stop.load(Ordering::Relaxed) {
                let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Disconnected));
                return;
            }
            
            print!(".");
            match polar.connect().await {
                Err(arctic::Error::NoBleAdaptor) => {
                    println!("No bluetooth adapter found");
                    let _ = handler.sender.send(SensorUpdate::ConnectionStatus(
                        ConnectionStatus::Error("No bluetooth adapter found".to_string())
                    ));
                    return;
                }
                Err(why) => {
                    println!("Could not connect: {:?}", why);
                    if should_stop.load(Ordering::Relaxed) {
                        let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Disconnected));
                        return;
                    }
                }
                _ => {}
            }
        }
        println!("Connected");
        let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Connected));

        if let Err(why) = polar.subscribe(arctic::NotifyStream::HeartRate).await {
            println!("Could not subscirbe to heart rate notifications: {:?}", why)
        }
        if let Err(why) = polar.subscribe(arctic::NotifyStream::MeasurementData).await {
            println!("Could not subscirbe to heart rate notifications: {:?}", why)
        }

        // Subscribe to ECG data
        polar.data_type_push(arctic::H10MeasurementType::Acc);
        polar.data_type_push(arctic::H10MeasurementType::Ecg);

        polar.event_handler(handler.clone());
    }

    // Run event loop with cancellation check
    // We need to run the event loop in a select with the stop signal
    let polar_guard = polar.read().await;
    tokio::select! {
        result = polar_guard.event_loop() => {
            println!("Event loop ended: {:?}", result);
            let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Disconnected));
        }
        _ = async {
            loop {
                if should_stop.load(Ordering::Relaxed) {
                    println!("Disconnecting...");
                    break;
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
            }
        } => {
            let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Disconnected));
        }
    }
}
