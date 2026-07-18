import { useEffect, useMemo, useState } from "react";
import { Layout } from "./components/Layout";
import { api } from "./lib/tauri";
import type { AppPaths, Campaign, ModelInfo, ScreenId, SessionInfo, TranscriptionJob } from "./lib/types";
import { Campaigns } from "./screens/Campaigns";
import { ImportAudio } from "./screens/ImportAudio";
import { ModelManager } from "./screens/ModelManager";
import { NewCampaign } from "./screens/NewCampaign";
import { RecordSession } from "./screens/RecordSession";
import { SessionNotes } from "./screens/SessionNotes";
import { Settings } from "./screens/Settings";
import { TranscriptReview } from "./screens/TranscriptReview";
import { TranscriptionQueue } from "./screens/TranscriptionQueue";

export default function App() {
  const [activeScreen, setActiveScreen] = useState<ScreenId>("campaigns");
  const [campaigns, setCampaigns] = useState<Campaign[]>([]);
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [jobs, setJobs] = useState<TranscriptionJob[]>([]);
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [selectedCampaignId, setSelectedCampaignId] = useState<string | null>(null);
  const [selectedSessionName, setSelectedSessionName] = useState<string | null>(null);

  useEffect(() => {
    // Clear any orphan recording state left over from a page reload or webview
    // crash. Idempotent on the Rust side: no-op if nothing is active.
    void api.recordingCancel().catch(() => undefined);
    void Promise.all([api.getPaths(), api.listCampaigns(), api.listModels(), api.listJobs()]).then(
      ([nextPaths, nextCampaigns, nextModels, nextJobs]) => {
        setPaths(nextPaths);
        setCampaigns(nextCampaigns);
        setModels(nextModels);
        setJobs(nextJobs);
        setSelectedCampaignId((current) => current || nextCampaigns[0]?.id || null);
      }
    );
  }, []);

  const currentCampaign = useMemo(
    () => campaigns.find((campaign) => campaign.id === selectedCampaignId) || campaigns[0],
    [campaigns, selectedCampaignId]
  );
  const currentSession = useMemo(
    () => sessions.find((session) => session.sessionName === selectedSessionName) || sessions[0],
    [sessions, selectedSessionName]
  );

  useEffect(() => {
    if (!currentCampaign) {
      setSessions([]);
      return;
    }
    void api.listSessions(currentCampaign.id).then((nextSessions) => {
      setSessions(nextSessions);
      setSelectedSessionName((current) => current || nextSessions[0]?.sessionName || null);
    });
  }, [currentCampaign?.id]);

  const createCampaign = async (name: string, system?: string, description?: string) => {
    const campaign = await api.createCampaign(name, system, description);
    setCampaigns((current) => [campaign, ...current.filter((item) => item.id !== campaign.id)]);
    setSelectedCampaignId(campaign.id);
    setActiveScreen("campaigns");
  };

  const refreshSessions = async () => {
    if (!currentCampaign) return;
    const nextSessions = await api.listSessions(currentCampaign.id);
    setSessions(nextSessions);
    setSelectedSessionName(nextSessions[0]?.sessionName || null);
  };

  return (
    <Layout active={activeScreen} onNavigate={setActiveScreen}>
      {activeScreen === "campaigns" && (
        <Campaigns
          campaigns={campaigns}
          paths={paths}
          selectedCampaignId={currentCampaign?.id}
          onSelectCampaign={setSelectedCampaignId}
          onNewCampaign={() => setActiveScreen("new-campaign")}
        />
      )}
      {activeScreen === "new-campaign" && <NewCampaign onCreate={createCampaign} onCancel={() => setActiveScreen("campaigns")} />}
      {activeScreen === "record-session" && <RecordSession campaign={currentCampaign} onCreated={refreshSessions} />}
      {activeScreen === "import-audio" && <ImportAudio campaign={currentCampaign} onImported={refreshSessions} />}
      {activeScreen === "queue" && (
        <TranscriptionQueue
          jobs={jobs}
          sessions={sessions}
          onSelectSession={setSelectedSessionName}
          onRefresh={refreshSessions}
        />
      )}
      {activeScreen === "transcript-review" && (
        <TranscriptReview campaign={currentCampaign} session={currentSession} onRefresh={refreshSessions} />
      )}
      {activeScreen === "session-notes" && <SessionNotes campaign={currentCampaign} session={currentSession} />}
      {activeScreen === "model-manager" && <ModelManager models={models} paths={paths} />}
      {activeScreen === "settings" && <Settings paths={paths} />}
    </Layout>
  );
}
