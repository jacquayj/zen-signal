//! # Recording and Data Storage Module
//!
//! Manages persistent recording of sensor data to HDF5 files with memory-efficient buffering.
//! Provides transparent access to data whether in memory or on disk.
//!
//! ## Architecture
//! - **PolarDataManager**: Orchestrates recording lifecycle and memory management
//! - **ChannelBuffer**: Per-channel memory buffer with configurable limits
//! - **Writer Thread**: Background thread for async HDF5 file operations
//! - **Reader**: Transparent data access from memory cache or file
//!
//! ## Key Features
//! 1. Non-blocking writes: Data flows sensor → memory → async file writer
//! 2. Memory limits: Configurable max memory with automatic flushing
//! 3. HDF5 format: Industry standard, excellent Python support (h5py)
//! 4. Transparent reads: Automatic fallback from cache to file
//!
//! ## HDF5 File Structure
//! ```text
//! recording_YYYYMMDD_HHMMSS.h5
//! ├── /ecg
//! │   ├── times (dataset: u64 nanoseconds)
//! │   └── values (dataset: i32)
//! ├── /acc_x
//! │   ├── times
//! │   └── values
//! ... (similar for acc_y, acc_z, hr, rr, hrv)
//! └── /metadata
//!     ├── start_time (attribute)
//!     ├── sample_rates (group with attributes)
//!     └── description (attribute)
//! ```
//!
//! ## Memory Management
//! - Each channel maintains a rolling buffer
//! - When total memory exceeds limit, oldest data is flushed to disk
//! - LRU eviction policy for multi-channel fairness

use crate::timeseries::Point;
use crossbeam_channel::{unbounded, Receiver, Sender};
use hdf5::{File, Group};
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

/// Channel identifier for routing data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelId {
    Ecg,
    AccX,
    AccY,
    AccZ,
    Hr,
    Rr,
    Hrv,
}

impl ChannelId {
    /// Get HDF5 group name for this channel
    pub fn group_name(&self) -> &'static str {
        match self {
            ChannelId::Ecg => "ecg",
            ChannelId::AccX => "acc_x",
            ChannelId::AccY => "acc_y",
            ChannelId::AccZ => "acc_z",
            ChannelId::Hr => "hr",
            ChannelId::Rr => "rr",
            ChannelId::Hrv => "hrv",
        }
    }

    /// Get all channel IDs
    pub fn all() -> Vec<ChannelId> {
        vec![
            ChannelId::Ecg,
            ChannelId::AccX,
            ChannelId::AccY,
            ChannelId::AccZ,
            ChannelId::Hr,
            ChannelId::Rr,
            ChannelId::Hrv,
        ]
    }
}

/// Recording command sent from main thread to writer thread
#[derive(Debug)]
enum RecorderCommand {
    /// Add data point to channel buffer
    AddPoint {
        channel: ChannelId,
        time: u64,
        value: i32,
    },
    /// Flush all buffers to disk
    Flush,
    /// Stop recording and cleanup
    Stop,
}

/// Cache key for LRU cache (channel, start_time, end_time)
type CacheKey = (ChannelId, u64, u64);

/// State shared between recorder and writer thread
struct RecorderState {
    /// Unified LRU cache for both live and file data
    cache: LruCache<CacheKey, Vec<Point>>,
    /// Per-channel staging buffers for incoming live data (before caching)
    staging_buffers: HashMap<ChannelId, Vec<Point>>,
    /// Current memory usage in bytes
    memory_usage: usize,
    /// Maximum memory allowed in bytes
    max_memory: usize,
    /// Path to HDF5 file (if recording to disk)
    file_path: Option<PathBuf>,
    /// Recording start time
    start_time: u64,
    /// Duration of live buffer in nanoseconds
    live_buffer_duration_ns: u64,
    /// Whether to write to disk
    write_to_disk: bool,
    /// Time range of opened file (min, max) in nanoseconds
    file_time_range: Option<(u64, u64)>,
}

