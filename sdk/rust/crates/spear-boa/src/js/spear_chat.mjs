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
  sleepMs: (ms) => __spear_sleep_ms(Number(ms) | 0),
  chat: {
    completions: {
      create: async (options) => {
        const raw = __spear_cchat_completion(JSON.stringify(options ?? {}));
        return new ChatCompletionResponse(raw);
      },
    },
  },
  userStream: {
    Direction: {
      INBOUND: 1,
      OUTBOUND: 2,
      BIDIRECTIONAL: 3,
    },
    open: (streamId, direction) => {
      const sid = Number(streamId) | 0;
      const dir = direction == null ? 3 : Number(direction) | 0;
      const fd = __spear_user_stream_open(sid, dir);
      const write = (data) => {
        const u8 =
          data instanceof Uint8Array
            ? data
            : new TextEncoder().encode(typeof data === "string" ? data : String(data));
        __spear_user_stream_write(fd, u8_to_bin(u8));
      };
      const read = () => {
        const bin = __spear_user_stream_read(fd);
        if (bin == null) return null;
        return bin_to_u8(bin);
      };
      const close = () => __spear_user_stream_close(fd);
      return { fd, read, write, close };
    },
    ctlOpen: () => {
      const fd = __spear_user_stream_ctl_open();
      const readEvent = () => __spear_user_stream_ctl_read_event(fd);
      const close = () => __spear_user_stream_close(fd);
      return { fd, readEvent, close };
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

function u8_to_bin(u8) {
  let s = "";
  for (let i = 0; i < u8.length; i++) s += String.fromCharCode(u8[i] & 255);
  return s;
}

function bin_to_u8(bin) {
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i) & 255;
  return out;
}
