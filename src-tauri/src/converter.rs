use std::cell::Cell;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::readstat_writer::{ColDef, ColType, Value, Writer};
use crate::schema::{self, ColType as SchemaColType, CsvSchema};

const CSV_BUF_SIZE: usize = 512 * 1024;
const PROGRESS_INTERVAL: usize = 10_000;
const CANCEL_CHECK_INTERVAL: usize = 1_000;

fn truncate_utf8(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

struct CountingReader<R> {
    inner: R,
    bytes_read: Rc<Cell<u64>>,
}

impl<R: Read> CountingReader<R> {
    fn new(inner: R) -> (Self, Rc<Cell<u64>>) {
        let counter = Rc::new(Cell::new(0u64));
        let reader = Self {
            inner,
            bytes_read: counter.clone(),
        };
        (reader, counter)
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read.set(self.bytes_read.get() + n as u64);
        Ok(n)
    }
}

fn make_col_defs(schema: &CsvSchema) -> Vec<ColDef> {
    schema
        .headers
        .iter()
        .zip(&schema.col_types)
        .enumerate()
        .map(|(i, (header, col_type))| {
            let name = format!("V{}", i + 1);
            let sav_type = match col_type {
                SchemaColType::Numeric => ColType::Numeric,
                SchemaColType::String(w) => ColType::String(*w),
            };
            ColDef {
                name,
                label: header.clone(),
                col_type: sav_type,
            }
        })
        .collect()
}

/// Converts CSV to ZSAV using two passes:
/// 1. Count rows via CSV parser (handles quoted multi-line fields).
/// 2. Stream rows into ZSAV writer with exact row count.
pub fn convert_csv_to_zsav(
    input: &Path,
    output: &Path,
    csv_schema: &CsvSchema,
    cancelled: &AtomicBool,
    on_progress: &dyn Fn(usize, u64, u64),
) -> Result<usize, String> {
    let total_rows = schema::count_rows(input, cancelled)?;

    if cancelled.load(Ordering::Relaxed) {
        return Err("Cancelled".to_string());
    }

    let col_defs = make_col_defs(csv_schema);
    let out_file =
        File::create(output).map_err(|e| format!("Failed to create ZSAV file: {e}"))?;
    let mut writer = Writer::new_zsav(out_file, &col_defs, total_rows)
        .map_err(|e| format!("Failed to init writer: {e}"))?;

    let csv_file =
        File::open(input).map_err(|e| format!("Failed to open CSV for conversion: {e}"))?;
    let (counting, bytes_counter) = CountingReader::new(csv_file);
    let csv_buf = BufReader::with_capacity(CSV_BUF_SIZE, counting);
    let mut reader = csv::Reader::from_reader(csv_buf);

    let col_types = &csv_schema.col_types;
    let col_count = col_types.len();
    let mut row_count = 0usize;
    let mut string_buf: Vec<String> = vec![String::new(); col_count];

    for result in reader.records() {
        let record =
            result.map_err(|e| format!("CSV read error at row {}: {e}", row_count + 1))?;
        row_count += 1;

        if row_count % CANCEL_CHECK_INTERVAL == 0 && cancelled.load(Ordering::Relaxed) {
            drop(writer);
            let _ = std::fs::remove_file(output);
            return Err("Cancelled".to_string());
        }

        for i in 0..col_count {
            let field = record.get(i).unwrap_or("").trim();
            string_buf[i].clear();
            match &col_types[i] {
                SchemaColType::String(max_width) => {
                    string_buf[i].push_str(truncate_utf8(field, *max_width));
                }
                _ => {
                    string_buf[i].push_str(field);
                }
            }
        }

        let row_values: Vec<Value<'_>> = col_types
            .iter()
            .enumerate()
            .map(|(i, col_type)| {
                let field = string_buf[i].as_str();
                match col_type {
                    SchemaColType::Numeric => {
                        if field.is_empty() {
                            Value::Number(None)
                        } else {
                            match field.parse::<f64>() {
                                Ok(n) => Value::Number(Some(n)),
                                Err(_) => Value::Number(None),
                            }
                        }
                    }
                    SchemaColType::String(_) => Value::Str(field),
                }
            })
            .collect();

        writer
            .write_row(&row_values)
            .map_err(|e| format!("Failed to write row {}: {e}", row_count))?;

        if row_count % PROGRESS_INTERVAL == 0 {
            on_progress(row_count, bytes_counter.get(), csv_schema.file_size);
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize ZSAV file: {e}"))?;

    Ok(row_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn test_zsav_magic_bytes() {
        let input = Path::new("../testFiles/pc.csv");
        if !input.exists() {
            return;
        }
        let output = std::env::temp_dir().join("csv2sav_test_output.zsav");
        let cancelled = AtomicBool::new(false);

        let schema = crate::schema::infer_schema(input, 10_000, &cancelled).unwrap();
        convert_csv_to_zsav(input, &output, &schema, &cancelled, &|_, _, _| {}).unwrap();

        let data = std::fs::read(&output).unwrap();
        let magic = &data[..4];
        assert_eq!(magic, b"$FL3", "ZSAV file must start with $FL3, got {:?}", std::str::from_utf8(magic));

        std::fs::remove_file(&output).ok();
    }

    #[test]
    fn generate_zsav_for_validation() {
        let input = Path::new("../testFiles/pc.csv");
        if !input.exists() {
            return;
        }
        let output = std::path::PathBuf::from("/tmp/validate_output.zsav");
        let cancelled = AtomicBool::new(false);
        let schema = crate::schema::infer_schema(input, 10_000, &cancelled).unwrap();
        convert_csv_to_zsav(input, &output, &schema, &cancelled, &|_, _, _| {}).unwrap();
        println!("Generated ZSAV at /tmp/validate_output.zsav");
    }
}
