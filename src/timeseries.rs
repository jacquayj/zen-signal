use std::sync::mpsc;
use arctic::PmdData;
use crate::SensorUpdate;

pub struct Channels {
    pub ecg: TimeSeries,
    pub acc: TimeSeries,
    pub hr: TimeSeries,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            ecg: TimeSeries::new(),
            acc: TimeSeries::new(),
            hr: TimeSeries::new(),
        }
    }

    pub fn handle_received_data(&mut self, receiver: &mpsc::Receiver<SensorUpdate>) -> bool {
        match receiver.try_recv() {
            Ok(sensor_update) => match sensor_update {
                SensorUpdate::HeartRate(hr) => {
                    self.hr.add_point(self.hr.data.len().try_into().unwrap(), (*hr.bpm()).into()); 

                    true
                }
                SensorUpdate::MeasurementData(data) => {
                    let timestamp = data.time_stamp();

                    for (inx, d) in data.data().iter().enumerate() {
                        match d {
                            PmdData::Acc(_acc) => {}
                            PmdData::Ecg(ecg) => {
                                // Magic number
                                let timestep = 1000000000 / 130;

                                self.ecg.add_point(timestamp + (inx * timestep) as u64, *ecg.val());
                            }
                        }
                    }

                 true
                }
            },
            Err(_) => {
                false
            }
        }
    }
}

pub struct Point {
    pub time: u64,
    pub value: i32,
}

pub struct TimeSeries {
    data: Vec<Point>,
}

pub trait PointSliceExt {
    fn min_max_time(&self) -> Option<(u64, u64)>;
    fn min_max_value(&self) -> Option<(i32, i32)>;
}

// Implement the trait for a slice of `Point`
impl PointSliceExt for &[Point] {
    fn min_max_time(&self) -> Option<(u64, u64)> {
        self.iter().fold(None, |acc, point| match acc {
            None => Some((point.time, point.time)),
            Some((min, max)) => Some((min.min(point.time), max.max(point.time))),
        })
    }

    fn min_max_value(&self) -> Option<(i32, i32)> {
        self.iter().fold(None, |acc, point| match acc {
            None => Some((point.value, point.value)),
            Some((min, max)) => Some((min.min(point.value), max.max(point.value))),
        })
    }
}

impl TimeSeries {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    pub fn add_point(&mut self, time: u64, value: i32) {
        self.data.push(Point { time, value });
    }

    pub fn last_points(&self, n: usize) -> &[Point] {
        &self.data[self.data.len().saturating_sub(n)..]
    }

}