impl RecorderState {
    fn new(live_buffer_seconds: u64, max_memory_mb: usize) -> Self {
        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        let mut staging_buffers = HashMap::new();
        for channel_id in ChannelId::all() {
            staging_buffers.insert(channel_id, Vec::new());
        }

        // Use full memory budget for unified cache
        let cache_size = max_memory_mb * 100; // Rough estimate: ~1KB per cache entry
        let cache_capacity = NonZeroUsize::new(cache_size.max(100)).unwrap();

        Self {
            cache: LruCache::new(cache_capacity),
            staging_buffers,
            memory_usage: 0,
            max_memory: max_memory_mb * 1024 * 1024,
            file_path: None,
            start_time,
            live_buffer_duration_ns: live_buffer_seconds * 1_000_000_000,
            write_to_disk: false,
            file_time_range: None,
        }
    }

    /// Add point to staging buffer and update memory tracking
    fn add_point(&mut self, channel: ChannelId, time: u64, value: i32) {
        if let Some(buffer) = self.staging_buffers.get_mut(&channel) {
            buffer.push(Point { time, value });
            // Each Point is 12 bytes (u64 + i32 + padding)
            self.memory_usage += 16;
        }
    }

    /// Check if memory limit exceeded
    fn should_flush(&self) -> bool {
        // Flush frequently when recording to disk to prevent data loss
        if self.write_to_disk {
            self.memory_usage >= 100_000 // 100 KB (~6 seconds of data)
        } else {
            self.memory_usage >= self.max_memory
        }
    }

    /// Get total points across all channels (staging + cache)
    fn total_points(&self) -> usize {
        let staging_points: usize = self.staging_buffers.values().map(|v| v.len()).sum();
        let cache_points: usize = self.cache.iter().map(|(_, v)| v.len()).sum();
        staging_points + cache_points
    }

    /// Enable disk recording with specified file path
    fn enable_disk_recording(&mut self, file_path: PathBuf) {
        self.file_path = Some(file_path);
        self.write_to_disk = true;
    }

    /// Disable disk recording
    fn disable_disk_recording(&mut self) {
        self.write_to_disk = false;
    }

    /// Move staged data to cache and age out old entries
    fn age_out_old_data(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        let _cutoff_time = now.saturating_sub(self.live_buffer_duration_ns);

        // Move staging buffers to cache with proper time range keys
        for (channel, buffer) in self.staging_buffers.iter_mut() {
            if !buffer.is_empty() {
                let start_time = buffer.first().unwrap().time;
                let end_time = buffer.last().unwrap().time;
                let cache_key = (*channel, start_time, end_time);
                self.cache.put(cache_key, buffer.clone());
                buffer.clear();
            }
        }
        
        // Reset memory usage for staging (cache manages its own memory)
        self.memory_usage = 0;
    }

    /// Flush all buffers to HDF5 file
    fn flush_to_disk(&mut self) -> Result<(), String> {
        let staging_points: usize = self.staging_buffers.values().map(|v| v.len()).sum();
        
        if staging_points == 0 {
            return Ok(());
        }

        // If we have a file path, write staging buffers to file
        // (This can be called on stop even after write_to_disk is disabled)
        if let Some(file_path) = &self.file_path {
            log::debug!(
                "Flushing {} points ({:.2} MB) to disk",
                staging_points,
                self.memory_usage as f64 / (1024.0 * 1024.0)
            );

            // Open or create HDF5 file
            let file = File::open_rw(file_path)
                .or_else(|_| File::create(file_path))
                .map_err(|e| format!("Failed to open HDF5 file: {}", e))?;

            // Write each channel's staging buffer
            for (channel_id, buffer) in self.staging_buffers.iter() {
                if buffer.is_empty() {
                    continue;
                }

                let group_name = channel_id.group_name();
                
                // Get or create group for this channel
                let group = if file.group(group_name).is_ok() {
                    file.group(group_name)
                        .map_err(|e| format!("Failed to open group {}: {}", group_name, e))?
                } else {
                    file.create_group(group_name)
                        .map_err(|e| format!("Failed to create group {}: {}", group_name, e))?
                };

                // Append times and values to datasets
                Self::append_to_dataset(&group, "times", buffer, |p| p.time)?;
                Self::append_to_dataset(&group, "values", buffer, |p| p.value)?;
            }

            // Write metadata
            Self::write_metadata(&file, self.start_time)?;

            log::debug!("Flush complete");
        }

        // Move staging to cache (for both recording and live modes)
        self.age_out_old_data();

        Ok(())
    }

