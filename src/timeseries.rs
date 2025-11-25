use arctic::PmdData;

// Sample rates configured for Polar H10 device
// Note: These should match the rates configured via polar.ecg_sample_rate() and polar.acc_sample_rate()
// The actual rate is queried from the device and set to maximum in sensor.rs
const ECG_SAMPLE_RATE_HZ: u64 = 130; // Default ECG sampling rate in Hz (can be configured)
const ACC_SAMPLE_RATE_HZ: u64 = 200; // Default accelerometer sampling rate in Hz

// Nanoseconds in one second
const NANOS_PER_SECOND: u64 = 1_000_000_000;

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
            ecg: TimeSeries::new(ECG_SAMPLE_RATE_HZ),
            acc_x: TimeSeries::new(ACC_SAMPLE_RATE_HZ),
            acc_y: TimeSeries::new(ACC_SAMPLE_RATE_HZ),
            acc_z: TimeSeries::new(ACC_SAMPLE_RATE_HZ),
            hr: TimeSeries::new(1), // HR doesn't use sample rate for time calculations
            rr: TimeSeries::new(1), // RR doesn't use sample rate for time calculations
        }
    }

    pub fn set_ecg_sample_rate(&mut self, rate: u64) {
        self.ecg.set_sample_rate(rate);
    }

    pub fn set_acc_sample_rate(&mut self, rate: u64) {
        self.acc_x.set_sample_rate(rate);
        self.acc_y.set_sample_rate(rate);
        self.acc_z.set_sample_rate(rate);
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
                    // Calculate time delta between samples based on configured sample rate
                    let acc_timestep = NANOS_PER_SECOND / self.acc_x.sample_rate();

                    let t = timestamp + (inx as u64 * acc_timestep);
                    let acc = acc.data();

                    self.acc_x.add_point(t, acc.0);
                    self.acc_y.add_point(t, acc.1);
                    self.acc_z.add_point(t, acc.2);
                }
                PmdData::Ecg(ecg) => {
                    // Calculate time delta between samples based on configured sample rate
                    let ecg_timestep = NANOS_PER_SECOND / self.ecg.sample_rate();
                    self.ecg
                        .add_point(timestamp + (inx as u64 * ecg_timestep), *ecg.val());
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
    sample_rate: u64, // Sample rate in Hz
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
    pub fn new(sample_rate: u64) -> Self {
        Self { 
            data: Vec::new(),
            sample_rate,
        }
    }

    pub fn set_sample_rate(&mut self, rate: u64) {
        self.sample_rate = rate;
    }

    pub fn sample_rate(&self) -> u64 {
        self.sample_rate
    }

    pub fn add_point(&mut self, time: u64, value: i32) {
        self.data.push(Point { time, value });
    }

    pub fn last_points(&self, n: usize) -> &[Point] {
        &self.data[self.data.len().saturating_sub(n)..]
    }
}
