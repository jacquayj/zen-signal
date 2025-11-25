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
    SampleRateConfig { ecg_rate: u64, acc_rate: u64 },
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

async fn connect_to_device(
    polar: &mut PolarSensor,
    handler: &Handler,
    should_stop: &Arc<AtomicBool>,
) -> Result<(), ()> {
    print!("Connecting");
    while !polar.is_connected().await {
        if should_stop.load(Ordering::Relaxed) {
            let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Disconnected));
            return Err(());
        }
        
        print!(".");
        match polar.connect().await {
            Err(arctic::Error::NoBleAdaptor) => {
                println!("No bluetooth adapter found");
                let _ = handler.sender.send(SensorUpdate::ConnectionStatus(
                    ConnectionStatus::Error("No bluetooth adapter found".to_string())
                ));
                return Err(());
            }
            Err(why) => {
                println!("Could not connect: {:?}", why);
                if should_stop.load(Ordering::Relaxed) {
                    let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Disconnected));
                    return Err(());
                }
            }
            _ => {}
        }
    }
    println!("Connected");
    let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Connected));
    Ok(())
}

async fn subscribe_to_streams(polar: &mut PolarSensor) {
    if let Err(why) = polar.subscribe(arctic::NotifyStream::HeartRate).await {
        println!("Could not subscribe to heart rate notifications: {:?}", why)
    }
    if let Err(why) = polar.subscribe(arctic::NotifyStream::MeasurementData).await {
        println!("Could not subscribe to measurement data notifications: {:?}", why)
    }
}

fn setup_data_types(polar: &mut PolarSensor) {
    polar.data_type_push(arctic::H10MeasurementType::Acc);
    polar.data_type_push(arctic::H10MeasurementType::Ecg);
}

async fn configure_sample_rates(polar: &mut PolarSensor) -> (u64, u64) {
    let mut configured_ecg_rate = 130u64; // default
    let mut configured_acc_rate = 200u64; // default
    
    match polar.settings().await {
        Ok(settings) => {
            for setting in settings {
                match setting.measurement_type() {
                    arctic::H10MeasurementType::Ecg => {
                        let sample_rates = setting.sample_rate();
                        if !sample_rates.is_empty() {
                            if let Some(&max_rate) = sample_rates.iter().max() {
                                println!("Available ECG sample rates: {:?}", sample_rates);
                                println!("Setting ECG sample rate to maximum: {} Hz", max_rate);
                                if let Err(e) = polar.ecg_sample_rate(max_rate) {
                                    println!("Failed to set ECG sample rate: {:?}", e);
                                } else {
                                    configured_ecg_rate = max_rate as u64;
                                }
                            }
                        }
                    }
                    arctic::H10MeasurementType::Acc => {
                        let sample_rates = setting.sample_rate();
                        if !sample_rates.is_empty() {
                            if let Some(&max_rate) = sample_rates.iter().max() {
                                println!("Available ACC sample rates: {:?}", sample_rates);
                                println!("Setting ACC sample rate to maximum: {} Hz", max_rate);
                                if let Err(e) = polar.acc_sample_rate(max_rate) {
                                    println!("Failed to set ACC sample rate: {:?}", e);
                                } else {
                                    configured_acc_rate = max_rate as u64;
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(e) => {
            println!("Could not query device settings: {:?}", e);
        }
    }
    
    (configured_ecg_rate, configured_acc_rate)
}

async fn run_event_loop(
    polar: &PolarSensor,
    handler: &Handler,
    should_stop: &Arc<AtomicBool>,
) {
    tokio::select! {
        result = polar.event_loop() => {
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

pub async fn start_data_collection(
    polar: Arc<RwLock<PolarSensor>>,
    handler: Handler,
    should_stop: Arc<AtomicBool>,
) {
    println!("Attempting connection");
    let _ = handler.sender.send(SensorUpdate::ConnectionStatus(ConnectionStatus::Connecting));

    {
        let mut polar = polar.write().await;

        if connect_to_device(&mut polar, &handler, &should_stop).await.is_err() {
            return;
        }

        subscribe_to_streams(&mut polar).await;
        setup_data_types(&mut polar);

        let (ecg_rate, acc_rate) = configure_sample_rates(&mut polar).await;

        // Send the configured sample rates to the UI thread
        let _ = handler.sender.send(SensorUpdate::SampleRateConfig {
            ecg_rate,
            acc_rate,
        });

        polar.event_handler(handler.clone());
    }

    // Run event loop with cancellation check
    let polar_guard = polar.read().await;
    run_event_loop(&polar_guard, &handler, &should_stop).await;
}
