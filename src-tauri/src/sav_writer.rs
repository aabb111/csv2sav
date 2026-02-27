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

#[cfg(test)]
mod tests {
    use super::*;

    // ── helpers ──────────────────────────────────────────────────────────────

    fn num_col(name: &str, label: &str) -> ColDef {
        ColDef { name: name.into(), label: label.into(), col_type: ColType::Numeric }
    }

    fn str_col(name: &str, label: &str, width: u16) -> ColDef {
        ColDef { name: name.into(), label: label.into(), col_type: ColType::String(width) }
    }

    fn build(cols: Vec<ColDef>, rows: Vec<Vec<Value>>) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut w = SavWriter::new(&mut buf, cols).unwrap();
        for row in rows {
            w.write_row(&row).unwrap();
        }
        w.finish().unwrap();
        buf
    }

    // Parse a little-endian i32 from a byte slice at the given offset.
    fn read_i32(buf: &[u8], offset: usize) -> i32 {
        i32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap())
    }

    fn read_f64(buf: &[u8], offset: usize) -> f64 {
        f64::from_le_bytes(buf[offset..offset + 8].try_into().unwrap())
    }

    // ── header ───────────────────────────────────────────────────────────────

    #[test]
    fn header_magic_bytes() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        assert_eq!(&buf[0..4], b"$FL2");
    }

    #[test]
    fn header_layout_code_is_2() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        // layout_code is at offset 64 (4 magic + 60 prod_name)
        assert_eq!(read_i32(&buf, 64), 2);
    }

    #[test]
    fn header_compression_is_1() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        // compression at offset 72 (64 + 4 layout + 4 case_size)
        assert_eq!(read_i32(&buf, 72), 1);
    }

    #[test]
    fn header_bias_is_100() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        // bias at offset 84 (72 + 4 compression + 4 weight + 4 ncases)
        assert_eq!(read_f64(&buf, 84), 100.0);
    }

    #[test]
    fn header_nominal_case_size_numeric() {
        // 2 numeric cols → row_width = 2
        let buf = build(vec![num_col("V1", ""), num_col("V2", "")], vec![]);
        assert_eq!(read_i32(&buf, 68), 2);
    }

    #[test]
    fn header_nominal_case_size_string() {
        // String(8) → 1 segment, String(9) → 2 segments → row_width = 3
        let buf = build(
            vec![str_col("V1", "", 8), str_col("V2", "", 9)],
            vec![],
        );
        assert_eq!(read_i32(&buf, 68), 3);
    }

    // ── col_segments ─────────────────────────────────────────────────────────

    #[test]
    fn col_segments_numeric_is_1() {
        assert_eq!(col_segments(&ColType::Numeric), 1);
    }

    #[test]
    fn col_segments_string_rounds_up() {
        assert_eq!(col_segments(&ColType::String(1)), 1);
        assert_eq!(col_segments(&ColType::String(8)), 1);
        assert_eq!(col_segments(&ColType::String(9)), 2);
        assert_eq!(col_segments(&ColType::String(16)), 2);
        assert_eq!(col_segments(&ColType::String(17)), 3);
    }

    // ── compression opcodes ──────────────────────────────────────────────────

    /// Find the start of the data section (after dict terminator record).
    /// Dict terminator = rec_type 999 (i32) + filler 0 (i32) = 8 bytes.
    fn data_offset(buf: &[u8]) -> usize {
        // Scan for the 999 record type marker.
        let mut i = 0;
        while i + 4 <= buf.len() {
            if read_i32(buf, i) == 999 {
                return i + 8; // skip rec_type + filler
            }
            i += 1;
        }
        panic!("dict terminator not found");
    }

    #[test]
    fn integer_in_range_uses_opcode() {
        // Value 0.0 → shifted = 100 → opcode 100
        let buf = build(
            vec![num_col("V1", "")],
            vec![vec![Value::Number(Some(0.0))]],
        );
        let d = data_offset(&buf);
        // First opcode block: 8 bytes of opcodes, first opcode = 100
        assert_eq!(buf[d], 100);
    }

    #[test]
    fn missing_numeric_uses_sysmis_opcode() {
        let buf = build(
            vec![num_col("V1", "")],
            vec![vec![Value::Number(None)]],
        );
        let d = data_offset(&buf);
        assert_eq!(buf[d], 255); // OP_SYSMIS
    }

    #[test]
    fn float_out_of_range_uses_raw_opcode() {
        // 1.5 is not an integer → must use OP_RAW (0)
        let buf = build(
            vec![num_col("V1", "")],
            vec![vec![Value::Number(Some(1.5))]],
        );
        let d = data_offset(&buf);
        assert_eq!(buf[d], 0); // OP_RAW
        // Raw value follows after the 8-opcode block
        let raw_val = read_f64(&buf, d + 8);
        assert_eq!(raw_val, 1.5);
    }

    #[test]
    fn all_spaces_string_uses_spaces_opcode() {
        let buf = build(
            vec![str_col("V1", "", 8)],
            vec![vec![Value::String(b"        ".to_vec())]],
        );
        let d = data_offset(&buf);
        assert_eq!(buf[d], 254); // OP_SPACES
    }

    #[test]
    fn non_space_string_uses_raw_opcode() {
        let buf = build(
            vec![str_col("V1", "", 8)],
            vec![vec![Value::String(b"hello".to_vec())]],
        );
        let d = data_offset(&buf);
        assert_eq!(buf[d], 0); // OP_RAW
        // Raw 8-byte block: "hello   "
        assert_eq!(&buf[d + 8..d + 16], b"hello   ");
    }

    #[test]
    fn empty_string_uses_spaces_opcode() {
        let buf = build(
            vec![str_col("V1", "", 8)],
            vec![vec![Value::String(vec![])]],
        );
        let d = data_offset(&buf);
        assert_eq!(buf[d], 254); // OP_SPACES — empty string = all spaces
    }

    // ── opcode block flushing ─────────────────────────────────────────────────

    #[test]
    fn opcode_block_is_8_bytes() {
        // 8 numeric columns, all integer values → one full opcode block, no raw data
        let cols: Vec<ColDef> = (1..=8).map(|i| num_col(&format!("V{i}"), "")).collect();
        let row: Vec<Value> = (1..=8).map(|_| Value::Number(Some(1.0))).collect();
        let buf = build(cols, vec![row]);
        let d = data_offset(&buf);
        // Exactly 8 opcode bytes, no raw data after
        assert_eq!(buf.len() - d, 8);
        // All opcodes = 101 (1.0 + 100)
        assert!(buf[d..d + 8].iter().all(|&b| b == 101));
    }

    #[test]
    fn nine_cols_produce_two_opcode_blocks() {
        let cols: Vec<ColDef> = (1..=9).map(|i| num_col(&format!("V{i}"), "")).collect();
        let row: Vec<Value> = (1..=9).map(|_| Value::Number(Some(2.0))).collect();
        let buf = build(cols, vec![row]);
        let d = data_offset(&buf);
        // First block: 8 opcodes = 102 each
        assert!(buf[d..d + 8].iter().all(|&b| b == 102));
        // Second block: 8 bytes, first opcode = 102, rest = 0 (unused)
        assert_eq!(buf[d + 8], 102);
    }

    // ── variable records ─────────────────────────────────────────────────────

    #[test]
    fn variable_record_type_is_2() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        // Variable record starts right after the 176-byte header.
        assert_eq!(read_i32(&buf, 176), 2);
    }

    #[test]
    fn numeric_col_type_code_is_0() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        assert_eq!(read_i32(&buf, 180), 0);
    }

    #[test]
    fn string_col_type_code_equals_width() {
        let buf = build(vec![str_col("V1", "", 10)], vec![]);
        assert_eq!(read_i32(&buf, 180), 10);
    }

    #[test]
    fn label_written_when_present() {
        let buf = build(vec![num_col("V1", "My Label")], vec![]);
        // has_label = 1 at offset 184
        assert_eq!(read_i32(&buf, 184), 1);
        // label_len at offset 208 (after 6×i32 + 8-byte name)
        let label_len = read_i32(&buf, 208) as usize;
        assert_eq!(label_len, 8);
        assert_eq!(&buf[212..212 + 8], b"My Label");
    }

    #[test]
    fn no_label_when_empty() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        assert_eq!(read_i32(&buf, 184), 0); // has_label = 0
    }

    // ── dict terminator ───────────────────────────────────────────────────────

    #[test]
    fn dict_terminator_record_type_is_999() {
        let buf = build(vec![num_col("V1", "")], vec![]);
        let d = data_offset(&buf);
        // data_offset points past the terminator; go back 8 bytes
        assert_eq!(read_i32(&buf, d - 8), 999);
    }

    // ── multiple rows ─────────────────────────────────────────────────────────

    #[test]
    fn two_rows_produce_correct_opcodes() {
        // 1 numeric col, 2 rows → 2 values → fits in one 8-opcode block
        // opcode block: [101, 102, 0, 0, 0, 0, 0, 0]
        let buf = build(
            vec![num_col("V1", "")],
            vec![
                vec![Value::Number(Some(1.0))],
                vec![Value::Number(Some(2.0))],
            ],
        );
        let d = data_offset(&buf);
        // Both values fit in the same 8-opcode block
        assert_eq!(buf[d], 101); // 1.0 + 100
        assert_eq!(buf[d + 1], 102); // 2.0 + 100
    }
}
