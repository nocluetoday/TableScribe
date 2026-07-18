import { Mic, Square } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { AudioLevelMeter } from "../components/AudioLevelMeter";
import { Button } from "../components/Button";
import { Card } from "../components/Card";
import recorderWorkletUrl from "../audio/recorder-worklet.js?url";
import { api } from "../lib/tauri";
import type { Campaign } from "../lib/types";

type LivePartialPayload = { text: string; recordedSeconds: number };
type LiveDisabledPayload = { reason: string };

type RecorderState = {
  context: AudioContext;
  source: MediaStreamAudioSourceNode;
  worklet: AudioWorkletNode;
  analyser: AnalyserNode;
  stream: MediaStream;
  sampleRate: number;
};

export function RecordSession({ campaign, onCreated }: { campaign?: Campaign; onCreated: () => Promise<void> }) {
  const [sessionName, setSessionName] = useState("session-001");
  const [isRecording, setIsRecording] = useState(false);
  const [seconds, setSeconds] = useState(0);
  const [message, setMessage] = useState("");
  const [inputLevel, setInputLevel] = useState(0);
  const [diagnostics, setDiagnostics] = useState("Microphone idle.");
  const [livePartial, setLivePartial] = useState("");
  const [liveDisabled, setLiveDisabled] = useState<string | null>(null);
  const recorderRef = useRef<RecorderState | null>(null);
  const animationRef = useRef<number | null>(null);
  const pumpRef = useRef<Promise<void>>(Promise.resolve());
  const activeRecordingRef = useRef(false);
  const startMsRef = useRef<number | null>(null);

  useEffect(() => {
    if (!isRecording) return;
    // Derive elapsed seconds from a captured wall-clock start time rather
    // than incrementing on each tick — WebKit throttles setInterval when
    // the window is backgrounded, so an increment-based timer falls behind
    // real time whenever the user clicks away.
    const id = window.setInterval(() => {
      const startMs = startMsRef.current;
      if (startMs === null) return;
      setSeconds(Math.floor((Date.now() - startMs) / 1000));
    }, 1000);
    return () => window.clearInterval(id);
  }, [isRecording]);

  useEffect(() => {
    return () => {
      void cleanupRecorder();
      if (activeRecordingRef.current) {
        activeRecordingRef.current = false;
        void api.recordingCancel().catch(() => undefined);
      }
    };
  }, []);

  useEffect(() => {
    if (!isRecording) return;
    let unlistenPartial: (() => void) | undefined;
    let unlistenDisabled: (() => void) | undefined;
    let cancelled = false;
    void listen<LivePartialPayload>("live-partial", (event) => {
      if (cancelled) return;
      setLivePartial(event.payload.text);
    }).then((fn) => {
      if (cancelled) fn();
      else unlistenPartial = fn;
    });
    void listen<LiveDisabledPayload>("live-partial-disabled", (event) => {
      if (cancelled) return;
      setLiveDisabled(event.payload.reason);
    }).then((fn) => {
      if (cancelled) fn();
      else unlistenDisabled = fn;
    });
    return () => {
      cancelled = true;
      unlistenPartial?.();
      unlistenDisabled?.();
    };
  }, [isRecording]);

  async function start() {
    if (!campaign) return;
    if (!navigator.mediaDevices?.getUserMedia) {
      setMessage("This WebView does not expose microphone recording.");
      setDiagnostics("navigator.mediaDevices.getUserMedia is unavailable.");
      return;
    }

    let stream: MediaStream | null = null;
    let context: AudioContext | null = null;
    let recordingStarted = false;
    try {
      setDiagnostics("Requesting microphone permission...");
      stream = await navigator.mediaDevices.getUserMedia({
        audio: {
          channelCount: 1,
          echoCancellation: false,
          noiseSuppression: false,
          autoGainControl: false
        }
      });
      context = new AudioContext();
      await context.resume();
      await context.audioWorklet.addModule(recorderWorkletUrl);

      const source = context.createMediaStreamSource(stream);
      const analyser = context.createAnalyser();
      analyser.fftSize = 512;
      const worklet = new AudioWorkletNode(context, "recorder-processor");

      const audioPath = await api.recordingStart(campaign.id, sessionName);
      recordingStarted = true;
      activeRecordingRef.current = true;
      pumpRef.current = Promise.resolve();

      worklet.port.onmessage = (event) => {
        const data = event.data as { type?: string; bytes?: Uint8Array };
        if (data?.type !== "chunk" || !data.bytes) return;
        const chunk = data.bytes;
        const encoded = bytesToBase64(chunk);
        pumpRef.current = pumpRef.current.then(() =>
          api.recordingAppendChunk(encoded).catch((error) => {
            console.error("Failed to append audio chunk", error);
            setMessage(error instanceof Error ? error.message : String(error));
          })
        );
      };

      source.connect(analyser);
      source.connect(worklet);
      // Intentionally do not connect the worklet to ctx.destination — the
      // MediaStreamSource pulls it from upstream and routing to the speakers
      // would create a microphone feedback loop.

      recorderRef.current = {
        context,
        source,
        worklet,
        analyser,
        stream,
        sampleRate: context.sampleRate
      };
      startMsRef.current = Date.now();
      setSeconds(0);
      setLivePartial("");
      setLiveDisabled(null);
      setMessage(`Recording to ${audioPath}. Keep this window open until you stop.`);
      setDiagnostics(
        `Input active. Capture rate: ${context.sampleRate} Hz, streamed at 16 kHz mono. Tracks: ${stream
          .getAudioTracks()
          .map((track) => track.label || "microphone")
          .join(", ")}`
      );
      setIsRecording(true);
      updateInputLevel();
    } catch (error) {
      if (recordingStarted) {
        activeRecordingRef.current = false;
        await api.recordingCancel().catch(() => undefined);
      }
      if (stream) stream.getTracks().forEach((track) => track.stop());
      if (context) await context.close().catch(() => undefined);
      setMessage(microphoneErrorMessage(error));
      setDiagnostics(error instanceof Error ? `${error.name}: ${error.message}` : String(error));
      setIsRecording(false);
    }
  }

  async function stop() {
    if (!campaign || !recorderRef.current) return;
    setIsRecording(false);
    setInputLevel(0);
    startMsRef.current = null;
    await cleanupRecorder();

    if (!activeRecordingRef.current) {
      setDiagnostics("Recording stopped before any audio was captured.");
      return;
    }
    activeRecordingRef.current = false;

    try {
      await pumpRef.current;
      const session = await api.recordingStop();
      setMessage(`Saved microphone WAV to ${session.audioPath}`);
      setDiagnostics(`Streamed audio finalized: ${session.audioPath}`);
      await onCreated();
    } catch (error) {
      setMessage(error instanceof Error ? error.message : String(error));
      setDiagnostics("Failed while finalizing the WAV file.");
    }
  }

  async function cleanupRecorder() {
    if (animationRef.current !== null) {
      window.cancelAnimationFrame(animationRef.current);
      animationRef.current = null;
    }
    const recorder = recorderRef.current;
    recorderRef.current = null;
    if (!recorder) return;
    try {
      recorder.worklet.port.onmessage = null;
      recorder.worklet.disconnect();
    } catch {
      // already disconnected
    }
    try {
      recorder.source.disconnect();
    } catch {
      // already disconnected
    }
    recorder.stream.getTracks().forEach((track) => track.stop());
    await recorder.context.close().catch(() => undefined);
  }

  function updateInputLevel() {
    const recorder = recorderRef.current;
    if (!recorder) return;
    const data = new Uint8Array(recorder.analyser.fftSize);
    recorder.analyser.getByteTimeDomainData(data);
    let sum = 0;
    for (const value of data) {
      const centered = value - 128;
      sum += centered * centered;
    }
    const rms = Math.sqrt(sum / data.length) / 128;
    setInputLevel(Math.min(100, Math.round(rms * 260)));
    animationRef.current = window.requestAnimationFrame(updateInputLevel);
  }

  const timer = new Date(seconds * 1000).toISOString().substring(11, 19);

  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Record Session</h1>
          <p>{campaign ? campaign.name : "Choose or create a campaign before recording."}</p>
        </div>
        <Button variant="primary" icon={<Mic size={18} />} disabled={!campaign || isRecording} onClick={start}>
          Start Recording
        </Button>
      </header>

      <label className="inline-field">
        Session name
        <input value={sessionName} onChange={(event) => setSessionName(event.target.value)} />
      </label>

      <div className="recording-panel">
        <div className="timer">{timer}</div>
        <AudioLevelMeter value={inputLevel} active={isRecording} />
        <Button icon={<Square size={18} />} disabled={!isRecording} onClick={stop}>Stop</Button>
      </div>

      <Card>
        <div className="live-draft-label">Live draft (low accuracy)</div>
        {liveDisabled ? (
          <p className="live-draft-disabled">{liveDisabled}</p>
        ) : (
          <p className="live-draft">
            {livePartial || (isRecording ? "Listening…" : "Start recording to see live draft text here.")}
          </p>
        )}
      </Card>

      <Card>
        <h2>Recording rules</h2>
        <p>Audio streams to disk as 16 kHz mono PCM while you record, so long sessions do not buffer in memory. The live draft above re-runs whisper on the trailing window every few seconds and is not the final transcript — accuracy and punctuation will improve when you stop and run full transcription.</p>
      </Card>

      <p className="permission-warning">
        If microphone access is disabled, open System Settings, then Privacy & Security, then Microphone, and enable TableScribe.
      </p>
      <Card>
        <h2>Microphone Status</h2>
        <p>{diagnostics}</p>
      </Card>
      {message && <p className="status-message">{message}</p>}
    </div>
  );
}

function microphoneErrorMessage(error: unknown): string {
  if (error instanceof DOMException && (error.name === "NotAllowedError" || error.name === "SecurityError")) {
    return "Microphone access was denied. Open System Settings, then Privacy & Security, then Microphone, and enable TableScribe.";
  }
  if (error instanceof DOMException && error.name === "NotFoundError") {
    return "No microphone was found. Connect or enable an input device, then try again.";
  }
  return `Could not start microphone recording: ${String(error)}`;
}

function bytesToBase64(bytes: Uint8Array): string {
  const chunkSize = 0x8000;
  let binary = "";
  for (let i = 0; i < bytes.length; i += chunkSize) {
    const slice = bytes.subarray(i, i + chunkSize);
    binary += String.fromCharCode.apply(null, slice as unknown as number[]);
  }
  return btoa(binary);
}
