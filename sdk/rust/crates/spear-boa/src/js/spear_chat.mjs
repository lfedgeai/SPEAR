class ChatCompletionResponse {
  constructor(rawJson) {
    this._rawJson = rawJson;
  }

  json() {
    return JSON.parse(this._rawJson);
  }

  text() {
    try {
      const j = this.json();
      const c = j?.choices?.[0]?.message?.content;
      if (typeof c === "string") return c;
    } catch (_) {}
    return this._rawJson;
  }

  raw() {
    const enc = new TextEncoder();
    return enc.encode(this._rawJson);
  }
}

if (typeof globalThis.console !== "object" || globalThis.console === null) {
  globalThis.console = {};
}
if (typeof globalThis.console.log !== "function") {
  globalThis.console.log = (...args) =>
    __spear_print(args.map((x) => String(x)).join(" "));
}

export const Spear = {
  chat: {
    completions: {
      create: async (options) => {
        const raw = __spear_cchat_completion(JSON.stringify(options ?? {}));
        return new ChatCompletionResponse(raw);
      },
    },
  },
  tool: (spec) => {
    const name = spec?.name;
    const description = spec?.description;
    const parameters = spec?.parameters;
    const handler = spec?.handler;

    if (typeof name !== "string" || name.length === 0) {
      throw new Error("invalid tool name");
    }
    if (typeof handler !== "function") {
      throw new Error("invalid tool handler");
    }

    const fnObj = {
      type: "function",
      function: {
        name,
        description: typeof description === "string" ? description : "",
        parameters: parameters ?? { type: "object", properties: {} },
      },
    };

    const fnJson = JSON.stringify(fnObj);
    const wrapper = (argsJson) => handler(JSON.parse(argsJson));
    return __spear_tool_register(fnJson, wrapper);
  },
};
