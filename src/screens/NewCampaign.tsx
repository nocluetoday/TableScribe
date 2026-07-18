import { FormEvent, useState } from "react";
import { Button } from "../components/Button";

export function NewCampaign({
  onCreate,
  onCancel
}: {
  onCreate: (name: string, system?: string, description?: string) => Promise<void>;
  onCancel: () => void;
}) {
  const [name, setName] = useState("");
  const [system, setSystem] = useState("");
  const [description, setDescription] = useState("");

  async function submit(event: FormEvent) {
    event.preventDefault();
    if (!name.trim()) return;
    await onCreate(name.trim(), system.trim() || undefined, description.trim() || undefined);
  }

  return (
    <form className="screen form-screen" onSubmit={submit}>
      <header className="screen-header">
        <div>
          <h1>New Campaign</h1>
          <p>Create the readable folder structure TableScribe will use for every session.</p>
        </div>
      </header>

      <label>
        Campaign name
        <input value={name} onChange={(event) => setName(event.target.value)} placeholder="Curse of Strahd" />
      </label>
      <label>
        Game system
        <input value={system} onChange={(event) => setSystem(event.target.value)} placeholder="Dungeons & Dragons" />
      </label>
      <label>
        Notes
        <textarea value={description} onChange={(event) => setDescription(event.target.value)} placeholder="Who plays, tone, house rules, or campaign premise." />
      </label>

      <div className="button-row">
        <Button type="submit" variant="primary">Create Campaign</Button>
        <Button type="button" variant="quiet" onClick={onCancel}>Cancel</Button>
      </div>
    </form>
  );
}
