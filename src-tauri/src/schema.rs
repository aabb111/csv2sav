use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

const BUF_SIZE: usize = 256 * 1024;
/// SPSS Very Long String max: 32767 bytes per logical variable.
pub const MAX_STRING_WIDTH: usize = 32767;
/// Minimum declared width for string columns so short strings still have some room.
const MIN_STRING_WIDTH: usize = 64;

#[derive(Debug, Clone)]
pub enum ColType {
    Numeric,
    /// Width in bytes (1..=32767).
    String(usize),
}

#[derive(Debug, Clone)]
pub struct ColInfo {
    is_numeric: bool,
    pub max_byte_len: usize,
}

impl ColInfo {
    pub fn new() -> Self {
        Self {
            is_numeric: true,
            max_byte_len: 0,
        }
    }

    pub fn observe_type(&mut self, value: &str) {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return;
        }
        if self.is_numeric && trimmed.parse::<f64>().is_err() {
            self.is_numeric = false;
        }
    }

    pub fn observe_width(&mut self, value: &str) {
        let byte_len = value.trim().len();
        if byte_len > self.max_byte_len {
            self.max_byte_len = byte_len;
        }
    }

    pub fn col_type(&self) -> ColType {
        if self.is_numeric {
            ColType::Numeric
        } else {
            let width = self.max_byte_len.max(MIN_STRING_WIDTH).min(MAX_STRING_WIDTH);
            ColType::String(width)
        }
    }
}

#[derive(Debug, Clone)]
pub struct CsvSchema {
    pub headers: Vec<String>,
    pub col_types: Vec<ColType>,
    pub file_size: u64,
    pub row_count: usize,
    /// Column names whose observed values exceed MAX_STRING_WIDTH and will be truncated.
    pub truncated_cols: Vec<String>,
}

/// Full scan: counts rows and records the actual max byte width for each column.
pub fn scan_rows_and_widths(
    path: &Path,
    col_infos: &mut [ColInfo],
    cancelled: &AtomicBool,
) -> Result<usize, String> {
    let file = File::open(path).map_err(|e| format!("Failed to open CSV: {e}"))?;
    let buf = BufReader::with_capacity(BUF_SIZE, file);
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(buf);

    let mut count = 0usize;
    for result in reader.records() {
        let record =
            result.map_err(|e| format!("CSV read error at row {}: {e}", count + 1))?;
        count += 1;

        for (i, field) in record.iter().enumerate() {
            if i < col_infos.len() {
                col_infos[i].observe_width(field);
            }
        }

        if count % 100_000 == 0 && cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }
    }
    Ok(count)
}

pub fn infer_schema(
    path: &Path,
    sample_rows: usize,
    cancelled: &AtomicBool,
) -> Result<CsvSchema, String> {
    let file_size = fs::metadata(path).map(|m| m.len()).unwrap_or(0);

    let file = File::open(path).map_err(|e| format!("Failed to open CSV: {e}"))?;
    let buf = BufReader::with_capacity(BUF_SIZE, file);
    let mut reader = csv::Reader::from_reader(buf);

    let headers: Vec<String> = reader
        .headers()
        .map_err(|e| format!("Failed to read CSV headers: {e}"))?
        .iter()
        .map(|h| h.to_string())
        .collect();

    if headers.is_empty() {
        return Err("CSV has no columns".to_string());
    }

    let mut col_infos: Vec<ColInfo> = vec![ColInfo::new(); headers.len()];
    let mut sampled_rows = 0usize;

    for result in reader.records() {
        if cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let record =
            result.map_err(|e| format!("CSV read error at row {}: {e}", sampled_rows + 1))?;
        sampled_rows += 1;

        for (i, field) in record.iter().enumerate() {
            if i < col_infos.len() {
                col_infos[i].observe_type(field);
            }
        }

        if sampled_rows >= sample_rows {
            break;
        }
    }

    // Full scan to get exact row count and actual max widths
    let row_count = scan_rows_and_widths(path, &mut col_infos, cancelled)?;

    let truncated_cols: Vec<String> = headers
        .iter()
        .zip(&col_infos)
        .filter(|(_, info)| !info.is_numeric && info.max_byte_len > MAX_STRING_WIDTH)
        .map(|(h, _)| h.clone())
        .collect();

    let col_types: Vec<ColType> = col_infos.iter().map(|c| c.col_type()).collect();

    Ok(CsvSchema {
        headers,
        col_types,
        file_size,
        row_count,
        truncated_cols,
    })
}
