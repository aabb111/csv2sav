export type FileStatus = "pending" | "converting" | "success" | "error";

export type OutputFormat = "sav" | "zsav";

export interface ConvertFile {
  id: string;
  inputPath: string;
  fileName: string;
  fileSize: number;
  status: FileStatus;
  progress: number;
  totalRows: number;
  currentRows: number;
  errorMessage?: string;
  outputPath?: string;
}

export interface ConvertProgress {
  file: string;
  current_rows: number;
  bytes_read: number;
  file_size: number;
}

export interface ConvertResult {
  input_path: string;
  output_path: string;
  total_rows: number;
  success: boolean;
  error?: string;
}
