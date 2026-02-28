#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_char, c_int, c_void};

pub type readstat_off_t = i64;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum readstat_type_t {
    READSTAT_TYPE_STRING = 0,
    READSTAT_TYPE_INT8 = 1,
    READSTAT_TYPE_INT16 = 2,
    READSTAT_TYPE_INT32 = 3,
    READSTAT_TYPE_FLOAT = 4,
    READSTAT_TYPE_DOUBLE = 5,
    READSTAT_TYPE_STRING_REF = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum readstat_compress_t {
    READSTAT_COMPRESS_NONE = 0,
    READSTAT_COMPRESS_ROWS = 1,
    READSTAT_COMPRESS_BINARY = 2,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum readstat_measure_t {
    READSTAT_MEASURE_UNKNOWN = 0,
    READSTAT_MEASURE_NOMINAL = 1,
    READSTAT_MEASURE_ORDINAL = 2,
    READSTAT_MEASURE_SCALE = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum readstat_alignment_t {
    READSTAT_ALIGNMENT_UNKNOWN = 0,
    READSTAT_ALIGNMENT_LEFT = 1,
    READSTAT_ALIGNMENT_CENTER = 2,
    READSTAT_ALIGNMENT_RIGHT = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum readstat_error_t {
    READSTAT_OK = 0,
    READSTAT_ERROR_OPEN = 1,
    READSTAT_ERROR_READ = 2,
    READSTAT_ERROR_MALLOC = 3,
    READSTAT_ERROR_USER_ABORT = 4,
    READSTAT_ERROR_PARSE = 5,
    READSTAT_ERROR_UNSUPPORTED_COMPRESSION = 6,
    READSTAT_ERROR_UNSUPPORTED_CHARSET = 7,
    READSTAT_ERROR_COLUMN_COUNT_MISMATCH = 8,
    READSTAT_ERROR_ROW_COUNT_MISMATCH = 9,
    READSTAT_ERROR_ROW_WIDTH_MISMATCH = 10,
    READSTAT_ERROR_BAD_FORMAT_STRING = 11,
    READSTAT_ERROR_VALUE_TYPE_MISMATCH = 12,
    READSTAT_ERROR_WRITE = 13,
    READSTAT_ERROR_WRITER_NOT_INITIALIZED = 14,
    READSTAT_ERROR_SEEK = 15,
    READSTAT_ERROR_CONVERT = 16,
    READSTAT_ERROR_CONVERT_BAD_STRING = 17,
    READSTAT_ERROR_CONVERT_SHORT_STRING = 18,
    READSTAT_ERROR_CONVERT_LONG_STRING = 19,
    READSTAT_ERROR_NUMERIC_VALUE_IS_OUT_OF_RANGE = 20,
    READSTAT_ERROR_TAGGED_VALUE_IS_OUT_OF_RANGE = 21,
    READSTAT_ERROR_STRING_VALUE_IS_TOO_LONG = 22,
    READSTAT_ERROR_TAGGED_VALUES_NOT_SUPPORTED = 23,
    READSTAT_ERROR_UNSUPPORTED_FILE_FORMAT_VERSION = 24,
    READSTAT_ERROR_NAME_BEGINS_WITH_ILLEGAL_CHARACTER = 25,
    READSTAT_ERROR_NAME_CONTAINS_ILLEGAL_CHARACTER = 26,
    READSTAT_ERROR_NAME_IS_RESERVED_WORD = 27,
    READSTAT_ERROR_NAME_IS_TOO_LONG = 28,
    READSTAT_ERROR_BAD_TIMESTAMP_STRING = 29,
    READSTAT_ERROR_BAD_FREQUENCY_WEIGHT = 30,
    READSTAT_ERROR_TOO_MANY_MISSING_VALUE_DEFINITIONS = 31,
    READSTAT_ERROR_NOTE_IS_TOO_LONG = 32,
    READSTAT_ERROR_STRING_REFS_NOT_SUPPORTED = 33,
    READSTAT_ERROR_STRING_REF_IS_REQUIRED = 34,
    READSTAT_ERROR_ROW_IS_TOO_WIDE_FOR_PAGE = 35,
    READSTAT_ERROR_TOO_FEW_COLUMNS = 36,
    READSTAT_ERROR_TOO_MANY_COLUMNS = 37,
    READSTAT_ERROR_NAME_IS_ZERO_LENGTH = 38,
    READSTAT_ERROR_BAD_TIMESTAMP_VALUE = 39,
}

#[repr(C)]
pub struct readstat_writer_t {
    _opaque: [u8; 0],
}

#[repr(C)]
pub struct readstat_variable_t {
    _opaque: [u8; 0],
}

pub type readstat_data_writer =
    Option<unsafe extern "C" fn(data: *const c_void, len: usize, ctx: *mut c_void) -> isize>;

extern "C" {
    pub fn readstat_error_message(error: readstat_error_t) -> *const c_char;

    pub fn readstat_writer_init() -> *mut readstat_writer_t;
    pub fn readstat_writer_free(writer: *mut readstat_writer_t);

    pub fn readstat_set_data_writer(
        writer: *mut readstat_writer_t,
        data_writer: readstat_data_writer,
    ) -> readstat_error_t;

    pub fn readstat_writer_set_file_format_version(
        writer: *mut readstat_writer_t,
        file_format_version: u8,
    ) -> readstat_error_t;

    pub fn readstat_add_variable(
        writer: *mut readstat_writer_t,
        name: *const c_char,
        var_type: readstat_type_t,
        storage_width: usize,
    ) -> *mut readstat_variable_t;

    pub fn readstat_variable_set_label(
        variable: *mut readstat_variable_t,
        label: *const c_char,
    );

    pub fn readstat_variable_set_format(
        variable: *mut readstat_variable_t,
        format: *const c_char,
    );

    pub fn readstat_variable_set_measure(
        variable: *mut readstat_variable_t,
        measure: readstat_measure_t,
    );

    pub fn readstat_variable_set_alignment(
        variable: *mut readstat_variable_t,
        alignment: readstat_alignment_t,
    );

    pub fn readstat_variable_set_display_width(
        variable: *mut readstat_variable_t,
        display_width: c_int,
    );

    pub fn readstat_writer_set_compression(
        writer: *mut readstat_writer_t,
        compression: readstat_compress_t,
    ) -> readstat_error_t;

    pub fn readstat_writer_set_file_label(
        writer: *mut readstat_writer_t,
        file_label: *const c_char,
    ) -> readstat_error_t;

    pub fn readstat_begin_writing_sav(
        writer: *mut readstat_writer_t,
        user_ctx: *mut c_void,
        row_count: std::os::raw::c_long,
    ) -> readstat_error_t;

    pub fn readstat_begin_row(writer: *mut readstat_writer_t) -> readstat_error_t;

    pub fn readstat_insert_double_value(
        writer: *mut readstat_writer_t,
        variable: *const readstat_variable_t,
        value: f64,
    ) -> readstat_error_t;

    pub fn readstat_insert_string_value(
        writer: *mut readstat_writer_t,
        variable: *const readstat_variable_t,
        value: *const c_char,
    ) -> readstat_error_t;

    pub fn readstat_insert_missing_value(
        writer: *mut readstat_writer_t,
        variable: *const readstat_variable_t,
    ) -> readstat_error_t;

    pub fn readstat_end_row(writer: *mut readstat_writer_t) -> readstat_error_t;

    pub fn readstat_end_writing(writer: *mut readstat_writer_t) -> readstat_error_t;

    pub fn readstat_get_variable(
        writer: *mut readstat_writer_t,
        index: c_int,
    ) -> *mut readstat_variable_t;
}
