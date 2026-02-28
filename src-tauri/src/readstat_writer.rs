use std::ffi::CString;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::os::raw::{c_long, c_void};

use crate::readstat_sys::*;

#[derive(Debug, Clone)]
pub enum ColType {
    Numeric,
    String(usize),
}

#[derive(Debug, Clone)]
pub struct ColDef {
    pub name: String,
    pub label: String,
    pub col_type: ColType,
}

#[derive(Debug)]
pub enum Value<'a> {
    Number(Option<f64>),
    Str(&'a str),
}

struct WriterCtx {
    output: BufWriter<File>,
    error: Option<String>,
}

unsafe extern "C" fn data_writer_callback(
    data: *const c_void,
    len: usize,
    ctx: *mut c_void,
) -> isize {
    let wctx = unsafe { &mut *(ctx as *mut WriterCtx) };
    let slice = unsafe { std::slice::from_raw_parts(data as *const u8, len) };
    match wctx.output.write_all(slice) {
        Ok(()) => len as isize,
        Err(e) => {
            wctx.error = Some(e.to_string());
            -1
        }
    }
}

fn check(err: readstat_error_t) -> Result<(), String> {
    if err == readstat_error_t::READSTAT_OK {
        return Ok(());
    }
    let msg = unsafe {
        let ptr = readstat_error_message(err);
        if ptr.is_null() {
            format!("ReadStat error: {:?}", err)
        } else {
            std::ffi::CStr::from_ptr(ptr)
                .to_string_lossy()
                .into_owned()
        }
    };
    Err(msg)
}

pub struct Writer {
    writer: *mut readstat_writer_t,
    ctx: *mut WriterCtx,
    var_count: usize,
    finished: bool,
}

fn init_writer(
    output_file: File,
    cols: &[ColDef],
    compression: readstat_compress_t,
    row_count: c_long,
) -> Result<Writer, String> {
    let ctx = Box::into_raw(Box::new(WriterCtx {
        output: BufWriter::with_capacity(512 * 1024, output_file),
        error: None,
    }));

    let writer = unsafe { readstat_writer_init() };
    if writer.is_null() {
        unsafe { drop(Box::from_raw(ctx)) };
        return Err("Failed to initialize ReadStat writer".to_string());
    }

    unsafe {
        check(readstat_set_data_writer(writer, Some(data_writer_callback)))?;
        check(readstat_writer_set_compression(writer, compression))?;
        if compression == readstat_compress_t::READSTAT_COMPRESS_BINARY {
            check(readstat_writer_set_file_format_version(writer, 3))?;
        }
    }

    for col in cols {
        let c_name = CString::new(col.name.as_str())
            .map_err(|_| format!("Invalid variable name: {}", col.name))?;

        let (var_type, width) = match &col.col_type {
            ColType::Numeric => (readstat_type_t::READSTAT_TYPE_DOUBLE, 0),
            ColType::String(w) => (readstat_type_t::READSTAT_TYPE_STRING, *w),
        };

        let var = unsafe { readstat_add_variable(writer, c_name.as_ptr(), var_type, width) };
        if var.is_null() {
            unsafe {
                readstat_writer_free(writer);
                drop(Box::from_raw(ctx));
            }
            return Err(format!("Failed to add variable: {}", col.name));
        }

        let c_label = CString::new(col.label.as_str()).unwrap_or_default();
        unsafe { readstat_variable_set_label(var, c_label.as_ptr()) };

        match &col.col_type {
            ColType::Numeric => {
                let c_fmt = CString::new("F8.2").unwrap();
                unsafe {
                    readstat_variable_set_format(var, c_fmt.as_ptr());
                    readstat_variable_set_measure(var, readstat_measure_t::READSTAT_MEASURE_SCALE);
                    readstat_variable_set_alignment(
                        var,
                        readstat_alignment_t::READSTAT_ALIGNMENT_RIGHT,
                    );
                }
            }
            ColType::String(w) => {
                let fmt = format!("A{}", w);
                let c_fmt = CString::new(fmt).unwrap();
                unsafe {
                    readstat_variable_set_format(var, c_fmt.as_ptr());
                    readstat_variable_set_measure(
                        var,
                        readstat_measure_t::READSTAT_MEASURE_NOMINAL,
                    );
                    readstat_variable_set_alignment(
                        var,
                        readstat_alignment_t::READSTAT_ALIGNMENT_LEFT,
                    );
                }
            }
        }
    }

    unsafe {
        check(readstat_begin_writing_sav(writer, ctx as *mut c_void, row_count))?;
    }

    Ok(Writer {
        writer,
        ctx,
        var_count: cols.len(),
        finished: false,
    })
}

impl Writer {
    /// ZSAV with zlib compression. Requires exact row_count upfront.
    pub fn new_zsav(output_file: File, cols: &[ColDef], row_count: usize) -> Result<Self, String> {
        init_writer(
            output_file,
            cols,
            readstat_compress_t::READSTAT_COMPRESS_BINARY,
            row_count as c_long,
        )
    }

    pub fn write_row(&mut self, values: &[Value<'_>]) -> Result<(), String> {
        if values.len() != self.var_count {
            return Err(format!(
                "Expected {} values, got {}",
                self.var_count,
                values.len()
            ));
        }

        unsafe { check(readstat_begin_row(self.writer))? };

        for (i, val) in values.iter().enumerate() {
            let var = unsafe { readstat_get_variable(self.writer, i as std::os::raw::c_int) };
            match val {
                Value::Number(None) => {
                    unsafe { check(readstat_insert_missing_value(self.writer, var))? };
                }
                Value::Number(Some(n)) => {
                    unsafe { check(readstat_insert_double_value(self.writer, var, *n))? };
                }
                Value::Str(s) => {
                    if s.is_empty() {
                        unsafe { check(readstat_insert_missing_value(self.writer, var))? };
                    } else {
                        let c_str = CString::new(*s).unwrap_or_default();
                        unsafe {
                            check(readstat_insert_string_value(
                                self.writer,
                                var,
                                c_str.as_ptr(),
                            ))?
                        };
                    }
                }
            }
        }

        unsafe { check(readstat_end_row(self.writer))? };

        let wctx = unsafe { &*self.ctx };
        if let Some(ref e) = wctx.error {
            return Err(format!("I/O error: {}", e));
        }

        Ok(())
    }

    pub fn finish(mut self) -> Result<(), String> {
        self.finished = true;
        unsafe { check(readstat_end_writing(self.writer))? };

        let wctx = unsafe { &mut *self.ctx };
        wctx.output
            .flush()
            .map_err(|e| format!("Failed to flush output: {e}"))?;

        if let Some(ref e) = wctx.error {
            return Err(format!("I/O error: {}", e));
        }
        Ok(())
    }
}

impl Drop for Writer {
    fn drop(&mut self) {
        if !self.finished {
            unsafe {
                let _ = readstat_end_writing(self.writer);
            }
        }
        unsafe {
            readstat_writer_free(self.writer);
            drop(Box::from_raw(self.ctx));
        }
    }
}

unsafe impl Send for Writer {}
