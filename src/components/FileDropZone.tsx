import { open } from "@tauri-apps/plugin-dialog";
import { Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useDragDrop } from "@/hooks/useDragDrop";

interface FileDropZoneProps {
  onFilesSelected: (paths: string[]) => void;
  disabled?: boolean;
}

export function FileDropZone({ onFilesSelected, disabled }: FileDropZoneProps) {
  const { dragOver } = useDragDrop(onFilesSelected);

  const handleClick = async () => {
    const selected = await open({
      multiple: true,
      filters: [{ name: "CSV", extensions: ["csv"] }],
      title: "选择 CSV 文件",
    });
    if (selected) {
      const paths = Array.isArray(selected) ? selected : [selected];
      onFilesSelected(paths);
    }
  };

  return (
    <section
      aria-label="File drop zone"
      className={cn(
        "relative flex flex-col items-center justify-center gap-4 rounded-xl border-2 border-dashed p-12 transition-colors",
        dragOver
          ? "border-primary bg-primary/5"
          : "border-muted-foreground/25 hover:border-muted-foreground/50",
        disabled && "pointer-events-none opacity-50"
      )}
    >
      <div className="rounded-full bg-muted p-4">
        <Upload className="h-8 w-8 text-muted-foreground" />
      </div>
      <div className="text-center">
        <p className="text-lg font-medium">拖放 CSV 文件到此处</p>
        <p className="mt-1 text-sm text-muted-foreground">
          支持多文件批量转换，每个文件可达 10GB+
        </p>
      </div>
      <Button variant="outline" onClick={handleClick} disabled={disabled}>
        选择文件
      </Button>
    </section>
  );
}
