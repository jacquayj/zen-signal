//! # Visualization Helpers Module
//!
//! Interpolation and data processing utilities for smooth chart rendering.
//! Provides methods to transform raw time series data into visually smooth
//! curves suitable for real-time biosignal visualization.
//!
//! ## Key Functions
//! - `interpolate_point`: Linear interpolation between two points
//! - `interpolate_between_points`: Fill gaps with intermediate points
//! - `add_start_boundary_point`: Ensure data at window left edge
//! - `add_end_boundary_point`: Ensure data at window right edge
//! - `range_from_time_interpolated`: Complete interpolation pipeline
//! - `current_display_time`: Calculate reference time for smooth scrolling
//!
//! ## Display Timing
//! The `current_display_time` function provides a reference time for chart rendering
//! with optional delay for smooth streaming of low-rate data.
//!
//! ## Why Interpolation
//! Low sample rate data (HR, RR at ~1Hz) appears steppy without interpolation.
//! These helpers create smooth curves while preserving the ability to view
//! raw data when interpolation is disabled.

use crate::timeseries::{Point, TimeSeries};

// Display delay for smooth scrolling (1.5 seconds in nanoseconds)
// This prevents gaps when low-rate data (HR, RR, HRV at ~1Hz) hasn't arrived yet
const DISPLAY_DELAY_NS: u64 = 1_500_000_000;

/// Time window duration for chart display
#[allow(dead_code)]
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

/// Get the current display reference time with optional smooth scrolling delay
///
/// ## Why
/// When smooth_streaming is true, returns current time minus a fixed delay to enable
/// smooth scrolling and prevent gaps in low-rate data streams.
/// When smooth_streaming is false, returns current time for immediate rendering.
///
/// ## Parameters
/// - `smooth_streaming`: Whether to apply delay for smooth scrolling
///
/// ## Returns
/// Current system time in nanoseconds, optionally adjusted by DISPLAY_DELAY_NS
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

/// Linear interpolation between two points at a specific time
///
/// ## Why
/// Creates smooth transitions by calculating intermediate values.
/// Used for both boundary interpolation and filling gaps.
pub fn interpolate_point(p1: &Point, p2: &Point, target_time: u64) -> Point {
    let time_diff = p2.time - p1.time;
    let value_diff = p2.value - p1.value;
    let time_from_p1 = target_time - p1.time;
    let progress = time_from_p1 as f64 / time_diff as f64;
    let interpolated_value = p1.value as f64 + (value_diff as f64 * progress);
    
    Point {
        time: target_time,
        value: interpolated_value.round() as i32,
    }
}

/// Interpolate points between two consecutive data points
///
/// ## Why
/// Low sample rate data (like HR at 1Hz) appears steppy without interpolation.
/// This creates smooth curves by adding intermediate points.
pub fn interpolate_between_points(
    p1: &Point,
    p2: &Point,
    target_interval_ns: u64,
    start_time: u64,
    end_time: u64,
) -> Vec<Point> {
    let mut result = Vec::new();
    let time_diff = p2.time.saturating_sub(p1.time);
    let value_diff = p2.value - p1.value;
    
    // Calculate number of interpolated points needed
    let num_steps = (time_diff / target_interval_ns).max(1);
    
    // Add interpolated points within the window
    for step in 0..num_steps {
        let t = p1.time + (time_diff * step / num_steps);
        
        // Only add points within the display window (excluding exact boundaries)
        if t > start_time && t < end_time {
            let progress = step as f64 / num_steps as f64;
            let interpolated_value = p1.value as f64 + (value_diff as f64 * progress);
            
            result.push(Point {
                time: t,
                value: interpolated_value.round() as i32,
            });
        }
    }
    
    result
}

/// Add interpolated point at the start boundary of the time window
///
/// ## Why
/// Ensures charts always show data from the exact left edge, preventing
/// visible gaps when the first real data point is after start_time.
pub fn add_start_boundary_point(result: &mut Vec<Point>, points: &[Point], start_time: u64) {
    let first_in_window = points.iter().position(|p| p.time >= start_time).unwrap_or(0);
    if first_in_window > 0 {
        let p1 = &points[first_in_window - 1];
        let p2 = &points[first_in_window];
        
        if p1.time <= start_time && p2.time >= start_time {
            result.push(interpolate_point(p1, p2, start_time));
        }
    }
}

