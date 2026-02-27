use std::cell::Cell;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::sav_writer::{sav_to_zsav, ColDef, ColType, SavWriter, Value};
use crate::schema::{ColType as SchemaColType, CsvSchema};

const CSV_BUF_SIZE: usize = 512 * 1024;
const PROGRESS_INTERVAL: usize = 10_000;
const CANCEL_CHECK_INTERVAL: usize = 1_000;

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

/// on_progress(current_rows, bytes_read, file_size)
///
/// Writes a temporary `.sav` file, then converts it to `.zsav` (zlib-compressed).
/// The temporary `.sav` file is deleted after successful conversion.
pub fn convert_csv_to_sav(
    input: &Path,
    output: &Path,
    schema: &CsvSchema,
    cancelled: &AtomicBool,
    on_progress: &dyn Fn(usize, u64, u64),
) -> Result<usize, String> {
    let col_defs = make_col_defs(schema);

    // Write to a temporary .sav file first, then compress to .zsav.
    let tmp_sav = output.with_extension("sav.tmp");

    let out_file =
        File::create(&tmp_sav).map_err(|e| format!("Failed to create temp SAV file: {e}"))?;
    let buf_writer = BufWriter::new(out_file);
    let mut writer = SavWriter::new(buf_writer, col_defs)
        .map_err(|e| format!("Failed to write SAV header: {e}"))?;

    let csv_file =
        File::open(input).map_err(|e| format!("Failed to open CSV for conversion: {e}"))?;
    let (counting, bytes_counter) = CountingReader::new(csv_file);
    let csv_buf = BufReader::with_capacity(CSV_BUF_SIZE, counting);
    let mut reader = csv::Reader::from_reader(csv_buf);

    let col_types = &schema.col_types;
    let col_count = col_types.len();
    let mut row_values: Vec<Value> = Vec::with_capacity(col_count);
    let mut row_count = 0usize;

    for result in reader.records() {
        let record =
            result.map_err(|e| format!("CSV read error at row {}: {e}", row_count + 1))?;
        row_count += 1;

        if row_count % CANCEL_CHECK_INTERVAL == 0 && cancelled.load(Ordering::Relaxed) {
            drop(writer);
            let _ = std::fs::remove_file(&tmp_sav);
            let _ = std::fs::remove_file(output);
            return Err("Cancelled".to_string());
        }

        row_values.clear();
        for (i, col_type) in col_types.iter().enumerate() {
            let field = record.get(i).unwrap_or("").trim();
            let value = match col_type {
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
                SchemaColType::String(_) => Value::String(field.as_bytes().to_vec()),
            };
            row_values.push(value);
        }

        writer
            .write_row(&row_values)
            .map_err(|e| format!("Failed to write row {}: {e}", row_count))?;

        if row_count % PROGRESS_INTERVAL == 0 {
            on_progress(row_count, bytes_counter.get(), schema.file_size);
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize SAV file: {e}"))?;

    // Compress the temp SAV to the final ZSAV output path.
    sav_to_zsav(&tmp_sav, output)
        .map_err(|e| format!("Failed to compress SAV to ZSAV: {e}"))?;

    // Clean up the temporary SAV file.
    let _ = std::fs::remove_file(&tmp_sav);

    Ok(row_count)
}
