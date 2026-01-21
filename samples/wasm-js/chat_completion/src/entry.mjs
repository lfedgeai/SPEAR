import { Spear } from "spear";

export default async function main() {
  const resp = await Spear.chat.completions.create({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "Hi" }],
    timeoutMs: 30_000,
  });

  return resp.text();
}