/// Add interpolated or extrapolated point at the end boundary of the time window
///
/// ## Why
/// Provides smooth scrolling by ensuring charts show data up to the exact
/// right edge. Uses extrapolation cautiously to avoid wild predictions.
pub fn add_end_boundary_point(result: &mut Vec<Point>, points: &[Point], end_time: u64) {
    let last_before_end = points.iter().rposition(|p| p.time <= end_time)
        .unwrap_or(points.len() - 1);
        
    if last_before_end + 1 < points.len() {
        // Interpolate between last point before end and first point after
        let p1 = &points[last_before_end];
        let p2 = &points[last_before_end + 1];
        
        if p1.time <= end_time && p2.time >= end_time {
            result.push(interpolate_point(p1, p2, end_time));
        }
    } else if points.len() >= 2 {
        // Extrapolate from last two points (with limits)
        let p1 = &points[points.len() - 2];
        let p2 = &points[points.len() - 1];
        
        if p2.time < end_time {
            let time_diff = p2.time - p1.time;
            let value_diff = p2.value - p1.value;
            let time_from_p2 = end_time - p2.time;
            
            // Only extrapolate if gap is reasonable (within 3x normal interval)
            // Why: Prevents wild extrapolations when data stream is interrupted
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

/// Get points with linear interpolation for smooth curves.
///
/// Adds interpolated points between actual data points to create smoother lines.
/// This is especially important for low-rate data streams (HR, RR at ~1Hz).
///
/// ## Parameters
/// - `ts`: The time series to interpolate
/// - `end_time`: Right edge of the time window
/// - `duration_ns`: Width of the time window
/// - `target_interval_ns`: Desired time between interpolated points (e.g., 100ms)
/// - `interpolate_end`: If true, adds interpolated point at end_time boundary
///
/// ## Why interpolate_end parameter
/// When smooth streaming is enabled, we delay display time to ensure data has arrived.
/// In this case, we can safely interpolate at the end boundary. Without delay,
/// interpolating at the current moment would create artifacts when new data arrives.
pub fn range_from_time_interpolated(
    ts: &TimeSeries,
    end_time: u64,
    duration_ns: u64,
    target_interval_ns: u64,
    interpolate_end: bool,
) -> Vec<Point> {
    if ts.is_empty() {
        return Vec::new();
    }

    let start_time = end_time.saturating_sub(duration_ns);
    
    // Find indices for points in and around the window
    let start_idx = ts.partition_point_time(start_time);
    let end_idx = ts.partition_point_time_inclusive(end_time);
    
    // Include points just outside the window for interpolation
    let actual_start_idx = if start_idx > 0 { start_idx - 1 } else { start_idx };
    let actual_end_idx = (end_idx + 1).min(ts.len());
    let points_for_interp = ts.slice(actual_start_idx, actual_end_idx);
    
    if points_for_interp.len() < 2 {
        // Not enough points to interpolate, fall back to forward-fill
        return ts.range_from_time_with_fill(end_time, duration_ns);
    }
    
    // Pre-allocate capacity for better performance
    let estimated_size = (duration_ns / target_interval_ns) as usize;
    let mut result = Vec::with_capacity(estimated_size);
    
    // Add interpolated point at start boundary
    add_start_boundary_point(&mut result, points_for_interp, start_time);
    
    // Interpolate between consecutive points
    for window in points_for_interp.windows(2) {
        let interpolated = interpolate_between_points(
            &window[0],
            &window[1],
            target_interval_ns,
            start_time,
            end_time,
        );
        result.extend(interpolated);
    }
    
    // Add actual data points that fall within the window
    for point in points_for_interp {
        if point.time > start_time && point.time < end_time {
            result.push(Point {
                time: point.time,
                value: point.value,
            });
        }
    }
    
    // Conditionally add interpolated point at end boundary
    if interpolate_end {
        add_end_boundary_point(&mut result, points_for_interp, end_time);
    }
    
    // Sort by time since we added points out of order
    result.sort_by_key(|p| p.time);
    
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interpolate_point() {
        let p1 = Point { time: 1000, value: 100 };
        let p2 = Point { time: 3000, value: 300 };
        let target_time = 2000;
        
        let interpolated = interpolate_point(&p1, &p2, target_time);
        
        assert_eq!(interpolated.time, 2000);
        assert_eq!(interpolated.value, 200); // Midpoint
    }

    #[test]
    fn test_interpolate_between_points() {
        let p1 = Point { time: 1000, value: 100 };
        let p2 = Point { time: 5000, value: 500 };
        let target_interval = 1000;
        
        let interpolated = interpolate_between_points(&p1, &p2, target_interval, 0, 6000);
        
        // Should create points at 1000, 2000, 3000, 4000 (excluding boundaries)
        assert_eq!(interpolated.len(), 4);
    }

    #[test]
    fn test_add_start_boundary_point() {
        let points = vec![
            Point { time: 500, value: 50 },
            Point { time: 2000, value: 200 },
        ];
        let mut result = Vec::new();
        
        add_start_boundary_point(&mut result, &points, 1000);
        
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].time, 1000);
        // Linear interpolation: 50 + (200-50) * (1000-500)/(2000-500) = 50 + 150 * 0.333 = 100
        assert_eq!(result[0].value, 100);
    }
}
