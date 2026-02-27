mod converter;
mod sav_writer;
mod schema;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize)]
struct ConvertProgress {
    file: String,
    current_rows: usize,
    bytes_read: u64,
    file_size: u64,
}

#[derive(Serialize)]
struct ConvertResult {
    input_path: String,
    output_path: String,
    total_rows: usize,
    success: bool,
    error: Option<String>,
    /// Columns whose values were truncated to 32767 bytes in the output.
    truncated_cols: Vec<String>,
}

#[derive(Clone)]
struct CancelFlag(Arc<AtomicBool>);

// Scan all rows to infer schema, avoiding truncation from undersampling.
const SAMPLE_ROWS: usize = usize::MAX;

fn emit_progress(app: &AppHandle, file: &str, current_rows: usize, bytes_read: u64, file_size: u64) {
    let _ = app.emit(
        "convert-progress",
        ConvertProgress {
            file: file.to_string(),
            current_rows,
            bytes_read,
            file_size,
        },
    );
}

#[tauri::command]
async fn cancel_conversion(app: AppHandle) {
    if let Some(flag) = app.try_state::<CancelFlag>() {
        flag.0.store(true, Ordering::Relaxed);
    }
}

#[tauri::command]
async fn convert_csv_to_sav(
    app: AppHandle,
    input_path: String,
    output_path: String,
) -> Result<ConvertResult, String> {
    let cancel_flag = app
        .try_state::<CancelFlag>()
        .ok_or("CancelFlag not managed")?;

    cancel_flag.0.store(false, Ordering::Relaxed);
    let cancelled = cancel_flag.0.clone();

    let input = input_path.clone();
    let output = output_path.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        let input_p = Path::new(&input);
        let output_p = Path::new(&output);
        let file_name = input.clone();

        let csv_schema = schema::infer_schema(input_p, SAMPLE_ROWS, &cancelled)?;

        if cancelled.load(Ordering::Relaxed) {
            return Err("Cancelled".to_string());
        }

        let file_size = csv_schema.file_size;
        let truncated_cols = csv_schema.truncated_cols.clone();
        emit_progress(&app, &file_name, 0, 0, file_size);

        let actual_rows = converter::convert_csv_to_sav(
            input_p,
            output_p,
            &csv_schema,
            &cancelled,
            &|current_rows, bytes_read, file_size| {
                emit_progress(&app, &file_name, current_rows, bytes_read, file_size);
            },
        )?;

        emit_progress(&app, &file_name, actual_rows, file_size, file_size);

        Ok::<_, String>((actual_rows, truncated_cols))
    })
    .await
    .map_err(|e| format!("Task failed: {e}"))?;

    match result {
        Ok((total_rows, truncated_cols)) => Ok(ConvertResult {
            input_path,
            output_path,
            total_rows,
            success: true,
            error: None,
            truncated_cols,
        }),
        Err(e) if e == "Cancelled" => Ok(ConvertResult {
            input_path,
            output_path,
            total_rows: 0,
            success: false,
            error: Some("已取消".to_string()),
            truncated_cols: vec![],
        }),
        Err(e) => Ok(ConvertResult {
            input_path,
            output_path,
            total_rows: 0,
            success: false,
            error: Some(e),
            truncated_cols: vec![],
        }),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(CancelFlag(Arc::new(AtomicBool::new(false))))
        .invoke_handler(tauri::generate_handler![convert_csv_to_sav, cancel_conversion])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
