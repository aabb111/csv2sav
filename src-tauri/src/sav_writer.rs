/// Minimal SPSS SAV writer.
///
/// Spec reference: https://www.gnu.org/software/pspp/pspp-dev/html_node/System-File-Format.html
use std::io::{self, Write};

const SYSMIS: f64 = -f64::MAX; // SPSS system-missing value
const REC_TYPE_VARIABLE: i32 = 2;
const REC_TYPE_DICT_TERMINATOR: i32 = 999;

/// Column type for SAV output.
#[derive(Debug, Clone)]
pub enum ColType {
    Numeric,
    /// Width in bytes, 1–255.
    String(u16),
}

/// A column definition.
#[derive(Debug, Clone)]
pub struct ColDef {
    pub name: String,
    pub label: String,
    pub col_type: ColType,
}

/// A single data value.
#[derive(Debug, Clone)]
pub enum Value {
    Number(Option<f64>),
    String(Vec<u8>),
}

pub struct SavWriter<W: Write> {
    writer: W,
    cols: Vec<ColDef>,
}

impl<W: Write> SavWriter<W> {
    pub fn new(mut writer: W, cols: Vec<ColDef>) -> io::Result<Self> {
        let row_width: usize = cols.iter().map(|c| col_segments(&c.col_type)).sum();
        write_header(&mut writer, row_width)?;
        write_variable_records(&mut writer, &cols)?;
        write_dict_terminator(&mut writer)?;
        Ok(Self { writer, cols })
    }

    pub fn write_row(&mut self, values: &[Value]) -> io::Result<()> {
        for (col, value) in self.cols.iter().zip(values.iter()) {
            match (&col.col_type, value) {
                (ColType::Numeric, Value::Number(Some(n))) => {
                    self.writer.write_all(&n.to_le_bytes())?;
                }
                (ColType::Numeric, Value::Number(None)) => {
                    self.writer.write_all(&SYSMIS.to_le_bytes())?;
                }
                (ColType::String(width), Value::String(bytes)) => {
                    let segments = col_segments(&col.col_type);
                    let total = segments * 8;
                    let mut buf = vec![b' '; total];
                    let copy_len = bytes.len().min(*width as usize);
                    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    self.writer.write_all(&buf)?;
                }
                _ => {
                    match &col.col_type {
                        ColType::Numeric => {
                            self.writer.write_all(&SYSMIS.to_le_bytes())?;
                        }
                        ColType::String(_) => {
                            let segments = col_segments(&col.col_type);
                            let buf = vec![b' '; segments * 8];
                            self.writer.write_all(&buf)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

/// Number of 8-byte segments a column occupies in the data record.
fn col_segments(col_type: &ColType) -> usize {
    match col_type {
        ColType::Numeric => 1,
        ColType::String(w) => ((*w as usize) + 7) / 8,
    }
}

fn write_le_i32(w: &mut impl Write, v: i32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_le_f64(w: &mut impl Write, v: f64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

/// Write a fixed-length ASCII field, padded with spaces.
fn write_padded(w: &mut impl Write, s: &str, len: usize) -> io::Result<()> {
    let bytes = s.as_bytes();
    let copy = bytes.len().min(len);
    w.write_all(&bytes[..copy])?;
    for _ in 0..(len - copy) {
        w.write_all(b" ")?;
    }
    Ok(())
}

fn write_header(w: &mut impl Write, row_width: usize) -> io::Result<()> {
    // rec_type: "$FL2" (4 bytes)
    w.write_all(b"$FL2")?;
    // prod_name: 60 bytes
    write_padded(w, "@(#) SPSS DATA FILE csv2sav", 60)?;
    // layout_code: 2 (little-endian)
    write_le_i32(w, 2)?;
    // nominal_case_size
    write_le_i32(w, row_width as i32)?;
    // compression: 0 = none
    write_le_i32(w, 0)?;
    // weight_index: 0 = no weight
    write_le_i32(w, 0)?;
    // ncases: -1 = unknown
    write_le_i32(w, -1)?;
    // bias: 100.0
    write_le_f64(w, 100.0)?;
    // creation_date: 9 bytes "DD MMM YY"
    write_padded(w, "01 Jan 70", 9)?;
    // creation_time: 8 bytes "HH:MM:SS"
    write_padded(w, "00:00:00", 8)?;
    // file_label: 64 bytes
    write_padded(w, "", 64)?;
    // padding: 3 bytes
    w.write_all(&[0u8; 3])?;
    Ok(())
}

fn write_variable_records(w: &mut impl Write, cols: &[ColDef]) -> io::Result<()> {
    for col in cols {
        let segments = col_segments(&col.col_type);
        let type_code: i32 = match &col.col_type {
            ColType::Numeric => 0,
            ColType::String(width) => *width as i32,
        };
        let has_label: i32 = if col.label.is_empty() { 0 } else { 1 };

        // First segment (or only segment for numeric)
        write_le_i32(w, REC_TYPE_VARIABLE)?;
        write_le_i32(w, type_code)?;
        write_le_i32(w, has_label)?;
        write_le_i32(w, 0)?; // n_missing_values
        // print/write format: F8.2 for numeric, A<width> for string
        let fmt = encode_format(&col.col_type);
        write_le_i32(w, fmt)?; // print_format
        write_le_i32(w, fmt)?; // write_format
        // name: 8 bytes
        write_padded(w, &col.name, 8)?;

        if has_label == 1 {
            let label_bytes = col.label.as_bytes();
            let label_len = label_bytes.len().min(255);
            write_le_i32(w, label_len as i32)?;
            w.write_all(&label_bytes[..label_len])?;
            // pad to multiple of 4
            let pad = (4 - label_len % 4) % 4;
            w.write_all(&vec![0u8; pad])?;
        }

        // Continuation segments for long strings
        for _ in 1..segments {
            write_le_i32(w, REC_TYPE_VARIABLE)?;
            write_le_i32(w, -1)?; // continuation marker
            write_le_i32(w, 0)?;
            write_le_i32(w, 0)?;
            write_le_i32(w, 0)?;
            write_le_i32(w, 0)?;
            write_padded(w, "        ", 8)?;
        }
    }
    Ok(())
}

fn write_dict_terminator(w: &mut impl Write) -> io::Result<()> {
    write_le_i32(w, REC_TYPE_DICT_TERMINATOR)?;
    write_le_i32(w, 0)?;
    Ok(())
}

/// Encode SPSS variable format as a 4-byte integer.
/// Byte layout (little-endian): [decimals, width, type, 0]
fn encode_format(col_type: &ColType) -> i32 {
    match col_type {
        ColType::Numeric => {
            // type=5 (F), width=8, decimals=2
            let type_code: i32 = 5;
            let width: i32 = 8;
            let decimals: i32 = 2;
            (type_code << 16) | (width << 8) | decimals
        }
        ColType::String(w) => {
            // type=1 (A), width=w, decimals=0
            let type_code: i32 = 1;
            let width: i32 = (*w as i32).min(255);
            (type_code << 16) | (width << 8)
        }
    }
}
