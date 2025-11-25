use arctic::PmdData;

// Sample rates configured for Polar H10 device
// Note: These should match the rates configured via polar.ecg_sample_rate() and polar.acc_sample_rate()
// The actual rate is queried from the device and set to maximum in sensor.rs
const ECG_SAMPLE_RATE_HZ: u64 = 130; // Default ECG sampling rate in Hz (can be configured)
const ACC_SAMPLE_RATE_HZ: u64 = 200; // Default accelerometer sampling rate in Hz

// Nanoseconds in one second
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Time window duration for chart display
#[derive(Debug, Clone, Copy)]
pub enum ChartWindow {
    /// 10 seconds
    TenSeconds,
    /// 30 seconds
    ThirtySeconds,
    /// 60 seconds
    OneMinute,
    /// 120 seconds
    TwoMinutes,
}

impl ChartWindow {
    /// Get the duration in nanoseconds
    pub fn as_nanos(&self) -> u64 {
        match self {
            ChartWindow::TenSeconds => 10_000_000_000,
            ChartWindow::ThirtySeconds => 30_000_000_000,
            ChartWindow::OneMinute => 60_000_000_000,
            ChartWindow::TwoMinutes => 120_000_000_000,
        }
    }
}

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
        // Use current system time as approximate timestamp for HR
        // HR data doesn't come with precise timestamps from the device
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        self.hr.add_point(now, (*hr.bpm()).into());

        println!("Heart rate: {:?}", hr);

        let rr = hr.rr().clone().unwrap_or(vec![]);
        let rr_len = rr.len();

        // Handle RR intervals - each interval is a separate data point
        // RR intervals are the time between beats in milliseconds
        if rr_len == 0 {
            // If no RR data, repeat last value to maintain continuity
            if let Some(last) = self.rr.data.last() {
                self.rr.add_point(now, last.value);
            }
        } else {
            // Add each RR interval as a separate point
            // Space them out evenly within the time since last measurement
            let time_spacing = if let Some(last) = self.rr.data.last() {
                (now - last.time) / rr_len as u64
            } else {
                1_000_000_000 // 1 second default spacing
            };

            for (i, &rr_value) in rr.iter().enumerate() {
                let t = now - ((rr_len - i - 1) as u64 * time_spacing);
                self.rr.add_point(t, rr_value as i32);
            }
        }
    }

    pub fn handle_measurement_data(&mut self, data: arctic::PmdRead) {
        let timestamp = data.time_stamp();
        let data_vec = data.data();
        let sample_count = data_vec.len();

        for (inx, d) in data_vec.iter().enumerate() {
            match d {
                PmdData::Acc(acc) => {
                    // Calculate time delta between samples based on configured sample rate
                    let acc_timestep = NANOS_PER_SECOND / self.acc_x.sample_rate();
                    
                    // Calculate timestamp for this sample (working backwards from the last timestamp)
                    let samples_from_end = (sample_count - 1 - inx) as u64;
                    let t = timestamp.saturating_sub(samples_from_end * acc_timestep);
                    
                    // Validate on first sample: check if calculated first timestamp matches expectations
                    if inx == 0 && self.acc_x.data.len() > 0 {
                        if let Some(last) = self.acc_x.data.last() {
                            let expected_gap = acc_timestep;
                            let actual_gap = t.saturating_sub(last.time);
                            let tolerance = acc_timestep / 2;
                            
                            if actual_gap > expected_gap + tolerance || actual_gap < expected_gap.saturating_sub(tolerance) {
                                eprintln!("Warning: ACC timestamp mismatch. Expected gap: {}ns, Actual gap: {}ns", 
                                    expected_gap, actual_gap);
                            }
                        }
                    }

                    let acc = acc.data();
                    self.acc_x.add_point(t, acc.0);
                    self.acc_y.add_point(t, acc.1);
                    self.acc_z.add_point(t, acc.2);
                }
                PmdData::Ecg(ecg) => {
                    // Calculate time delta between samples based on configured sample rate
                    let ecg_timestep = NANOS_PER_SECOND / self.ecg.sample_rate();
                    
                    // Calculate timestamp for this sample (working backwards from the last timestamp)
                    let samples_from_end = (sample_count - 1 - inx) as u64;
                    let t = timestamp.saturating_sub(samples_from_end * ecg_timestep);
                    
                    // Validate on first sample: check if calculated first timestamp matches expectations
                    if inx == 0 && self.ecg.data.len() > 0 {
                        if let Some(last) = self.ecg.data.last() {
                            let expected_gap = ecg_timestep;
                            let actual_gap = t.saturating_sub(last.time);
                            let tolerance = ecg_timestep / 2;
                            
                            if actual_gap > expected_gap + tolerance || actual_gap < expected_gap.saturating_sub(tolerance) {
                                eprintln!("Warning: ECG timestamp mismatch. Expected gap: {}ns, Actual gap: {}ns", 
                                    expected_gap, actual_gap);
                            }
                        }
                    }
                    
                    self.ecg.add_point(t, *ecg.val());
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
    sample_rate: u64, // Sample rate in Hz (nominal, for calculating expected intervals)
    start_time: Option<u64>, // First timestamp in nanoseconds
}

impl TimeSeries {
    /// Calculate expected time interval between samples in nanoseconds
    fn expected_interval_ns(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        NANOS_PER_SECOND / self.sample_rate
    }

    /// Check if there's a gap between the last point and a new timestamp
    /// Returns true if the gap is larger than 1.5x the expected interval
    pub fn has_gap(&self, new_timestamp: u64) -> bool {
        if let Some(last) = self.data.last() {
            let expected = self.expected_interval_ns();
            if expected > 0 {
                let actual_gap = new_timestamp.saturating_sub(last.time);
                return actual_gap > expected + (expected / 2);
            }
        }
        false
    }

    /// Get the number of samples that appear to be missing before this timestamp
    pub fn missing_samples(&self, new_timestamp: u64) -> usize {
        if let Some(last) = self.data.last() {
            let expected = self.expected_interval_ns();
            if expected > 0 {
                let actual_gap = new_timestamp.saturating_sub(last.time);
                let expected_samples = (actual_gap / expected) as usize;
                return expected_samples.saturating_sub(1);
            }
        }
        0
    }
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
            start_time: None,
        }
    }

    pub fn set_sample_rate(&mut self, rate: u64) {
        self.sample_rate = rate;
    }

    pub fn sample_rate(&self) -> u64 {
        self.sample_rate
    }

    pub fn add_point(&mut self, time: u64, value: i32) {
        // Set start time on first point
        if self.start_time.is_none() {
            self.start_time = Some(time);
        }

        // Fill gaps with zero values
        if self.has_gap(time) {
            let missing = self.missing_samples(time);
            if missing > 0 {
                eprintln!("Warning: Detected gap in data. Filling {} missing samples with zeros", missing);
                
                // Fill the gap with zero-value points
                let expected_interval = self.expected_interval_ns();
                let last_time = self.data.last().map(|p| p.time).unwrap_or(time);
                
                for i in 1..=missing {
                    let fill_time = last_time + (i as u64 * expected_interval);
                    self.data.push(Point { time: fill_time, value: 0 });
                }
            }
        }

        self.data.push(Point { time, value });
    }

    pub fn last_points(&self, n: usize) -> &[Point] {
        &self.data[self.data.len().saturating_sub(n)..]
    }

    /// Get points from the last `duration_ns` nanoseconds
    /// Returns all points whose timestamp is >= (latest_timestamp - duration_ns)
    pub fn last_duration(&self, duration_ns: u64) -> &[Point] {
        if self.data.is_empty() {
            return &[];
        }

        let latest_time = self.data.last().unwrap().time;
        let cutoff_time = latest_time.saturating_sub(duration_ns);

        // Binary search for the first point >= cutoff_time
        let start_idx = self.data.partition_point(|p| p.time < cutoff_time);
        
        &self.data[start_idx..]
    }

    /// Get the time range that should be displayed for a given duration window
    /// This ensures the time axis is fixed and data appears from right to left
    pub fn display_time_range(&self, duration_ns: u64) -> (u64, u64) {
        if self.data.is_empty() {
            return (0, duration_ns);
        }

        let latest_time = self.data.last().unwrap().time;
        let start_time = latest_time.saturating_sub(duration_ns);
        (start_time, latest_time)
    }
}
