use std::cell::Cell;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};

use encoding_rs::UTF_8;
use pspp::data::{ByteString, Datum};
use pspp::dictionary::Dictionary;
use pspp::identifier::Identifier;
use pspp::sys::WriteOptions;
use pspp::sys::raw::records::Compression;
use pspp::variable::{VarWidth, Variable};

use crate::schema::{ColType, CsvSchema};

const CSV_BUF_SIZE: usize = 512 * 1024;
const PROGRESS_INTERVAL: usize = 10_000;
const CANCEL_CHECK_INTERVAL: usize = 10_000;

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

fn build_dictionary(schema: &CsvSchema) -> Result<Dictionary, String> {
    let mut dict = Dictionary::new(UTF_8);

    for (i, (header, col_type)) in schema.headers.iter().zip(&schema.col_types).enumerate() {
        let var_name = format!("V{}", i + 1);
        let width = match col_type {
            ColType::Numeric => VarWidth::Numeric,
            ColType::String(len) => VarWidth::String(*len),
        };

        let id = Identifier::new(var_name.clone())
            .map_err(|e| format!("Invalid variable name {var_name}: {e}"))?;

        let mut var = Variable::new(id, width, UTF_8);
        var.label = Some(header.clone());

        dict.add_var(var)
            .map_err(|e| format!("Failed to add variable {var_name}: {e:?}"))?;
    }

    Ok(dict)
}

/// on_progress(current_rows, bytes_read, file_size)
pub fn convert_csv_to_sav(
    input: &Path,
    output: &Path,
    schema: &CsvSchema,
    use_zlib: bool,
    cancelled: &AtomicBool,
    on_progress: &dyn Fn(usize, u64, u64),
) -> Result<usize, String> {
    let dict = build_dictionary(schema)?;

    let compression = if use_zlib {
        Some(Compression::ZLib)
    } else {
        Some(Compression::Simple)
    };

    let mut writer = WriteOptions::new()
        .with_compression(compression)
        .write_file(&dict, output)
        .map_err(|e| format!("Failed to create SAV file: {e}"))?;

    let csv_file =
        File::open(input).map_err(|e| format!("Failed to open CSV for conversion: {e}"))?;
    let (counting, bytes_counter) = CountingReader::new(csv_file);
    let csv_buf = BufReader::with_capacity(CSV_BUF_SIZE, counting);
    let mut reader = csv::Reader::from_reader(csv_buf);

    let col_types = &schema.col_types;
    let col_count = col_types.len();

    let mut datums: Vec<Datum<ByteString>> = Vec::with_capacity(col_count);
    let mut pad_bufs: Vec<Vec<u8>> = col_types
        .iter()
        .map(|ct| match ct {
            ColType::Numeric => Vec::new(),
            ColType::String(max_len) => vec![b' '; *max_len as usize],
        })
        .collect();

    let mut row_count = 0usize;

    for result in reader.records() {
        if row_count % CANCEL_CHECK_INTERVAL == 0 && cancelled.load(Ordering::Relaxed) {
            drop(writer);
            let _ = std::fs::remove_file(output);
            return Err("Cancelled".to_string());
        }

        let record =
            result.map_err(|e| format!("CSV read error at row {}: {e}", row_count + 1))?;
        row_count += 1;

        datums.clear();
        for (i, col_type) in col_types.iter().enumerate() {
            let field = record.get(i).unwrap_or("");
            let trimmed = field.trim();

            let datum = match col_type {
                ColType::Numeric => {
                    if trimmed.is_empty() {
                        Datum::Number(None)
                    } else {
                        match trimmed.parse::<f64>() {
                            Ok(n) => Datum::Number(Some(n)),
                            Err(_) => Datum::Number(None),
                        }
                    }
                }
                ColType::String(max_len) => {
                    let target_len = *max_len as usize;
                    let buf = &mut pad_bufs[i];
                    buf.fill(b' ');
                    let bytes = trimmed.as_bytes();
                    let copy_len = bytes.len().min(target_len);
                    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    Datum::String(ByteString(buf.clone()))
                }
            };
            datums.push(datum);
        }

        writer
            .write_case(datums.drain(..))
            .map_err(|e| format!("Failed to write case {}: {e}", row_count))?;

        if row_count % PROGRESS_INTERVAL == 0 {
            on_progress(row_count, bytes_counter.get(), schema.file_size);
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Failed to finalize SAV file: {e}"))?;

    Ok(row_count)
}
