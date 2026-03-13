import { Spear } from "spear";

async function runChat(options) {
  console.log("sending chat request: " + JSON.stringify(options));
  const resp = await Spear.chat.completions.create(options);
  const json = resp.json();
  const backend = json?._spear?.backend ?? "";
  const out = { backend, json, text: resp.text() };
  console.log("Received response: " + JSON.stringify(out));
  return out;
}

export default async function main() {

  const r1 = await runChat({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "Hello, which LLM are you?" }],
    timeoutMs: 30_000,
  });

  const r2 = await runChat({
    messages: [{ role: "user", content: "Which LLM are you? Here is another question: Is this a good password 'P@ssw0rd!'" }],
    timeoutMs: 30_000,
  });

  if (!String(r2.backend).startsWith("managed/llamacpp/")) {
    throw new Error("expected managed/llamacpp/*, got " + String(r2.backend));
  }

  return JSON.stringify({
    req1_backend: r1.backend,
    req2_backend: r2.backend,
  });
}
