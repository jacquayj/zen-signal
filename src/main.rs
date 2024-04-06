use std::sync::mpsc::{self, Sender};

use gtk::cairo::Context;
use gtk::glib::ControlFlow;
use gtk4 as gtk;

use arctic::{self, HeartRate};
use gtk::{glib, Application, ApplicationWindow};
use gtk::{prelude::*, DrawingArea};
use plotters::chart::ChartBuilder;
use plotters::drawing::IntoDrawingArea;
use plotters::series::LineSeries; // Import the LineSeries module
use plotters::style::{RED, WHITE};
use plotters_cairo::CairoBackend;
use tokio::runtime::Runtime;
use std::rc::Rc;
use std::cell::RefCell;


struct Handler {
    sender: Sender<HeartRate>,
}

impl Handler {
    fn new(sender: Sender<HeartRate>) -> Self {
        Self{
            sender,
        }
    }
}

#[arctic::async_trait]
impl arctic::EventHandler for Handler {
    async fn heart_rate_update(&self, _ctx: &arctic::PolarSensor, heartrate: arctic::HeartRate) {
        if let Err(why) = self.sender.send(heartrate) {
            println!("Could not send heart rate data: {:?}", why);
        }
    }
}

async fn collect_data(sender: Sender<HeartRate>) {
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

        // Create a channel for communication between async runtime and GTK main loop
        let (sender, receiver) = mpsc::channel::<HeartRate>();
        std::thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            rt.block_on(async move {
                collect_data(sender).await;
            });
        });


        let heart_rate_data: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));

        let drawing_area = DrawingArea::new();
        window.set_child(Some(&drawing_area));

        let da = Rc::new(drawing_area);

        let heart_rate_data_clone = heart_rate_data.clone();
        da.set_draw_func(move |_, cr: &Context, width: i32, height: i32| {
            let backend = CairoBackend::new(cr, (width as u32, height as u32)).unwrap();
            let root = backend.into_drawing_area();

            root.fill(&WHITE).unwrap();

            let mut chart = ChartBuilder::on(&root)
                .margin(5)
                .x_label_area_size(30)
                .y_label_area_size(30)
                .build_cartesian_2d(0..100, 20..220)
                .unwrap();

            chart.configure_mesh().draw().unwrap();

            chart
                .draw_series(LineSeries::new(
                    heart_rate_data_clone.borrow().iter().enumerate().map(|(x, y)| (x as i32, *y as i32)),
                    &RED,
                ))
                .unwrap();

            root.present().expect("Unable to draw");
        });


        // When idle, check if there is any data to plot
        glib::source::idle_add_local( move || {
            if let Ok(plot_data) = receiver.try_recv() {
                println!("{:?}", plot_data);

                heart_rate_data.borrow_mut().push(*plot_data.bpm());

                da.queue_draw();
            }

            ControlFlow::Continue
        });

        // Show the window
        window.present();
    });

    application.run()
}

