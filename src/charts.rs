use crate::app::{Message, ZenSignal};
use crate::timeseries::PointSliceExt;
use plotters::chart::ChartBuilder;
use plotters::series::LineSeries;
use plotters::style::{BLUE, CYAN, GREEN, MAGENTA, RED, RGBColor};
use plotters_iced::{Chart, DrawingBackend};

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
        let points = ecg_series.last_points(400);
        
        let (min_time, max_time) = points.min_max_time().unwrap_or((0, 1));
        let (min_ecg, max_ecg) = points.min_max_value().unwrap_or((0, 1));

        let mut chart = builder
            .margin(15)
            .caption("ECG Signal", ("sans-serif", 20))
            .x_label_area_size(0)
            .y_label_area_size(0)
            .build_cartesian_2d(min_time..max_time, min_ecg..max_ecg)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");
        chart.configure_mesh().draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| (p.time, p.value)),
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
        let points = hr_series.last_points(400);
        
        let (min_time, max_time) = points.min_max_time().unwrap_or((0, 1));

        let mut chart = builder
            .margin(15)
            .caption("Heart Rate", ("sans-serif", 20))
            .x_label_area_size(0)
            .y_label_area_size(30)
            .build_cartesian_2d(min_time..max_time, 20..120)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| (p.time, p.value)),
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
        let points = rr_series.last_points(400);
        let rmssd = points.rmssd();
        
        let (min_time, max_time) = points.min_max_time().unwrap_or((0, 1));

        let mut chart = builder
            .margin(15)
            .caption(
                format!("RR Interval, RMSSD: {:.2}", rmssd),
                ("sans-serif", 20),
            )
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(min_time..max_time, 0..2000)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                points.iter().map(|p| (p.time, p.value)),
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
        let x_points = acc_x_series.last_points(400);

        let acc_y_series = &self.state.channels.acc_y;
        let y_points = acc_y_series.last_points(400);

        let acc_z_series = &self.state.channels.acc_z;
        let z_points = acc_z_series.last_points(400);

        let (min_time, max_time) = x_points.min_max_time().unwrap_or((0, 1));

        let (min_x_acc, max_x_acc) = x_points.min_max_value().unwrap_or((0, 1));
        let (min_y_acc, max_y_acc) = y_points.min_max_value().unwrap_or((0, 1));
        let (min_z_acc, max_z_acc) = z_points.min_max_value().unwrap_or((0, 1));

        let total_min = min_x_acc.min(min_y_acc).min(min_z_acc);
        let total_max = max_x_acc.max(max_y_acc).max(max_z_acc);

        let mut chart = builder
            .margin(15)
            .caption("Acceleration", ("sans-serif", 20))
            .x_label_area_size(0)
            .y_label_area_size(30)
            .build_cartesian_2d(min_time..max_time, total_min..total_max)
            .expect("Failed to build chart");

        chart.plotting_area().fill(&RGBColor(245, 245, 240)).expect("Failed to fill background");

        chart.configure_mesh()
            .axis_style(RGBColor(60, 60, 60))
            .draw().expect("Failed to draw mesh");

        chart
            .draw_series(LineSeries::new(
                x_points.iter().map(|p| (p.time, p.value)),
                &GREEN,
            ))
            .expect("Failed to draw X series");

        chart
            .draw_series(LineSeries::new(
                y_points.iter().map(|p| (p.time, p.value)),
                &MAGENTA,
            ))
            .expect("Failed to draw Y series");

        chart
            .draw_series(LineSeries::new(
                z_points.iter().map(|p| (p.time, p.value)),
                &CYAN,
            ))
            .expect("Failed to draw Z series");
    }
}
