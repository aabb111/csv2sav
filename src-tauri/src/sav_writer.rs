/// Minimal SPSS SAV writer with Simple (byte) compression.
///
/// Spec: https://www.gnu.org/software/pspp/pspp-dev/html_node/System-File-Format.html
use std::io::{self, Write};

/// Compression bias: integer values stored as (value + BIAS) in opcode byte.
const BIAS: f64 = 100.0;

const REC_TYPE_VARIABLE: i32 = 2;
const REC_TYPE_DICT_TERMINATOR: i32 = 999;

/// Opcode: raw 8-byte value follows in data stream.
const OP_RAW: u8 = 0;
/// Opcode: system-missing numeric value.
const OP_SYSMIS: u8 = 255;
/// Opcode: 8-byte block of spaces (for string padding).
const OP_SPACES: u8 = 254;

#[derive(Debug, Clone)]
pub enum ColType {
    Numeric,
    /// Width in bytes, 1–255.
    String(u16),
}

#[derive(Debug, Clone)]
pub struct ColDef {
    pub name: String,
    pub label: String,
    pub col_type: ColType,
}

#[derive(Debug, Clone)]
pub enum Value {
    Number(Option<f64>),
    String(Vec<u8>),
}

/// Number of 8-byte segments a column occupies in the data record.
fn col_segments(col_type: &ColType) -> usize {
    match col_type {
        ColType::Numeric => 1,
        ColType::String(w) => ((*w as usize) + 7) / 8,
    }
}

/// SAV writer with Simple compression.
///
/// Simple compression works by writing 8-opcode blocks followed by the raw
/// data only for opcodes that need it (OP_RAW). Each opcode describes one
/// 8-byte segment of the row.
pub struct SavWriter<W: Write> {
    writer: W,
    cols: Vec<ColDef>,
    /// Pending opcodes (up to 8 at a time).
    opcode_buf: [u8; 8],
    opcode_count: usize,
    /// Raw data blocks waiting to be flushed after the opcode block.
    raw_buf: Vec<u8>,
}

impl<W: Write> SavWriter<W> {
    pub fn new(mut writer: W, cols: Vec<ColDef>) -> io::Result<Self> {
        let row_width: usize = cols.iter().map(|c| col_segments(&c.col_type)).sum();
        write_header(&mut writer, row_width)?;
        write_variable_records(&mut writer, &cols)?;
        write_dict_terminator(&mut writer)?;
        Ok(Self {
            writer,
            cols,
            opcode_buf: [0u8; 8],
            opcode_count: 0,
            raw_buf: Vec::with_capacity(64),
        })
    }

