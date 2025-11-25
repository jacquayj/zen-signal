use arctic::PmdData;

// Sample rates configured for Polar H10 device
// Note: These should match the rates configured via polar.ecg_sample_rate() and polar.acc_sample_rate()
// The actual rate is queried from the device and set to maximum in sensor.rs
const ECG_SAMPLE_RATE_HZ: u64 = 130; // Default ECG sampling rate in Hz (can be configured)
const ACC_SAMPLE_RATE_HZ: u64 = 200; // Default accelerometer sampling rate in Hz

// Nanoseconds in one second
const NANOS_PER_SECOND: u64 = 1_000_000_000;

// Display delay for smooth scrolling (1.5 seconds in nanoseconds)
// This prevents gaps when low-rate data (HR, RR, HRV at ~1Hz) hasn't arrived yet
const DISPLAY_DELAY_NS: u64 = 1_500_000_000;

/// Time conversion constants
#[derive(Debug, Clone, Copy)]
pub enum TimeUnit {
    Nanoseconds,
    Seconds,
}

impl TimeUnit {
    /// Get the conversion factor from nanoseconds
    pub const fn nanos_per_unit(&self) -> f64 {
        match self {
            TimeUnit::Nanoseconds => 1.0,
            TimeUnit::Seconds => 1_000_000_000.0,
        }
    }
}

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
    pub hrv: TimeSeries, // RMSSD over time
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
            hrv: TimeSeries::new(1), // HRV (RMSSD) calculated periodically
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
            
            // Calculate and store HRV (RMSSD) from recent RR intervals
            // Use last 30 seconds of data for rolling RMSSD calculation
            const THIRTY_SECONDS_NS: u64 = 30_000_000_000;
            let recent_rr = self.rr.last_duration(THIRTY_SECONDS_NS);
            
            if recent_rr.len() >= 2 {
                let rmssd = recent_rr.rmssd();
                // Store RMSSD value as integer (rounded)
                self.hrv.add_point(now, rmssd as i32);
            }
        }
    }

    pub fn handle_measurement_data(&mut self, data: arctic::PmdRead) {
        // Use system time as the reference point for this batch
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        let data_vec = data.data();
        
        // Count samples by type to properly timestamp each channel
        let mut ecg_count = 0u64;
        let mut acc_count = 0u64;
        for d in data_vec.iter() {
            match d {
                PmdData::Ecg(_) => ecg_count += 1,
                PmdData::Acc(_) => acc_count += 1,
            }
        }
        
        // Determine the starting timestamp for each data type
        // Use the last timestamp + 1 sample interval, or fall back to calculating from 'now'
        let ecg_timestep = NANOS_PER_SECOND / self.ecg.sample_rate();
        let acc_timestep = NANOS_PER_SECOND / self.acc_x.sample_rate();
        
        let ecg_start_time = if let Some(last_point) = self.ecg.data.last() {
            // Continue from last timestamp + one interval
            last_point.time + ecg_timestep
        } else if ecg_count > 0 {
            // First batch: spread backwards from now
            now.saturating_sub((ecg_count - 1) * ecg_timestep)
        } else {
            // No ECG samples in this batch
            now
        };
        
        let acc_start_time = if let Some(last_point) = self.acc_x.data.last() {
            // Continue from last timestamp + one interval
            last_point.time + acc_timestep
        } else if acc_count > 0 {
            // First batch: spread backwards from now
            now.saturating_sub((acc_count - 1) * acc_timestep)
        } else {
            // No ACC samples in this batch
            now
        };
        
        // Track indices per data type
        let mut ecg_idx = 0u64;
        let mut acc_idx = 0u64;

        for d in data_vec.iter() {
            match d {
                PmdData::Acc(acc) => {
                    // Calculate timestamp as start_time + (index * timestep)
                    let t = acc_start_time + (acc_idx * acc_timestep);

                    let acc = acc.data();
                    self.acc_x.add_point(t, acc.0);
                    self.acc_y.add_point(t, acc.1);
                    self.acc_z.add_point(t, acc.2);
                    
                    acc_idx += 1;
                }
                PmdData::Ecg(ecg) => {
                    // Calculate timestamp as start_time + (index * timestep)
                    let t = ecg_start_time + (ecg_idx * ecg_timestep);
                    
                    self.ecg.add_point(t, *ecg.val());
                    
                    ecg_idx += 1;
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

    /// Get the current display reference time with optional smooth scrolling delay
    /// When smooth_streaming is true, returns current time minus a fixed delay to enable smooth scrolling
    /// and prevent gaps in low-rate data streams.
    /// When smooth_streaming is false, returns current time for immediate rendering.
    pub fn current_display_time(smooth_streaming: bool) -> u64 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        if smooth_streaming {
            now.saturating_sub(DISPLAY_DELAY_NS)
        } else {
            now
        }
    }

    /// Get points within a specific time range [end_time - duration_ns, end_time]
    /// This is used for rendering with a fixed reference time for smooth scrolling
    pub fn range_from_time(&self, end_time: u64, duration_ns: u64) -> &[Point] {
        if self.data.is_empty() {
            return &[];
        }

        let start_time = end_time.saturating_sub(duration_ns);
        
        // Find first point >= start_time
        let start_idx = self.data.partition_point(|p| p.time < start_time);
        
        // Find first point > end_time
        let end_idx = self.data.partition_point(|p| p.time <= end_time);
        
        &self.data[start_idx..end_idx]
    }

    /// Get points for rendering with forward-fill to handle gaps in low-rate data
    /// If there's a gap at the start of the window, adds a synthetic point with the last known value
    pub fn range_from_time_with_fill(&self, end_time: u64, duration_ns: u64) -> Vec<Point> {
        if self.data.is_empty() {
            return Vec::new();
        }

        let start_time = end_time.saturating_sub(duration_ns);
        
        // Find first point >= start_time
        let start_idx = self.data.partition_point(|p| p.time < start_time);
        
        // Find first point > end_time
        let end_idx = self.data.partition_point(|p| p.time <= end_time);
        
        let points_in_range = &self.data[start_idx..end_idx];
        
        // If we have points in range but the first one is after start_time,
        // look for the last point before the window to forward-fill
        if !points_in_range.is_empty() && points_in_range[0].time > start_time {
            // Find the last point before start_time
            if start_idx > 0 {
                let last_before = &self.data[start_idx - 1];
                // Create a synthetic point at the start of the window with the last known value
                let mut result = Vec::with_capacity(points_in_range.len() + 1);
                result.push(Point {
                    time: start_time,
                    value: last_before.value,
                });
                result.extend(points_in_range.iter().map(|p| Point {
                    time: p.time,
                    value: p.value,
                }));
                return result;
            }
        }
        
        // No fill needed, just return the points
        points_in_range.iter().map(|p| Point {
            time: p.time,
            value: p.value,
        }).collect()
    }

    /// Get points with linear interpolation for smooth curves
    /// Adds interpolated points between actual data points to create smoother lines
    /// target_interval_ns: desired time between interpolated points (e.g., 100ms = 100_000_000ns)
    /// interpolate_end: if true, interpolates at end_time; if false, only interpolates at start_time
    pub fn range_from_time_interpolated(&self, end_time: u64, duration_ns: u64, target_interval_ns: u64, interpolate_end: bool) -> Vec<Point> {
        if self.data.is_empty() {
            return Vec::new();
        }

        let start_time = end_time.saturating_sub(duration_ns);
        
        // Find first point >= start_time (or the point just before for interpolation)
        let start_idx = self.data.partition_point(|p| p.time < start_time);
        
        // Find first point > end_time (or include one after for forward interpolation)
        let end_idx = self.data.partition_point(|p| p.time <= end_time);
        
        // Get points including one before the window and one after for interpolation
        let actual_start_idx = if start_idx > 0 { start_idx - 1 } else { start_idx };
        let actual_end_idx = (end_idx + 1).min(self.data.len());
        let points_for_interp = &self.data[actual_start_idx..actual_end_idx];
        
        if points_for_interp.len() < 2 {
            // Not enough points to interpolate, just return what we have
            return self.range_from_time_with_fill(end_time, duration_ns);
        }
        
        let mut result = Vec::new();
        
        // ALWAYS add an exact point at start_time by interpolating
        let first_in_window = points_for_interp.iter().position(|p| p.time >= start_time).unwrap_or(0);
        if first_in_window > 0 {
            let p1 = &points_for_interp[first_in_window - 1];
            let p2 = &points_for_interp[first_in_window];
            
            if p1.time <= start_time && p2.time >= start_time {
                let time_diff = p2.time - p1.time;
                let value_diff = p2.value - p1.value;
                let time_from_p1 = start_time - p1.time;
                let progress = time_from_p1 as f64 / time_diff as f64;
                let interpolated_value = p1.value as f64 + (value_diff as f64 * progress);
                
                result.push(Point {
                    time: start_time,
                    value: interpolated_value.round() as i32,
                });
            }
        }
        
        // Interpolate between each pair of consecutive points
        for window in points_for_interp.windows(2) {
            let p1 = &window[0];
            let p2 = &window[1];
            
            let time_diff = p2.time.saturating_sub(p1.time);
            let value_diff = p2.value - p1.value;
            
            // Calculate number of interpolated points needed
            let num_steps = (time_diff / target_interval_ns).max(1);
            
            // Add interpolated points
            for step in 0..num_steps {
                let t = p1.time + (time_diff * step / num_steps);
                
                // Only add points within the display window (but not the exact boundaries - we handle those separately)
                if t > start_time && t < end_time {
                    let progress = step as f64 / num_steps as f64;
                    let interpolated_value = p1.value as f64 + (value_diff as f64 * progress);
                    
                    result.push(Point {
                        time: t,
                        value: interpolated_value.round() as i32,
                    });
                }
            }
        }
        
        // Add actual points that fall within the window (excluding boundaries)
        for point in points_for_interp {
            if point.time > start_time && point.time < end_time {
                result.push(Point {
                    time: point.time,
                    value: point.value,
                });
            }
        }
        
        // Conditionally add an exact point at end_time by forward-interpolating
        if interpolate_end {
            let last_before_end = points_for_interp.iter().rposition(|p| p.time <= end_time).unwrap_or(points_for_interp.len() - 1);
            if last_before_end + 1 < points_for_interp.len() {
                let p1 = &points_for_interp[last_before_end];
                let p2 = &points_for_interp[last_before_end + 1];
                
                if p1.time <= end_time && p2.time >= end_time {
                    let time_diff = p2.time - p1.time;
                    let value_diff = p2.value - p1.value;
                    let time_from_p1 = end_time - p1.time;
                    let progress = time_from_p1 as f64 / time_diff as f64;
                    let interpolated_value = p1.value as f64 + (value_diff as f64 * progress);
                    
                    result.push(Point {
                        time: end_time,
                        value: interpolated_value.round() as i32,
                    });
                }
            } else if points_for_interp.len() >= 2 {
                // Forward extrapolate from last two points
                let p1 = &points_for_interp[points_for_interp.len() - 2];
                let p2 = &points_for_interp[points_for_interp.len() - 1];
                
                if p2.time < end_time {
                    let time_diff = p2.time - p1.time;
                    let value_diff = p2.value - p1.value;
                    let time_from_p2 = end_time - p2.time;
                    
                    // Only extrapolate if gap is reasonable
                    if time_diff > 0 && time_from_p2 <= time_diff * 3 {
                        let progress = time_from_p2 as f64 / time_diff as f64;
                        let interpolated_value = p2.value as f64 + (value_diff as f64 * progress);
                        
                        result.push(Point {
                            time: end_time,
                            value: interpolated_value.round() as i32,
                        });
                    }
                }
            }
        }
        
        // Sort by time since we added points out of order
        result.sort_by_key(|p| p.time);
        
        result
    }
}
