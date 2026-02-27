import { Play, Square, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { ConvertFile } from "@/types";

interface ConvertButtonProps {
  files: ConvertFile[];
  converting: boolean;
  onConvert: () => void;
  onCancel: () => void;
  onClear: () => void;
}

export function ConvertButton({
  files,
  converting,
  onConvert,
  onCancel,
  onClear,
}: ConvertButtonProps) {
  const pendingCount = files.filter(
    (f) => f.status === "pending" || f.status === "error"
  ).length;

  if (files.length === 0) return null;

  return (
    <div className="flex items-center justify-between">
      <p className="text-sm text-muted-foreground">
        共 {files.length} 个文件，{pendingCount} 个待转换
      </p>
      <div className="flex gap-2">
        <Button variant="outline" onClick={onClear} disabled={converting}>
          <Trash2 className="h-4 w-4" />
          清空列表
        </Button>
        {converting ? (
          <Button variant="destructive" onClick={onCancel}>
            <Square className="h-4 w-4" />
            停止转换
          </Button>
        ) : (
          <Button onClick={onConvert} disabled={pendingCount === 0}>
            <Play className="h-4 w-4" />
            开始转换 ({pendingCount})
          </Button>
        )}
      </div>
    </div>
  );
}