    pub fn write_row(&mut self, values: &[Value]) -> io::Result<()> {
        // Collect column metadata first to avoid borrow conflicts.
        let col_info: Vec<ColType> = self.cols.iter().map(|c| c.col_type.clone()).collect();

        for (col_type, value) in col_info.iter().zip(values.iter()) {
            match (col_type, value) {
                (ColType::Numeric, Value::Number(opt)) => {
                    self.push_numeric(*opt)?;
                }
                (ColType::String(width), Value::String(bytes)) => {
                    let segments = col_segments(col_type);
                    let total = segments * 8;
                    let mut buf = vec![b' '; total];
                    let copy_len = bytes.len().min(*width as usize);
                    buf[..copy_len].copy_from_slice(&bytes[..copy_len]);
                    for chunk in buf.chunks(8) {
                        self.push_string_chunk(chunk)?;
                    }
                }
                _ => match col_type {
                    ColType::Numeric => self.push_numeric(None)?,
                    ColType::String(_) => {
                        let segments = col_segments(col_type);
                        for _ in 0..segments {
                            self.push_opcode(OP_SPACES)?;
                        }
                    }
                },
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<()> {
        // Flush any remaining opcodes.
        if self.opcode_count > 0 {
            self.flush_opcodes()?;
        }
        self.writer.flush()
    }

    fn push_numeric(&mut self, opt: Option<f64>) -> io::Result<()> {
        match opt {
            None => self.push_opcode(OP_SYSMIS),
            Some(n) => {
                // Try to encode as integer opcode (1..=251 = value + BIAS).
                let shifted = n + BIAS;
                if shifted >= 1.0
                    && shifted <= 251.0
                    && shifted.fract() == 0.0
                {
                    self.push_opcode(shifted as u8)
                } else {
                    // Must store raw 8-byte value.
                    self.push_raw(&n.to_le_bytes())
                }
            }
        }
    }

    fn push_string_chunk(&mut self, chunk: &[u8]) -> io::Result<()> {
        if chunk.iter().all(|&b| b == b' ') {
            self.push_opcode(OP_SPACES)
        } else {
            let mut block = [b' '; 8];
            block[..chunk.len()].copy_from_slice(chunk);
            self.push_raw(&block)
        }
    }

    fn push_opcode(&mut self, op: u8) -> io::Result<()> {
        self.opcode_buf[self.opcode_count] = op;
        self.opcode_count += 1;
        if self.opcode_count == 8 {
            self.flush_opcodes()?;
        }
        Ok(())
    }

    fn push_raw(&mut self, data: &[u8; 8]) -> io::Result<()> {
        self.opcode_buf[self.opcode_count] = OP_RAW;
        self.opcode_count += 1;
        self.raw_buf.extend_from_slice(data);
        if self.opcode_count == 8 {
            self.flush_opcodes()?;
        }
        Ok(())
    }

    fn flush_opcodes(&mut self) -> io::Result<()> {
        self.writer.write_all(&self.opcode_buf)?;
        if !self.raw_buf.is_empty() {
            self.writer.write_all(&self.raw_buf)?;
            self.raw_buf.clear();
        }
        self.opcode_buf = [0u8; 8];
        self.opcode_count = 0;
        Ok(())
    }
}

fn write_le_i32(w: &mut impl Write, v: i32) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

fn write_le_f64(w: &mut impl Write, v: f64) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

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
    w.write_all(b"$FL2")?;
    write_padded(w, "@(#) SPSS DATA FILE csv2sav", 60)?;
    write_le_i32(w, 2)?;                  // layout_code
    write_le_i32(w, row_width as i32)?;   // nominal_case_size
    write_le_i32(w, 1)?;                  // compression: 1 = simple
    write_le_i32(w, 0)?;                  // weight_index
    write_le_i32(w, -1)?;                 // ncases: unknown
    write_le_f64(w, BIAS)?;               // bias
    write_padded(w, "01 Jan 70", 9)?;     // creation_date
    write_padded(w, "00:00:00", 8)?;      // creation_time
    write_padded(w, "", 64)?;             // file_label
    w.write_all(&[0u8; 3])?;             // padding
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

        write_le_i32(w, REC_TYPE_VARIABLE)?;
        write_le_i32(w, type_code)?;
        write_le_i32(w, has_label)?;
        write_le_i32(w, 0)?; // n_missing_values
        let fmt = encode_format(&col.col_type);
        write_le_i32(w, fmt)?; // print_format
        write_le_i32(w, fmt)?; // write_format
        write_padded(w, &col.name, 8)?;

        if has_label == 1 {
            let label_bytes = col.label.as_bytes();
            let label_len = label_bytes.len().min(255);
            write_le_i32(w, label_len as i32)?;
            w.write_all(&label_bytes[..label_len])?;
            let pad = (4 - label_len % 4) % 4;
            w.write_all(&vec![0u8; pad])?;
        }

        // Continuation segments for long strings.
        for _ in 1..segments {
            write_le_i32(w, REC_TYPE_VARIABLE)?;
            write_le_i32(w, -1)?;
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

fn encode_format(col_type: &ColType) -> i32 {
    match col_type {
        ColType::Numeric => {
            let type_code: i32 = 5; // F
            let width: i32 = 8;
            let decimals: i32 = 2;
            (type_code << 16) | (width << 8) | decimals
        }
        ColType::String(w) => {
            let type_code: i32 = 1; // A
            let width: i32 = (*w as i32).min(255);
            (type_code << 16) | (width << 8)
        }
    }
}