    /// Append data to HDF5 dataset, creating if needed
    fn append_to_dataset<T, F>(
        group: &Group,
        dataset_name: &str,
        buffer: &[Point],
        extractor: F,
    ) -> Result<(), String>
    where
        T: hdf5::H5Type,
        F: Fn(&Point) -> T,
    {
        let values: Vec<T> = buffer.iter().map(extractor).collect();

        if let Ok(dataset) = group.dataset(dataset_name) {
            // Dataset exists, append to it
            let old_size = dataset.size();
            let new_size = old_size + values.len();
            dataset
                .resize(new_size)
                .map_err(|e| format!("Failed to resize dataset {}: {}", dataset_name, e))?;

            // Write new values at the end using simple slice notation
            dataset
                .write_slice(&values, old_size..new_size)
                .map_err(|e| format!("Failed to write to dataset {}: {}", dataset_name, e))?
        } else {
            // Create new dataset with chunking and unlimited max dimensions for resizing
            // Chunk size must not exceed initial data size
            let chunk_size = values.len().max(1).min(1024);
            
            use hdf5::Extent;
            let dataset = group
                .new_dataset::<T>()
                .shape(Extent::resizable(values.len())) // Resizable with unlimited max
                .chunk((chunk_size,))
                .create(dataset_name)
                .map_err(|e| format!("Failed to create dataset {}: {}", dataset_name, e))?;

            dataset
                .write(&values)
                .map_err(|e| format!("Failed to write dataset {}: {}", dataset_name, e))?;
        }

        Ok(())
    }

    /// Write recording metadata
    fn write_metadata(file: &File, start_time: u64) -> Result<(), String> {
        if file.group("metadata").is_err() {
            let metadata = file
                .create_group("metadata")
                .map_err(|e| format!("Failed to create metadata group: {}", e))?;

            metadata
                .new_attr::<u64>()
                .create("start_time_ns")
                .map_err(|e| format!("Failed to create start_time attribute: {}", e))?
                .write_scalar(&start_time)
                .map_err(|e| format!("Failed to write start_time: {}", e))?;

            // Note: String attributes require VarLenUnicode type in hdf5 0.8.1
            // For simplicity, we'll skip the description or use a fixed-size string
            // Users can add metadata via Python/h5py if needed
        }

        Ok(())
    }
}

/// Main data manager for Polar sensor data
///
/// Coordinates recording lifecycle, manages memory buffers, and provides
/// transparent data access. Thread-safe and non-blocking for UI.
/// Always exists to provide live data buffering, optionally writes to disk.
pub struct PolarDataManager {
    /// Command sender for writer thread
    command_tx: Sender<RecorderCommand>,
    /// Writer thread handle
    writer_thread: Option<thread::JoinHandle<()>>,
    /// Shared state for queries (wrapped for safe sharing)
    state: Arc<Mutex<RecorderState>>,
    /// Whether disk recording is active
    is_recording: Arc<Mutex<bool>>,
}

impl PolarDataManager {
    /// Create a new data manager
    ///
    /// # Arguments
    /// * `live_buffer_seconds` - Duration of live data to keep in memory
    /// * `max_memory_mb` - Maximum memory buffer size in MB before flushing
    pub fn new(live_buffer_seconds: u64, max_memory_mb: usize) -> Result<Self, String> {
        let (command_tx, command_rx) = unbounded();
        let initial_state = RecorderState::new(live_buffer_seconds, max_memory_mb);
        let state = Arc::new(Mutex::new(initial_state));
        let state_clone = state.clone();
        let is_recording = Arc::new(Mutex::new(false));

        // Spawn writer thread
        let writer_thread = thread::spawn(move || {
            Self::writer_loop(command_rx, state_clone);
        });

        Ok(PolarDataManager {
            command_tx,
            writer_thread: Some(writer_thread),
            state,
            is_recording,
        })
    }

