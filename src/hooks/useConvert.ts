import { useState, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { save } from "@tauri-apps/plugin-dialog";
import type { ConvertFile, ConvertProgress, ConvertResult } from "@/types";

let nextId = 0;

export function useConvert() {
  const [files, setFiles] = useState<ConvertFile[]>([]);
  const [converting, setConverting] = useState(false);
  const unlistenRef = useRef<UnlistenFn | null>(null);
  const cancelledRef = useRef(false);
  const filesRef = useRef<ConvertFile[]>(files);
  filesRef.current = files;

  function addFiles(paths: string[]) {
    const newFiles: ConvertFile[] = paths
      .filter((p) => p.toLowerCase().endsWith(".csv"))
      .map((inputPath) => ({
        id: String(++nextId),
        inputPath,
        fileName: inputPath.split(/[\\/]/).pop() ?? inputPath,
        fileSize: 0,
        status: "pending" as const,
        progress: 0,
        totalRows: 0,
        currentRows: 0,
      }));

    setFiles((prev) => {
      const existingPaths = new Set(prev.map((f) => f.inputPath));
      const unique = newFiles.filter((f) => !existingPaths.has(f.inputPath));
      return [...prev, ...unique];
    });
  }

  function removeFile(id: string) {
    setFiles((prev) => prev.filter((f) => f.id !== id));
  }

  function clearFiles() {
    setFiles([]);
  }

  async function cancelAll() {
    cancelledRef.current = true;
    await invoke("cancel_conversion");
  }

  async function convertAll() {
    const pendingFiles = filesRef.current.filter(
      (f) => f.status === "pending" || f.status === "error"
    );
    if (pendingFiles.length === 0) return;

    setConverting(true);
    cancelledRef.current = false;

    unlistenRef.current = await listen<ConvertProgress>(
      "convert-progress",
      (event) => {
        const { file, current_rows, bytes_read, file_size } = event.payload;
        setFiles((prev) =>
          prev.map((f) => {
            if (
              f.inputPath !== file ||
              f.status === "success" ||
              f.status === "error"
            )
              return f;

            const progress =
              file_size > 0
                ? Math.min((bytes_read / file_size) * 100, 100)
                : 0;

            return {
              ...f,
              status: "converting" as const,
              progress,
              currentRows: current_rows,
            };
          })
        );
      }
    );

    for (const file of pendingFiles) {
      if (cancelledRef.current) break;

      setFiles((prev) =>
        prev.map((f) =>
          f.id === file.id
            ? { ...f, status: "converting" as const, progress: 0 }
            : f
        )
      );

      const defaultOutput = file.inputPath.replace(/\.csv$/i, ".zsav");
      const outputPath = await save({
        defaultPath: defaultOutput,
        filters: [{ name: "SPSS ZSAV", extensions: ["zsav"] }],
        title: `保存 ${file.fileName} 为 ZSAV`,
      });

      if (!outputPath) {
        setFiles((prev) =>
          prev.map((f) =>
            f.id === file.id
              ? { ...f, status: "pending" as const, progress: 0 }
              : f
          )
        );
        continue;
      }

      try {
        const result = await invoke<ConvertResult>("convert_csv_to_sav", {
          inputPath: file.inputPath,
          outputPath,
        });

        setFiles((prev) =>
          prev.map((f) =>
            f.id === file.id
              ? {
                  ...f,
                  status: result.success ? "success" : "error",
                  progress: result.success ? 100 : 0,
                  totalRows: result.total_rows,
                  currentRows: result.total_rows,
                  outputPath: result.output_path,
                  errorMessage: result.error,
                }
              : f
          )
        );
      } catch (err) {
        setFiles((prev) =>
          prev.map((f) =>
            f.id === file.id
              ? {
                  ...f,
                  status: "error" as const,
                  progress: 0,
                  errorMessage: String(err),
                }
              : f
          )
        );
      }
    }

    if (cancelledRef.current) {
      setFiles((prev) =>
        prev.map((f) =>
          f.status === "converting"
            ? { ...f, status: "pending" as const, progress: 0 }
            : f
        )
      );
    }

    unlistenRef.current?.();
    unlistenRef.current = null;
    setConverting(false);
  }

  return {
    files,
    converting,
    addFiles,
    removeFile,
    clearFiles,
    convertAll,
    cancelAll,
  };
}
