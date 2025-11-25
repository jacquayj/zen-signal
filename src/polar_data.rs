//! # Polar Sensor Data Management Module
//!
//! Handles Polar H10 biosensor-specific data streams and protocol logic.
//! Converts Arctic library's data types into our time series storage format
//! with proper timestamp calculation and channel orchestration.
//!
//! ## Key Types
//! - `Channels`: Container managing all Polar H10 data streams (ECG, ACC, HR, RR, HRV)
//!
//! ## Responsibilities
//! 1. Parse Arctic's `PmdData`, `PmdRead`, and `HeartRate` types
//! 2. Calculate evenly-spaced timestamps for batched samples
//! 3. Distribute data to appropriate time series channels
//! 4. Compute derived metrics (HRV from RR intervals)
//!
//! ## Why Separate from TimeSeries
//! This keeps sensor-specific protocol logic separate from generic time series
//! storage, making TimeSeries reusable for other data sources.

use arctic::PmdData;
use crate::timeseries::{TimeSeries, PointSliceExt};

// Sample rates configured for Polar H10 device
// Note: These should match the rates configured via polar.ecg_sample_rate() and polar.acc_sample_rate()
// The actual rate is queried from the device and set to maximum in sensor.rs
const ECG_SAMPLE_RATE_HZ: u64 = 130; // Default ECG sampling rate in Hz (can be configured)
const ACC_SAMPLE_RATE_HZ: u64 = 200; // Default accelerometer sampling rate in Hz