    /// Start recording to a new file
    ///
    /// Creates a timestamped HDF5 file and enables disk writing.
    pub fn start_recording(&self, output_dir: impl AsRef<Path>) -> Result<(), String> {
        let mut is_recording = self.is_recording.lock().unwrap();
        if *is_recording {
            return Err("Recording already in progress".to_string());
        }

        let output_path = output_dir.as_ref();
        
        // Verify directory exists
        if !output_path.exists() {
            std::fs::create_dir_all(&output_path)
                .map_err(|e| format!("Failed to create output directory: {}", e))?;
        }

        // Generate timestamped filename
        let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
        let filename = format!("recording_{}.h5", timestamp);
        let file_path = output_path.join(filename);

        log::info!("Starting recording to: {}", file_path.display());

        // Enable disk recording
        self.state.lock().unwrap().enable_disk_recording(file_path);
        *is_recording = true;

        Ok(())
    }

    /// Stop recording and flush remaining data
    pub fn stop_recording(&self) -> Result<(), String> {
        let mut is_recording = self.is_recording.lock().unwrap();
        if !*is_recording {
            return Ok(());
        }

        log::info!("Stopping recording");

        // Send flush command to ensure data is written
        self.command_tx
            .send(RecorderCommand::Flush)
            .map_err(|e| format!("Failed to send flush command: {}", e))?;

        // Disable disk recording
        self.state.lock().unwrap().disable_disk_recording();
        *is_recording = false;

        Ok(())
    }

