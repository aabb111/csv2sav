import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { FileDropZone } from "@/components/FileDropZone";
import { FileList } from "@/components/FileList";
import { ConvertButton } from "@/components/ConvertButton";
import { FormatSelector } from "@/components/FormatSelector";
import { useConvert } from "@/hooks/useConvert";

function App() {
  const {
    files,
    converting,
    outputFormat,
    setOutputFormat,
    addFiles,
    removeFile,
    clearFiles,
    convertAll,
    cancelAll,
  } = useConvert();

  return (
    <div className="min-h-screen bg-background p-6">
      <div className="mx-auto max-w-2xl space-y-6">
        <Card>
          <CardHeader>
            <CardTitle className="text-2xl">CSV → SPSS 转换工具</CardTitle>
            <CardDescription>
              将 CSV 文件转换为 SPSS SAV / ZSAV 格式，支持大文件流式处理
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <FormatSelector
              value={outputFormat}
              onChange={setOutputFormat}
              disabled={converting}
            />
            <FileDropZone
              onFilesSelected={addFiles}
              disabled={converting}
            />
            <FileList
              files={files}
              onRemove={removeFile}
              disabled={converting}
            />
            <ConvertButton
              files={files}
              converting={converting}
              onConvert={convertAll}
              onCancel={cancelAll}
              onClear={clearFiles}
            />
          </CardContent>
        </Card>
      </div>
    </div>
  );
}

export default App;