// Nanoseconds in one second
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Container for all Polar H10 data channels
///
/// Manages synchronized time series for each sensor output:
/// - ECG: Electrocardiogram signal
/// - ACC (X/Y/Z): 3-axis accelerometer
/// - HR: Heart rate in BPM
/// - RR: RR intervals (time between heartbeats)
/// - HRV: Heart rate variability (RMSSD)
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

    /// Process heart rate data from Polar sensor
    ///
    /// Handles HR (beats per minute) and RR intervals (time between beats).
    /// Calculates rolling HRV (RMSSD) from recent RR intervals.
    pub fn handle_heart_rate(&mut self, hr: arctic::HeartRate) {
        // Use current system time as approximate timestamp for HR
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        self.hr.add_point(now, (*hr.bpm()).into());

        log::debug!("Heart rate: {:?}", hr);

        let rr = hr.rr().clone().unwrap_or(vec![]);
        let rr_len = rr.len();

        // Handle RR intervals - each interval is a separate data point
        // RR intervals are the time between beats in milliseconds
        if rr_len == 0 {
            // If no RR data, repeat last value to maintain continuity
            if let Some(last) = self.rr.last_point() {
                self.rr.add_point(now, last.value);
            }
        } else {
            // Add each RR interval as a separate point
            // Space them out evenly within the time since last measurement
            let time_spacing = if let Some(last) = self.rr.last_point() {
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

    /// Calculate the starting timestamp for a data stream based on last known point
    ///
    /// ## Why
    /// Sensor data arrives in batches without individual timestamps. We need to
    /// calculate evenly-spaced timestamps based on the known sample rate.
    /// Continues from last point if available to maintain continuity.
    fn calculate_start_time(
        last_point_time: Option<u64>,
        now: u64,
        sample_count: u64,
        timestep_ns: u64,
    ) -> u64 {
        if let Some(last_time) = last_point_time {
            // Continue from last timestamp + one interval for smooth continuation
            last_time + timestep_ns
        } else if sample_count > 0 {
            // First batch: spread backwards from now to avoid future timestamps
            now.saturating_sub((sample_count - 1) * timestep_ns)
        } else {
            // No samples in this batch
            now
        }
    }

    /// Count samples by type in measurement data
    ///
    /// ## Why
    /// We need counts to calculate proper timestamp spacing before processing.
    /// Single pass is more efficient than counting during processing.
    fn count_samples(data: &[PmdData]) -> (u64, u64) {
        let mut ecg_count = 0u64;
        let mut acc_count = 0u64;
        
        for d in data.iter() {
            match d {
                PmdData::Ecg(_) => ecg_count += 1,
                PmdData::Acc(_) => acc_count += 1,
            }
        }
        
        (ecg_count, acc_count)
    }

    /// Process measurement data batch from Polar sensor
    ///
    /// Handles mixed ECG and accelerometer samples, calculating proper timestamps
    /// for each channel based on sample rates and maintaining continuity.
    pub fn handle_measurement_data(&mut self, data: arctic::PmdRead) {
        // Use system time as the reference point for this batch
        // Why: Sensor doesn't provide absolute timestamps, only sample batches
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        
        let data_vec = data.data();
        
        // Count samples by type to properly timestamp each channel
        let (ecg_count, acc_count) = Self::count_samples(&data_vec);
        
        // Calculate timesteps for each data type based on sample rates
        let ecg_timestep = NANOS_PER_SECOND / self.ecg.sample_rate();
        let acc_timestep = NANOS_PER_SECOND / self.acc_x.sample_rate();
        
        // Determine starting timestamps for each channel
        let ecg_start_time = Self::calculate_start_time(
            self.ecg.last_point().map(|p| p.time),
            now,
            ecg_count,
            ecg_timestep,
        );
        
        let acc_start_time = Self::calculate_start_time(
            self.acc_x.last_point().map(|p| p.time),
            now,
            acc_count,
            acc_timestep,
        );
        
        // Track indices per data type for timestamp calculation
        let mut ecg_idx = 0u64;
        let mut acc_idx = 0u64;

        // Process each sample with calculated timestamps
        for d in data_vec.iter() {
            match d {
                PmdData::Acc(acc) => {
                    let t = acc_start_time + (acc_idx * acc_timestep);
                    let acc = acc.data();
                    self.acc_x.add_point(t, acc.0);
                    self.acc_y.add_point(t, acc.1);
                    self.acc_z.add_point(t, acc.2);
                    acc_idx += 1;
                }
                PmdData::Ecg(ecg) => {
                    let t = ecg_start_time + (ecg_idx * ecg_timestep);
                    self.ecg.add_point(t, *ecg.val());
                    ecg_idx += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channels_creation() {
        let channels = Channels::new();
        
        assert_eq!(channels.ecg.len(), 0);
        assert_eq!(channels.hr.len(), 0);
        assert_eq!(channels.rr.len(), 0);
    }

    #[test]
    fn test_set_sample_rates() {
        let mut channels = Channels::new();
        
        channels.set_ecg_sample_rate(256);
        assert_eq!(channels.ecg.sample_rate(), 256);
        
        channels.set_acc_sample_rate(100);
        assert_eq!(channels.acc_x.sample_rate(), 100);
        assert_eq!(channels.acc_y.sample_rate(), 100);
        assert_eq!(channels.acc_z.sample_rate(), 100);
    }

    #[test]
    fn test_calculate_start_time_first_batch() {
        let now = 10_000_000_000; // 10 seconds
        let sample_count = 5;
        let timestep = 1_000_000; // 1ms
        
        let start = Channels::calculate_start_time(None, now, sample_count, timestep);
        
        // Should back-calculate to spread samples evenly before 'now'
        assert_eq!(start, now - (sample_count - 1) * timestep);
    }

    #[test]
    fn test_calculate_start_time_continuation() {
        let last_time = 5_000_000_000;
        let now = 10_000_000_000;
        let sample_count = 5;
        let timestep = 1_000_000;
        
        let start = Channels::calculate_start_time(Some(last_time), now, sample_count, timestep);
        
        // Should continue from last timestamp
        assert_eq!(start, last_time + timestep);
    }

    // Note: Cannot test count_samples because arctic::Ecg and arctic::Acc
    // constructors are private. This is tested indirectly through integration tests.
}