    /// Add a data point to the live buffer
    ///
    /// Non-blocking: queues data for async processing. Safe to call from data collection thread.
    /// Points are always buffered in memory. If disk recording is active, they're also written to file.
    pub fn add_point(&self, channel: ChannelId, time: u64, value: i32) -> Result<(), String> {
        self.command_tx
            .send(RecorderCommand::AddPoint {
                channel,
                time,
                value,
            })
            .map_err(|e| format!("Failed to send add_point command: {}", e))?;

        Ok(())
    }

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        *self.is_recording.lock().unwrap()
    }

    /// Get current memory usage in MB
    pub fn memory_usage_mb(&self) -> f64 {
        let state = self.state.lock().unwrap();
        // Staging buffer memory + rough estimate for cache
        let staging_memory = state.memory_usage as f64;
        let cache_points: usize = state.cache.iter().map(|(_, v)| v.len()).sum();
        let cache_memory = (cache_points * 16) as f64; // 16 bytes per point
        (staging_memory + cache_memory) / (1024.0 * 1024.0)
    }

    /// Get current point count across all channels
    pub fn total_points(&self) -> usize {
        let state = self.state.lock().unwrap();
        state.total_points()
    }

    /// Open an existing recording file for reading
    ///
    /// Sets file path for transparent read access.
    /// Does not affect live buffering.
    ///
    /// # Arguments
    /// * `file_path` - Path to HDF5 recording file
    ///
    /// # Returns
    /// Ok with (start_time, end_time) tuple, or error message
    #[allow(dead_code)]
    pub fn open_recording(&self, file_path: impl AsRef<Path>) -> Result<(u64, u64), String> {
        let file_path = file_path.as_ref().to_path_buf();
        
        if !file_path.exists() {
            return Err(format!("File not found: {}", file_path.display()));
        }

        log::info!("Opening recording for read: {}", file_path.display());

        // Read time range from file
        let time_range = Self::read_time_range_from_file(&file_path)?;

        // Set file path for read access
        let mut state = self.state.lock().unwrap();
        state.file_time_range = Some(time_range);
        state.cache.clear();

        log::info!("Opened recording: {} to {} ({:.2}s duration)", 
            time_range.0, time_range.1, 
            (time_range.1 - time_range.0) as f64 / 1e9);

        Ok(time_range)
    }

    /// Close opened file
    #[allow(dead_code)]
    pub fn close_recording(&self) -> Result<(), String> {
        let mut state = self.state.lock().unwrap();

        log::info!("Closing opened file");
        
        state.file_time_range = None;
        state.cache.clear();

        Ok(())
    }

    /// Get time range of opened file
    ///
    /// Returns None if in live mode or no file is open
    #[allow(dead_code)]
    pub fn file_time_range(&self) -> Option<(u64, u64)> {
        let state = self.state.lock().unwrap();
        state.file_time_range
    }

    /// Read time range from HDF5 file
    ///
    /// Scans all channels and returns (min_time, max_time) tuple
    fn read_time_range_from_file(file_path: &Path) -> Result<(u64, u64), String> {
        let file = File::open(file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let mut min_time = u64::MAX;
        let mut max_time = u64::MIN;

        // Check each channel for time range
        for channel in ChannelId::all() {
            let group_name = channel.group_name();
            
            if let Ok(group) = file.group(group_name) {
                if let Ok(times_ds) = group.dataset("times") {
                    if let Ok(times) = times_ds.read_raw::<u64>() {
                        if !times.is_empty() {
                            min_time = min_time.min(*times.first().unwrap());
                            max_time = max_time.max(*times.last().unwrap());
                        }
                    }
                }
            }
        }

        if min_time == u64::MAX || max_time == u64::MIN {
            return Err("No valid time data found in file".to_string());
        }

        Ok((min_time, max_time))
    }

    /// Read data from a specific channel and time range
    ///
    /// Transparently reads from memory buffer, cache, or file.
    /// Checks in order: memory → cache → file
    ///
    /// # Arguments
    /// * `channel` - Channel to read from
    /// * `start_time` - Start of time range in nanoseconds
    /// * `end_time` - End of time range in nanoseconds
    ///
    /// # Returns
    /// Vector of points within the time range, or error message
    #[allow(dead_code)]
    pub fn read_range(
        &self,
        channel: ChannelId,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<Point>, String> {
        let mut state = self.state.lock().unwrap();
        let mut result = Vec::new();

        // First, check staging buffer for very recent live data
        if let Some(buffer) = state.staging_buffers.get(&channel) {
            for point in buffer {
                if point.time >= start_time && point.time <= end_time {
                    result.push(*point);
                }
            }
        }

        // If we got data from staging, return it (most recent live data)
        if !result.is_empty() {
            return Ok(result);
        }

        // Check unified cache (contains both historical live data and file chunks)
        let cache_key = (channel, start_time, end_time);
        if let Some(cached_data) = state.cache.get(&cache_key) {
            return Ok(cached_data.clone());
        }

        // Read from file if available
        if let Some(file_path) = &state.file_path {
            let file_path = file_path.clone();
            drop(state); // Release lock during file I/O

            let data = Self::read_from_file(&file_path, channel, start_time, end_time)?;
            
            // Cache the result
            let mut state = self.state.lock().unwrap();
            state.cache.put(cache_key, data.clone());

            // Prefetch adjacent chunks for smooth scrolling
            let chunk_duration = end_time - start_time;
            let prev_start = start_time.saturating_sub(chunk_duration);
            let prev_end = start_time;
            let next_start = end_time;
            let next_end = end_time + chunk_duration;

            // Prefetch previous chunk if not cached
            let prev_key = (channel, prev_start, prev_end);
            if !state.cache.contains(&prev_key) {
                if let Ok(prev_data) = Self::read_from_file(&file_path, channel, prev_start, prev_end) {
                    state.cache.put(prev_key, prev_data);
                }
            }

            // Prefetch next chunk if not cached
            let next_key = (channel, next_start, next_end);
            if !state.cache.contains(&next_key) {
                if let Ok(next_data) = Self::read_from_file(&file_path, channel, next_start, next_end) {
                    state.cache.put(next_key, next_data);
                }
            }

            Ok(data)
        } else {
            // No data in staging and no file open
            Ok(result)
        }
    }

    /// Read data from HDF5 file
    fn read_from_file(
        file_path: &std::path::Path,
        channel: ChannelId,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<Point>, String> {
        let file = File::open_rw(file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let group_name = channel.group_name();
        let group = file
            .group(group_name)
            .map_err(|e| format!("Failed to open group {}: {}", group_name, e))?;

        // Read times dataset
        let times_ds = group
            .dataset("times")
            .map_err(|e| format!("Failed to open times dataset: {}", e))?;
        let times: Vec<u64> = times_ds
            .read_raw()
            .map_err(|e| format!("Failed to read times: {}", e))?;

        // Read values dataset
        let values_ds = group
            .dataset("values")
            .map_err(|e| format!("Failed to open values dataset: {}", e))?;
        let values: Vec<i32> = values_ds
            .read_raw()
            .map_err(|e| format!("Failed to read values: {}", e))?;

        // Filter by time range and create Points
        let mut result = Vec::new();
        for (time, value) in times.iter().zip(values.iter()) {
            if *time >= start_time && *time <= end_time {
                result.push(Point {
                    time: *time,
                    value: *value,
                });
            }
        }

        Ok(result)
    }

    /// Get duration of recording in seconds
    #[allow(dead_code)]
    pub fn duration_seconds(&self) -> f64 {
        let state = self.state.lock().unwrap();
        
        // Calculate from start_time to now for live buffer
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;
        (now - state.start_time) as f64 / 1e9
    }

    /// Get last point in a channel from staging buffer
    #[allow(dead_code)]
    pub fn last_point(&self, channel: ChannelId) -> Option<Point> {
        let state = self.state.lock().unwrap();
        state.staging_buffers.get(&channel)?.last().copied()
    }

    /// Read all data from a channel in the file
    #[allow(dead_code)]
    fn read_all_from_file(
        file_path: &std::path::Path,
        channel: ChannelId,
    ) -> Result<Vec<Point>, String> {
        let file = File::open_rw(file_path)
            .map_err(|e| format!("Failed to open file: {}", e))?;

        let group_name = channel.group_name();
        let group = file
            .group(group_name)
            .map_err(|e| format!("Failed to open group {}: {}", group_name, e))?;

        let times_ds = group
            .dataset("times")
            .map_err(|e| format!("Failed to open times dataset: {}", e))?;
        let times: Vec<u64> = times_ds
            .read_raw()
            .map_err(|e| format!("Failed to read times: {}", e))?;

        let values_ds = group
            .dataset("values")
            .map_err(|e| format!("Failed to open values dataset: {}", e))?;
        let values: Vec<i32> = values_ds
            .read_raw()
            .map_err(|e| format!("Failed to read values: {}", e))?;

        Ok(times
            .into_iter()
            .zip(values.into_iter())
            .map(|(time, value)| Point { time, value })
            .collect())
    }

    /// Writer thread loop
    ///
    /// Processes commands from main thread and handles file I/O.
    /// Automatically flushes when memory limit reached and ages out old data.
    fn writer_loop(command_rx: Receiver<RecorderCommand>, state: Arc<Mutex<RecorderState>>) {
        loop {
            match command_rx.recv() {
                Ok(RecorderCommand::AddPoint {
                    channel,
                    time,
                    value,
                }) => {
                    let mut recorder_state = state.lock().unwrap();
                    recorder_state.add_point(channel, time, value);

                    // Auto-flush when memory limit reached (this also ages out data)
                    if recorder_state.should_flush() {
                        if let Err(e) = recorder_state.flush_to_disk() {
                            log::error!("Failed to flush data: {}", e);
                        }
                    }
                }
                Ok(RecorderCommand::Flush) => {
                    let mut recorder_state = state.lock().unwrap();
                    if let Err(e) = recorder_state.flush_to_disk() {
                        log::error!("Failed to flush data: {}", e);
                    }
                }
                Ok(RecorderCommand::Stop) => {
                    // Final flush before stopping
                    let mut recorder_state = state.lock().unwrap();
                    if let Err(e) = recorder_state.flush_to_disk() {
                        log::error!("Failed to flush data on stop: {}", e);
                    }
                    log::info!("Writer thread stopped");
                    break;
                }
                Err(_) => {
                    log::info!("Writer thread: command channel closed");
                    break;
                }
            }
        }
    }
}

impl Drop for PolarDataManager {
    fn drop(&mut self) {
        // Ensure recording is stopped
        let _ = self.stop_recording();

        // Signal writer thread to stop
        let _ = self.command_tx.send(RecorderCommand::Stop);

        // Wait for writer thread to finish
        if let Some(handle) = self.writer_thread.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_recorder_creation() {
        let recorder = PolarDataManager::new(30, 10);
        assert!(recorder.is_ok());
    }

    #[test]
    fn test_start_stop_recording() {
        let temp_dir = tempdir().unwrap();
        let recorder = PolarDataManager::new(30, 10).unwrap();

        assert!(!recorder.is_recording());
        assert!(recorder.start_recording(temp_dir.path()).is_ok());
        assert!(recorder.is_recording());
        assert!(recorder.stop_recording().is_ok());
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_channel_id_names() {
        assert_eq!(ChannelId::Ecg.group_name(), "ecg");
        assert_eq!(ChannelId::AccX.group_name(), "acc_x");
        assert_eq!(ChannelId::Hr.group_name(), "hr");
    }
}
