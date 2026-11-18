// Release notes (Markdown from GitHub, written in CHANGELOG.md) as plain lines for the update banner.
// Only what the changelog uses: headings, bullet points and text; links keep their words, emphasis goes.

export interface NoteLine {
  kind: "heading" | "item" | "text";
  text: string;
}

const plain = (s: string) =>
  s.replace(/\[([^\]]+)\]\([^)]*\)/g, "$1") // [words](url) -> words
    .replace(/(\*\*|__|`)/g, "")
    .replace(/(^|\s)[*_]([^*_]+)[*_](?=\s|$|[.,;:!?])/g, "$1$2")
    .trim();

export function noteLines(markdown: string): NoteLine[] {
  const lines: NoteLine[] = [];
  for (const raw of markdown.split(/\r?\n/)) {
    const line = raw.trim();
    if (!line) continue;
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    const item = line.match(/^[-*+]\s+(.*)$/);
    if (heading) lines.push({ kind: "heading", text: plain(heading[1]) });
    else if (item) lines.push({ kind: "item", text: plain(item[1]) });
    else lines.push({ kind: "text", text: plain(line) });
  }
  return lines.filter((l) => l.text);
}
