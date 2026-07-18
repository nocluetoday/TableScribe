export function displayPath(path: string): string {
  return path.replace(/\\/g, " / ").replace(/\//g, " / ");
}

export function bytesToSize(bytes?: number): string {
  if (!bytes) return "Not installed";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value.toFixed(value >= 10 ? 0 : 1)} ${units[unit]}`;
}
