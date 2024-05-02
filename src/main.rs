mod timeseries;

use arctic::{self, PolarSensor};
use gtk::cairo::Context;
use gtk::glib::ControlFlow;
use gtk::{glib, Application, ApplicationWindow};
use gtk::{prelude::*, DrawingArea};
use gtk4 as gtk;
use plotters::chart::ChartBuilder;
use plotters::drawing::IntoDrawingArea;
use plotters::series::LineSeries;
use plotters::style::{text_anchor, Color, RGBAColor, TextStyle, BLUE, CYAN, GREEN, MAGENTA, RED, WHITE};
use plotters_cairo::CairoBackend;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, RwLock};
use tokio::runtime::Runtime;
use timeseries::{Channels, PointSliceExt};


// enable clone and debug for the SensorUpdate enum
#[derive(Debug)]
enum SensorUpdate {
    HeartRate(arctic::HeartRate),
    MeasurementData(arctic::PmdRead),
}

#[derive(Clone)]
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

async fn start_data_collection(polar: Arc<RwLock<PolarSensor>>, handler: Handler) {
    println!("Attempting connection");

    {
        let mut polar = polar.write().unwrap();

        print!("Connecting");
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

        polar.event_handler(handler);
    }

    let result = polar.read().unwrap().event_loop().await;

    println!("No more data: {:?}", result);
}

fn main() -> glib::ExitCode {
    let application = Application::builder()
        .application_id("com.example.ZenSignal")
        .build();

    application.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("ZenSignal: Polar H10 Signal Viewer & Stress Monitor")
            .default_width(1200)
            .default_height(800)
            .build();

        // Create a channel for communication between the data collection thread and the UI thread
        let (sender, receiver) = mpsc::channel::<SensorUpdate>();
        std::thread::spawn(move || {
            // Create a new async runtime so we can run async functions in a blocking context
            let rt: Runtime = Runtime::new().unwrap();
            rt.block_on(async move {
                let polar = Arc::new(RwLock::new(
                    arctic::PolarSensor::new("56853526".to_string())
                        .await
                        .expect("Invalid ID"),
                ));

                let handler = Handler::new(sender.clone());

                let data_collection_future = start_data_collection(polar, handler);

                tokio::join!(data_collection_future);
            });
        });

        // We use Rc to share signal Channels between the draw closure and the data collection closure
        // RefCell is used to allow for interior mutability
        let channels = Rc::new(RefCell::new(Channels::new()));

        let drawing_area = DrawingArea::new();
        window.set_child(Some(&drawing_area));

        let da = Rc::new(drawing_area);

        let channels_clone = channels.clone();
        da.set_draw_func(move |_, cr: &Context, width: i32, height: i32| {
            let backend = CairoBackend::new(cr, (width as u32, height as u32)).unwrap();
            let root = backend.into_drawing_area();
            root.fill(&WHITE).unwrap();

            // Split drawing area into two columns, with the first column taking up 3-fourths of the width
            let (plot_area, stat_area) = root.split_horizontally(((width / 4) * 3) as u32);

            {
                let hr_series = &channels_clone.borrow().hr;
                let last_points = hr_series.last_points(1);
                let hr = last_points.last().map(|point| point.value).unwrap_or(0);
                let text = format!("Heart Rate: {}", hr);
                let _ = stat_area.draw_text(
                    &text,
                    &TextStyle::from(("sans-serif", 20)).color(&RED),
                    (10, 10)
                );
            }

            // Split the plot area into 4 rows for plots
            let (plot_top, plot_bottom) = plot_area.split_vertically((height / 2) as u32);
            let (plot_1, plot_2) = plot_top.split_vertically((height / 4) as u32);
            let (plot_3, plot_4) = plot_bottom.split_vertically((height / 4) as u32);

            // Draw the ECG signal
            {
                let ecg_series = &channels_clone.borrow().ecg;
                let last_points = ecg_series.last_points(400);
                let (min_time, max_time) = last_points.min_max_time().unwrap_or((0, 0));
                let (min_ecg, max_ecg) = last_points.min_max_value().unwrap_or((0, 0));

                let mut chart = ChartBuilder::on(&plot_1)
                    .margin(15)
                    // no x or y labels
                    .x_label_area_size(0)
                    .y_label_area_size(0)
                    .caption("ECG Signal", ("sans-serif", 14))
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
                let mut chart = ChartBuilder::on(&plot_2)
                    .margin(15)
                    //.x_label_area_size(30)
                    .y_label_area_size(30)
                    .caption("Heart Rate", ("sans-serif", 14))
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

            // Draw rr interval
            {
                let rr_series = &channels_clone.borrow().rr;
                let last_points = rr_series.last_points(400);

                let rmssd: f64 = last_points.rmssd();

                let (min_time, max_time) = last_points.min_max_time().unwrap_or((0, 0));
                let mut chart = ChartBuilder::on(&plot_3)
                    .margin(15)
                    .x_label_area_size(30)
                    .y_label_area_size(30)
                    .caption(format!("RR Interval, RMSSD: {}", rmssd), ("sans-serif", 14))
                    .build_cartesian_2d(min_time..max_time, 0..2000)
                    .unwrap();
                chart.configure_mesh().draw().unwrap();
                chart
                    .draw_series(LineSeries::new(
                        last_points.iter().map(|point| (point.time, point.value)),
                        &BLUE,
                    ))
                    .unwrap();
            }

            // Draw x, y, z accel
            {
                let acc_x_series = &channels_clone.borrow().acc_x;
                let last_x_points = acc_x_series.last_points(400);

                let acc_y_series = &channels_clone.borrow().acc_y;
                let last_y_points = acc_y_series.last_points(400);

                let acc_z_series = &channels_clone.borrow().acc_z;
                let last_z_points = acc_z_series.last_points(400);

                let (min_time, max_time) = last_x_points.min_max_time().unwrap_or((0, 0));

                let (min_x_acc, max_x_acc) = last_x_points.min_max_value().unwrap_or((0, 0));
                let (min_y_acc, max_y_acc) = last_y_points.min_max_value().unwrap_or((0, 0));
                let (min_z_acc, max_z_acc) = last_z_points.min_max_value().unwrap_or((0, 0));

                let total_min = min_x_acc.min(min_y_acc).min(min_z_acc);
                let total_max = max_x_acc.max(max_y_acc).max(max_z_acc);

                let mut chart = ChartBuilder::on(&plot_4)
                    .margin(15)
                    .x_label_area_size(0)
                    .y_label_area_size(30)
                    .caption("Acceleration", ("sans-serif", 14))
                    .build_cartesian_2d(min_time..max_time, total_min..total_max)
                    .unwrap();
                chart.configure_mesh().draw().unwrap();

                chart
                    .draw_series(LineSeries::new(
                        last_x_points.iter().map(|point| (point.time, point.value)),
                        &GREEN,
                    ))
                    .unwrap();

                chart
                    .draw_series(LineSeries::new(
                        last_y_points.iter().map(|point| (point.time, point.value)),
                        &MAGENTA,
                    ))
                    .unwrap();

                chart
                    .draw_series(LineSeries::new(
                        last_z_points.iter().map(|point| (point.time, point.value)),
                        &CYAN,
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

// timestamps for HR, RR interval, and ability to slice last x seconds of data
// Stats on right side of screen
// write buffer to file
// Scan and select device
// Reconnect logic

// Should the ECG offset be incorporated into arctic?
