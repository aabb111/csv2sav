import {
  FileSpreadsheet,
  CheckCircle2,
  XCircle,
  Loader2,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Progress } from "@/components/ui/progress";
import type { ConvertFile } from "@/types";

interface FileListProps {
  files: ConvertFile[];
  onRemove: (id: string) => void;
  disabled?: boolean;
}

function formatRows(n: number): string {
  if (n >= 10000) return `${(n / 10000).toFixed(1)}万`;
  return String(n);
}

function StatusIcon({ status }: { status: ConvertFile["status"] }) {
  switch (status) {
    case "pending":
      return <FileSpreadsheet className="h-5 w-5 text-muted-foreground" />;
    case "converting":
      return <Loader2 className="h-5 w-5 animate-spin text-primary" />;
    case "success":
      return <CheckCircle2 className="h-5 w-5 text-green-600" />;
    case "error":
      return <XCircle className="h-5 w-5 text-destructive" />;
  }
}

function FileItem({
  file,
  onRemove,
  disabled,
}: {
  file: ConvertFile;
  onRemove: () => void;
  disabled?: boolean;
}) {
  return (
    <div className="flex items-center gap-3 rounded-lg border px-4 py-3">
      <StatusIcon status={file.status} />
      <div className="flex-1 min-w-0">
        <p className="truncate text-sm font-medium">{file.fileName}</p>
        {file.status === "converting" && (
          <div className="mt-2 space-y-1">
            <Progress value={file.progress} />
            <p className="text-xs text-muted-foreground">
              {file.currentRows > 0
                ? `已处理 ${formatRows(file.currentRows)} 行 (${file.progress.toFixed(1)}%)`
                : `准备中...`}
            </p>
          </div>
        )}
        {file.status === "success" && (
          <p className="mt-1 text-xs text-green-600">
            转换完成 · {formatRows(file.totalRows)} 行
          </p>
        )}
        {file.status === "error" && (
          <p className="mt-1 text-xs text-destructive">{file.errorMessage}</p>
        )}
      </div>
      <Button
        variant="ghost"
        size="icon"
        onClick={onRemove}
        disabled={disabled || file.status === "converting"}
        className="shrink-0"
      >
        <Trash2 className="h-4 w-4" />
      </Button>
    </div>
  );
}

export function FileList({ files, onRemove, disabled }: FileListProps) {
  if (files.length === 0) return null;

  return (
    <div className="space-y-2">
      {files.map((file) => (
        <FileItem
          key={file.id}
          file={file}
          onRemove={() => onRemove(file.id)}
          disabled={disabled}
        />
      ))}
    </div>
  );
}
