//! # Time Series Data Storage Module
//!
//! Generic time series storage and query utilities for timestamped data.
//! Provides efficient storage, retrieval, and statistical operations on
//! sequential data points without any sensor-specific logic.
//!
//! ## Key Types
//! - `Point`: Individual timestamped data point (time in nanoseconds, integer value)
//! - `TimeSeries`: Vector-based storage with query methods
//! - `PointSliceExt`: Statistical operations on point slices (min/max, RMSSD)
//!
//! ## Architecture
//! TimeSeries maintains sorted points and provides efficient time-windowed queries
//! using binary search. Query methods return slices when possible to avoid allocation.
//!
//! ## Usage
//! For sensor-specific data handling, see `polar_data` module.
//! For visualization helpers, see `visualization` module.

// Nanoseconds in one second
#[allow(dead_code)]
const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Time conversion constants
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
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

/// Individual timestamped data point
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub time: u64,   // Timestamp in nanoseconds
    pub value: i32,  // Integer value
}

/// Trait for statistical operations on point slices
#[allow(dead_code)]
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

    // RMSSD (Root Mean Square of Successive Differences)
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

/// Time series storage with efficient queries
pub struct TimeSeries {
    data: Vec<Point>,
    sample_rate: u64, // Sample rate in Hz (nominal, for calculating expected intervals)
    start_time: Option<u64>, // First timestamp in nanoseconds
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

    /// Check if time series is empty
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// Get number of points
    pub fn len(&self) -> usize {
        self.data.len()
    }

    /// Get the last point if it exists
    pub fn last_point(&self) -> Option<&Point> {
        self.data.last()
    }

    /// Binary search for first point >= target time
    pub fn partition_point_time(&self, target_time: u64) -> usize {
        self.data.partition_point(|p| p.time < target_time)
    }

    /// Binary search for first point > target time
    pub fn partition_point_time_inclusive(&self, target_time: u64) -> usize {
        self.data.partition_point(|p| p.time <= target_time)
    }

    /// Get a slice of the internal data
    pub fn slice(&self, start_idx: usize, end_idx: usize) -> &[Point] {
        &self.data[start_idx..end_idx]
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeseries_add_point() {
        let mut ts = TimeSeries::new(100);
        ts.add_point(1000, 42);
        ts.add_point(2000, 84);
        
        assert_eq!(ts.len(), 2);
        assert_eq!(ts.last_point().unwrap().value, 84);
    }

    #[test]
    fn test_timeseries_last_points() {
        let mut ts = TimeSeries::new(100);
        ts.add_point(1000, 1);
        ts.add_point(2000, 2);
        ts.add_point(3000, 3);
        
        let last = ts.last_points(2);
        assert_eq!(last.len(), 2);
        assert_eq!(last[0].value, 2);
        assert_eq!(last[1].value, 3);
    }

    #[test]
    fn test_timeseries_last_duration() {
        let mut ts = TimeSeries::new(100);
        ts.add_point(1_000_000_000, 1);
        ts.add_point(2_000_000_000, 2);
        ts.add_point(3_000_000_000, 3);
        ts.add_point(4_000_000_000, 4);
        
        // Get last 2 seconds worth of data
        // From time 4s back to 2s (inclusive) = points at 2s, 3s, 4s
        let recent = ts.last_duration(2_000_000_000);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].value, 2);
        assert_eq!(recent[1].value, 3);
        assert_eq!(recent[2].value, 4);
    }

    #[test]
    fn test_rmssd_calculation() {
        let points = vec![
            Point { time: 1000, value: 800 },
            Point { time: 2000, value: 820 },
            Point { time: 3000, value: 810 },
            Point { time: 4000, value: 830 },
        ];
        
        let rmssd = points.as_slice().rmssd();
        // RMSSD = sqrt(mean of squared differences)
        // Differences: 20, -10, 20
        // Squared: 400, 100, 400
        // Mean: 300
        // sqrt(300) ≈ 17.32
        assert!((rmssd - 17.32).abs() < 0.1);
    }

    #[test]
    fn test_set_sample_rate() {
        let mut ts = TimeSeries::new(100);
        assert_eq!(ts.sample_rate(), 100);
        
        ts.set_sample_rate(200);
        assert_eq!(ts.sample_rate(), 200);
    }

    #[test]
    fn test_range_from_time() {
        let mut ts = TimeSeries::new(100);
        ts.add_point(1_000_000_000, 1);
        ts.add_point(2_000_000_000, 2);
        ts.add_point(3_000_000_000, 3);
        ts.add_point(4_000_000_000, 4);
        ts.add_point(5_000_000_000, 5);
        
        // Get points from 2s to 4s
        let range = ts.range_from_time(4_000_000_000, 2_000_000_000);
        
        assert_eq!(range.len(), 3);
        assert_eq!(range[0].value, 2);
        assert_eq!(range[1].value, 3);
        assert_eq!(range[2].value, 4);
    }
}
