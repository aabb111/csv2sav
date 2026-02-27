import { cn } from "@/lib/utils";
import type { OutputFormat } from "@/types";

interface FormatSelectorProps {
  value: OutputFormat;
  onChange: (format: OutputFormat) => void;
  disabled?: boolean;
}

const formats: { value: OutputFormat; label: string; description: string }[] = [
  { value: "sav", label: ".sav", description: "标准格式" },
  { value: "zsav", label: ".zsav", description: "ZLib 压缩，体积更小" },
];

export function FormatSelector({ value, onChange, disabled }: FormatSelectorProps) {
  return (
    <div className="flex items-center gap-2">
      <span className="text-sm text-muted-foreground">输出格式：</span>
      <div className="flex rounded-md border">
        {formats.map((fmt) => (
          <button
            key={fmt.value}
            type="button"
            disabled={disabled}
            onClick={() => onChange(fmt.value)}
            className={cn(
              "relative px-3 py-1.5 text-sm font-medium transition-colors",
              "first:rounded-l-md last:rounded-r-md",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring",
              "disabled:pointer-events-none disabled:opacity-50",
              value === fmt.value
                ? "bg-primary text-primary-foreground"
                : "hover:bg-muted"
            )}
            title={fmt.description}
          >
            {fmt.label}
          </button>
        ))}
      </div>
      <span className="text-xs text-muted-foreground">
        {formats.find((f) => f.value === value)?.description}
      </span>
    </div>
  );
}
