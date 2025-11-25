use arctic::PmdData;

pub struct Channels {
    pub ecg: TimeSeries,
    pub acc_x: TimeSeries,
    pub acc_y: TimeSeries,
    pub acc_z: TimeSeries,
    pub hr: TimeSeries,
    pub rr: TimeSeries,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            ecg: TimeSeries::new(),
            acc_x: TimeSeries::new(),
            acc_y: TimeSeries::new(),
            acc_z: TimeSeries::new(),
            hr: TimeSeries::new(),
            rr: TimeSeries::new(),
        }
    }

    pub fn handle_heart_rate(&mut self, hr: arctic::HeartRate) {
        self.hr
            .add_point(self.hr.data.len().try_into().unwrap(), (*hr.bpm()).into());

        println!("Heart rate: {:?}", hr);

        let rr = hr.rr().clone().unwrap_or(vec![]);
        let rr_len = rr.len();

        // handle zero length
        if rr_len == 0 {
            if let Some(last) = self.rr.data.last() {
                self.rr.add_point(
                    self.rr.data.len().try_into().unwrap(),
                    last.value,
                );
            }
        } else {
            let rr_average =
                (rr.iter().sum::<u16>() as f64 / rr_len as f64).ceil() as i32;
            self.rr
                .add_point(self.rr.data.len().try_into().unwrap(), rr_average);
        }
    }

    pub fn handle_measurement_data(&mut self, data: arctic::PmdRead) {
        let timestamp = data.time_stamp();

        for (inx, d) in data.data().iter().enumerate() {
            match d {
                PmdData::Acc(acc) => {
                    // Magic number
                    // I just increased the timestep until the graph looked good
                    let acc_timestep = 1000000000 / 200;

                    let t = timestamp + (inx * acc_timestep) as u64;
                    let acc = acc.data();

                    self.acc_x.add_point(t, acc.0);
                    self.acc_y.add_point(t, acc.1);
                    self.acc_z.add_point(t, acc.2);
                }
                PmdData::Ecg(ecg) => {
                    // Magic number
                    let ecg_timestep = 1000000000 / 130;
                    self.ecg
                        .add_point(timestamp + (inx * ecg_timestep) as u64, *ecg.val());
                }
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
    fn rmssd(&self) -> f64;
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

    // RMSSD
    fn rmssd(&self) -> f64 {
        let mut sum = 0.0;
        let mut count = 0;
        for window in self.windows(2) {
            let diff = window[1].value as f64 - window[0].value as f64;
            sum += diff * diff;
            count += 1;
        }
        (sum / count as f64).sqrt()
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
