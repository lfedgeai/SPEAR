import { Spear } from "spear";

export default async function main() {
  const tools = [
    Spear.tool({
      name: "sum",
      description: "Add two integers",
      parameters: {
        type: "object",
        properties: {
          a: { type: "integer" },
          b: { type: "integer" },
        },
        required: ["a", "b"],
      },
      handler: ({ a, b }) => {
        console.log("sum invoked:", "a=", a, "b=", b);
        return { sum: (a ?? 0) + (b ?? 0) };
      },
    }),
  ];

  const resp = await Spear.chat.completions.create({
    model: "gpt-4o-mini",
    messages: [{ role: "user", content: "Please call sum(a,b) for a=7 and b=35." }],
    tools,
    maxTotalToolCalls: 4,
    maxIterations: 4,
  });

  return resp.text();
}

