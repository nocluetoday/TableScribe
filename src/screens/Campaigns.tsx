import { Plus, Search } from "lucide-react";
import { Button } from "../components/Button";
import { Card } from "../components/Card";
import { displayPath } from "../lib/paths";
import type { AppPaths, Campaign } from "../lib/types";

export function Campaigns({
  campaigns,
  paths,
  selectedCampaignId,
  onSelectCampaign,
  onNewCampaign
}: {
  campaigns: Campaign[];
  paths: AppPaths | null;
  selectedCampaignId?: string;
  onSelectCampaign: (campaignId: string) => void;
  onNewCampaign: () => void;
}) {
  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Campaigns</h1>
          <p>Local archives for recordings, transcripts, notes, and exports.</p>
        </div>
        <Button variant="primary" icon={<Plus size={18} />} onClick={onNewCampaign}>
          New Campaign
        </Button>
      </header>

      <div className="toolbar">
        <Search size={18} />
        <input placeholder="Search campaigns" aria-label="Search campaigns" />
      </div>

      <div className="campaign-grid">
        {campaigns.map((campaign) => (
          <button
            key={campaign.id}
            className={campaign.id === selectedCampaignId ? "campaign-card selected" : "campaign-card"}
            onClick={() => onSelectCampaign(campaign.id)}
          >
            <div className="card-heading">
              <h2>{campaign.name}</h2>
              <span>{campaign.system || "RPG campaign"}</span>
            </div>
            <p>{campaign.description || "Ready for local recordings and Markdown notes."}</p>
            <code>{displayPath(campaign.rootPath)}</code>
          </button>
        ))}
        {!campaigns.length && (
          <Card>
            <h2>No campaigns yet</h2>
            <p>Create a campaign to start saving recordings, transcripts, and notes as local files.</p>
          </Card>
        )}
      </div>

      <section className="storage-note">
        <strong>Storage root</strong>
        <span>{paths ? displayPath(paths.campaignsDir) : "Resolving local documents folder..."}</span>
      </section>
    </div>
  );
}
