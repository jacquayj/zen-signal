mod timeseries;

use arctic;
use gtk::cairo::Context;
use gtk::glib::ControlFlow;
use gtk::{glib, Application, ApplicationWindow};
use gtk::{prelude::*, DrawingArea};
use gtk4 as gtk;
use plotters::chart::ChartBuilder;
use plotters::drawing::IntoDrawingArea;
use plotters::series::LineSeries;
use plotters::style::{RED, WHITE};
use plotters_cairo::CairoBackend;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use tokio::runtime::Runtime;

use timeseries::{Channels, PointSliceExt, TimeSeries};

// enable clone and debug for the SensorUpdate enum
#[derive(Debug)]
enum SensorUpdate {
    HeartRate(arctic::HeartRate),
    MeasurementData(arctic::PmdRead),
}

struct Handler {
    sender: Sender<SensorUpdate>,
}

impl Handler {
    fn new(sender: Sender<SensorUpdate>) -> Self {
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

async fn start_data_collection(sender: Sender<SensorUpdate>) {
    let mut polar = arctic::PolarSensor::new("56853526".to_string())
        .await
        .expect("Invalid ID");

    println!("Attempting connection");
    while !polar.is_connected().await {
        print!(".");
        match polar.connect().await {
            Err(arctic::Error::NoBleAdaptor) => {
                println!("No bluetooth adapter found");
            }
            Err(why) => println!("Could not connect: {:?}", why),
            _ => {}
        }
    }
    println!("Connected");

    if let Err(why) = polar.subscribe(arctic::NotifyStream::HeartRate).await {
        println!("Could not subscirbe to heart rate notifications: {:?}", why)
    }
    if let Err(why) = polar.subscribe(arctic::NotifyStream::MeasurementData).await {
        println!("Could not subscirbe to heart rate notifications: {:?}", why)
    }

    // Subscribe to ECG data
    polar.data_type_push(arctic::H10MeasurementType::Acc);
    polar.data_type_push(arctic::H10MeasurementType::Ecg);

    polar.event_handler(Handler::new(sender));

    let result = polar.event_loop().await;

    println!("No more data: {:?}", result);
}

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id("com.example.ZenSignal")
        .build();

    application.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("ZenSignal: Polar H10 ECG Signal Viewer & Stress Monitor")
            .default_width(800)
            .default_height(600)
            .build();

        // Create a channel for communication between the data collection thread and the UI thread
        let (sender, receiver) = mpsc::channel::<SensorUpdate>();
        std::thread::spawn(move || {
            // Create a new async runtime so we can run async functions in a blocking context
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                start_data_collection(sender).await;
            });
        });

        let channels = Rc::new(RefCell::new(Channels::new()));

        let drawing_area = DrawingArea::new();
        window.set_child(Some(&drawing_area));

        let da = Rc::new(drawing_area);

        let channels_clone = channels.clone();
        da.set_draw_func(move |_, cr: &Context, width: i32, height: i32| {
            let backend = CairoBackend::new(cr, (width as u32, height as u32)).unwrap();
            let root = backend.into_drawing_area();
            root.fill(&WHITE).unwrap();

            // First, split the drawing area into two parts, with the first part taking up two-thirds of the area
            let (upper_two_thirds, lower_one_third) =
                root.split_vertically((height as u32 * 2) / 3);

            // Then, split the upper_two_thirds area into two to get three equally sized areas
            let (upper, middle) = upper_two_thirds.split_vertically(height as u32 / 3);

            // Draw the ECG signal
            {
                let ecg_series = &channels_clone.borrow().ecg;
                let last_points = ecg_series.last_points(400);
                let (min_time, max_time) = last_points.min_max_time().unwrap_or((0, 0));
                let (min_ecg, max_ecg) = last_points.min_max_value().unwrap_or((0, 0));
                let mut chart = ChartBuilder::on(&upper)
                    .margin(5)
                    .x_label_area_size(30)
                    .y_label_area_size(30)
                    .build_cartesian_2d(min_time..max_time, min_ecg..max_ecg)
                    .unwrap();
                chart.configure_mesh().draw().unwrap();
                chart
                    .draw_series(LineSeries::new(
                        last_points.iter().map(|point| (point.time, point.value)),
                        &RED,
                    ))
                    .unwrap();
            }

            // Draw the HR signal
            {
                let hr_series = &channels_clone.borrow().hr;
                let last_points = hr_series.last_points(400);
                let (min_time, max_time) = last_points.min_max_time().unwrap_or((0, 0));
                let mut chart = ChartBuilder::on(&middle)
                    .margin(5)
                    //.x_label_area_size(30)
                    .y_label_area_size(30)
                    .build_cartesian_2d(min_time..max_time, 20..120)
                    .unwrap();
                chart.configure_mesh().draw().unwrap();
                chart
                    .draw_series(LineSeries::new(
                        last_points.iter().map(|point| (point.time, point.value)),
                        &RED,
                    ))
                    .unwrap();
            }

            root.present().expect("Unable to draw");
        });

        // When idle, add data to be plotted from data collection thread
        glib::source::idle_add_local(move || {
            match channels.borrow_mut().handle_received_data(&receiver) {
                true => da.queue_draw(),
                false => {}
            }

            ControlFlow::Continue
        });

        // Show the window
        window.present();
    });

    application.run()
}

// Should the ECG offset be incorporated into arctic?

// Scan and select device
