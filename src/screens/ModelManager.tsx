import { Download, Trash2 } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { Button } from "../components/Button";
import { Card } from "../components/Card";
import { ProgressBar } from "../components/ProgressBar";
import { bytesToSize, displayPath } from "../lib/paths";
import { api } from "../lib/tauri";
import type { AppPaths, ModelInfo } from "../lib/types";

type ModelDownloadProgress = {
  name: string;
  downloadedBytes: number;
  totalBytes?: number;
  percent?: number;
  status: string;
};

export function ModelManager({ models, paths }: { models: ModelInfo[]; paths: AppPaths | null }) {
  const [modelList, setModelList] = useState(models);
  const [workingModel, setWorkingModel] = useState<string | null>(null);
  const [progress, setProgress] = useState<ModelDownloadProgress | null>(null);
  const [message, setMessage] = useState("medium.en is the recommended default for better transcripts.");

  useEffect(() => {
    setModelList(models);
  }, [models]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<ModelDownloadProgress>("model-download-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.status === "complete") {
        setMessage(`${event.payload.name} download complete.`);
      }
    }).then((nextUnlisten) => {
      unlisten = nextUnlisten;
    });
    return () => {
      unlisten?.();
    };
  }, []);

  async function refresh() {
    setModelList(await api.listModels());
  }

  async function download(name: string) {
    setWorkingModel(name);
    setProgress({ name, downloadedBytes: 0, percent: 0, status: "starting" });
    setMessage(`Downloading ${name}. This can take a while for larger models.`);
    try {
      await api.downloadModel(name);
      await refresh();
      setMessage(`${name} installed.`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setWorkingModel(null);
      setProgress(null);
    }
  }

  async function remove(name: string) {
    setWorkingModel(name);
    try {
      await api.deleteModel(name);
      await refresh();
      setMessage(`${name} removed.`);
    } catch (error) {
      setMessage(errorMessage(error));
    } finally {
      setWorkingModel(null);
    }
  }

  const visibleModels = modelList.length ? modelList : models;

  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Model Manager</h1>
          <p>Install whisper.cpp models locally. Models are not bundled in the app installer.</p>
        </div>
      </header>

      <div className="model-list">
        {visibleModels.map((model) => (
          <Card key={model.name} className="model-row">
            <div>
              <h2>{model.name}</h2>
              <span>{model.recommendedUse}</span>
            </div>
            <strong>{bytesToSize(model.sizeBytes)}</strong>
            <Button
              icon={model.installed ? <Trash2 size={18} /> : <Download size={18} />}
              disabled={workingModel !== null}
              onClick={() => model.installed ? remove(model.name) : download(model.name)}
            >
              {workingModel === model.name ? "Working" : model.installed ? "Delete" : "Download"}
            </Button>
          </Card>
        ))}
      </div>

      {progress && (
        <Card className="download-dialog">
          <div>
            <h2>Downloading {progress.name}</h2>
            <span>{formatProgress(progress)}</span>
          </div>
          <ProgressBar value={progress.percent ?? 0} />
        </Card>
      )}

      <section className="storage-note">
        <strong>Model folder</strong>
        <span>{paths ? displayPath(paths.modelsDir) : "Resolving app data folder..."}</span>
      </section>
      {message && <p className="status-message">{message}</p>}
    </div>
  );
}

function formatProgress(progress: ModelDownloadProgress): string {
  const downloaded = bytesToSize(progress.downloadedBytes);
  const total = progress.totalBytes ? bytesToSize(progress.totalBytes) : "unknown size";
  const percent = progress.percent === undefined ? "" : ` (${progress.percent}%)`;
  return `${downloaded} of ${total}${percent}`;
}

function errorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  if (error instanceof Error) return error.message;
  return `Model action failed: ${JSON.stringify(error)}`;
}
