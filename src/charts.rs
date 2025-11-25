use crate::app::{Message, ZenSignal};
use crate::timeseries::{ChartWindow, TimeUnit};
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

const HRV_MIN_MS: i32 = 0;
const HRV_MAX_MS: i32 = 150;

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

pub struct HrvChartType<'a> {
    pub state: &'a ZenSignal,
}

pub struct AccChartType<'a> {
    pub state: &'a ZenSignal,
}

// ECG Chart
impl<'a> Chart<Message> for EcgChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use crate::timeseries::TimeSeries;
        
        let ecg_series = &self.state.channels.ecg;
        // Show last 10 seconds of ECG data
        let window = ChartWindow::TenSeconds.as_nanos();
        let smooth_streaming = self.state.config.smooth_data_streaming;
        let display_time = TimeSeries::current_display_time(smooth_streaming);
        let points = ecg_series.range_from_time(display_time, window);
        
        let mut chart = builder
            .margin(15)
            .caption("ECG Signal", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(-CHART_TIME_WINDOW_SECONDS..0.0, ECG_MIN_UV..ECG_MAX_UV)
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
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
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
        use crate::timeseries::TimeSeries;
        
        let hr_series = &self.state.channels.hr;
        // Show last 10 seconds of HR data
        let window = ChartWindow::TenSeconds.as_nanos();
        let smooth_streaming = self.state.config.smooth_data_streaming;
        let display_time = TimeSeries::current_display_time(smooth_streaming);
        
        // Always use interpolation, but only interpolate at the end when smooth streaming is enabled
        let points = hr_series.range_from_time_interpolated(display_time, window, 100_000_000, smooth_streaming);

        let mut chart = builder
            .margin(15)
            .caption("Heart Rate", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(-CHART_TIME_WINDOW_SECONDS..0.0, HR_MIN_BPM..HR_MAX_BPM)
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
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
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
        use crate::timeseries::TimeSeries;
        
        let rr_series = &self.state.channels.rr;
        // Show last 10 seconds of RR data
        let window = ChartWindow::TenSeconds.as_nanos();
        let smooth_streaming = self.state.config.smooth_data_streaming;
        let display_time = TimeSeries::current_display_time(smooth_streaming);
        
        // Always use interpolation, but only interpolate at the end when smooth streaming is enabled
        let points = rr_series.range_from_time_interpolated(display_time, window, 100_000_000, smooth_streaming);

        let mut chart = builder
            .margin(15)
            .caption("RR Interval", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(-CHART_TIME_WINDOW_SECONDS..0.0, RR_MIN_MS..RR_MAX_MS)
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
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &BLUE,
            ))
            .expect("Failed to draw series");
    }
}

// HRV Chart
impl<'a> Chart<Message> for HrvChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use crate::timeseries::TimeSeries;
        
        let hrv_series = &self.state.channels.hrv;
        // Show last 10 seconds of HRV (RMSSD) data
        let window = ChartWindow::TenSeconds.as_nanos();
        let smooth_streaming = self.state.config.smooth_data_streaming;
        let display_time = TimeSeries::current_display_time(smooth_streaming);
        
        // Always use interpolation, but only interpolate at the end when smooth streaming is enabled
        let points = hrv_series.range_from_time_interpolated(display_time, window, 100_000_000, smooth_streaming);

        let mut chart = builder
            .margin(15)
            .caption("HRV (RMSSD)", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(-CHART_TIME_WINDOW_SECONDS..0.0, HRV_MIN_MS..HRV_MAX_MS)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .x_desc("Time (s)")
            .y_desc("RMSSD (ms)")
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| {
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &GREEN,
            ))
            .expect("Failed to draw series");
    }
}

// Acceleration Chart
impl<'a> Chart<Message> for AccChartType<'a> {
    type State = ();

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use crate::timeseries::TimeSeries;
        
        let acc_x_series = &self.state.channels.acc_x;
        // Show last 10 seconds of accelerometer data
        let window = ChartWindow::TenSeconds.as_nanos();
        let smooth_streaming = self.state.config.smooth_data_streaming;
        let display_time = TimeSeries::current_display_time(smooth_streaming);
        let x_points = acc_x_series.range_from_time(display_time, window);

        let acc_y_series = &self.state.channels.acc_y;
        let y_points = acc_y_series.range_from_time(display_time, window);

        let acc_z_series = &self.state.channels.acc_z;
        let z_points = acc_z_series.range_from_time(display_time, window);

        let mut chart = builder
            .margin(15)
            .caption("Acceleration", ("sans-serif", 20))
            .x_label_area_size(30)
            .y_label_area_size(40)
            .build_cartesian_2d(-CHART_TIME_WINDOW_SECONDS..0.0, ACC_MIN_MG..ACC_MAX_MG)
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
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &GREEN,
            ))
            .expect("Failed to draw X series");

        chart
            .draw_series(LineSeries::new(
                y_points.iter().map(|p| {
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &MAGENTA,
            ))
            .expect("Failed to draw Y series");

        chart
            .draw_series(LineSeries::new(
                z_points.iter().map(|p| {
                    let time_sec = (p.time as f64 - display_time as f64) / TimeUnit::Seconds.nanos_per_unit();
                    (time_sec, p.value)
                }),
                &CYAN,
            ))
            .expect("Failed to draw Z series");
    }
}
