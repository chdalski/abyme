/** A note's whole text: its name and, when it has one, a line break and its body. */
export function noteText(name: string, body: string): string {
  return body === "" ? name : `${name}\n${body}`;
}
