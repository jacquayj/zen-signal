use crate::app::{Message, ZenSignal};
use crate::timeseries::{ChartWindow, PointSliceExt, TimeUnit};
use plotters::chart::ChartBuilder;
use plotters::series::LineSeries;
use plotters::style::{BLUE, CYAN, GREEN, MAGENTA, RED, RGBColor};
use plotters_iced::{Chart, DrawingBackend};

// Chart display constants
const CHART_TIME_WINDOW_SECONDS: f64 = 10.0;

// Y-axis ranges for different chart types
const ECG_MIN_UV: i32 = -2000;
const ECG_MAX_UV: i32 = 2000;

const HR_MIN_BPM: i32 = 40;
const HR_MAX_BPM: i32 = 180;

const RR_MIN_MS: i32 = 400;
const RR_MAX_MS: i32 = 1400;

const ACC_MIN_MG: i32 = -8000;
const ACC_MAX_MG: i32 = 8000;

// Chart types
pub struct EcgChartType<'a> {
    pub state: &'a ZenSignal,
}

pub struct HrChartType<'a> {
    pub state: &'a ZenSignal,
}

pub struct RrChartType<'a> {
    pub state: &'a ZenSignal,
}

pub struct AccChartType<'a> {
    pub state: &'a ZenSignal,
}

// ECG Chart
impl<'a> Chart<Message> for EcgChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        let ecg_series = &self.state.channels.ecg;
        // Show last 10 seconds of ECG data
        let window = ChartWindow::TenSeconds.as_nanos();
        let points = ecg_series.last_duration(window);
        
        let (min_time, _) = ecg_series.display_time_range(window);
        
        let mut chart = builder
            .margin(15)
            .caption("ECG Signal", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(0.0..CHART_TIME_WINDOW_SECONDS, ECG_MIN_UV..ECG_MAX_UV)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");
        
        chart.configure_mesh()
            .x_desc("Time (s)")
            .y_desc("ECG (μV)")
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| {
                    let time_sec = (p.time - min_time) as f64 / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &RED,
            ))
            .expect("Failed to draw series");
    }
}

// HR Chart
impl<'a> Chart<Message> for HrChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        let hr_series = &self.state.channels.hr;
        // Show last 10 seconds of HR data
        let window = ChartWindow::TenSeconds.as_nanos();
        let points = hr_series.last_duration(window);
        
        let (min_time, _) = hr_series.display_time_range(window);

        let mut chart = builder
            .margin(15)
            .caption("Heart Rate", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(0.0..CHART_TIME_WINDOW_SECONDS, HR_MIN_BPM..HR_MAX_BPM)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .x_desc("Time (s)")
            .y_desc("HR (bpm)")
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| {
                    let time_sec = (p.time - min_time) as f64 / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &RED,
            ))
            .expect("Failed to draw series");
    }
}

// RR Chart
impl<'a> Chart<Message> for RrChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        let rr_series = &self.state.channels.rr;
        // Show last 10 seconds of RR data
        let window = ChartWindow::TenSeconds.as_nanos();
        let points = rr_series.last_duration(window);
        let rmssd = points.rmssd();
        
        let (min_time, _) = rr_series.display_time_range(window);

        let mut chart = builder
            .margin(15)
            .caption(
                format!("RR Interval, RMSSD: {:.2} ms", rmssd),
                ("sans-serif", 20),
            )
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(0.0..CHART_TIME_WINDOW_SECONDS, RR_MIN_MS..RR_MAX_MS)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .x_desc("Time (s)")
            .y_desc("RR (ms)")
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| {
                    let time_sec = (p.time - min_time) as f64 / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &BLUE,
            ))
            .expect("Failed to draw series");
    }
}

// Acceleration Chart
impl<'a> Chart<Message> for AccChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        let acc_x_series = &self.state.channels.acc_x;
        // Show last 10 seconds of accelerometer data
        let window = ChartWindow::TenSeconds.as_nanos();
        let x_points = acc_x_series.last_duration(window);

        let acc_y_series = &self.state.channels.acc_y;
        let y_points = acc_y_series.last_duration(window);

        let acc_z_series = &self.state.channels.acc_z;
        let z_points = acc_z_series.last_duration(window);

        let (min_time, _) = acc_x_series.display_time_range(window);

        let mut chart = builder
            .margin(15)
            .caption("Acceleration", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(0.0..CHART_TIME_WINDOW_SECONDS, ACC_MIN_MG..ACC_MAX_MG)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .x_desc("Time (s)")
            .y_desc("Acc (mg)")
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                x_points.iter().map(|p| {
                    let time_sec = (p.time - min_time) as f64 / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &GREEN,
            ))
            .expect("Failed to draw X series");

        chart
            .draw_series(LineSeries::new(
                y_points.iter().map(|p| {
                    let time_sec = (p.time - min_time) as f64 / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &MAGENTA,
            ))
            .expect("Failed to draw Y series");

        chart
            .draw_series(LineSeries::new(
                z_points.iter().map(|p| {
                    let time_sec = (p.time - min_time) as f64 / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &CYAN,
            ))
            .expect("Failed to draw Z series");
    }
}
