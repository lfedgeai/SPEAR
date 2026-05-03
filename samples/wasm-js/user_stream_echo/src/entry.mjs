import { Spear } from "spear";

export default function main() {
  console.log("user_stream_echo(js) started (waiting for user streams)");
  console.log("env:", {
    hasMap: typeof Map,
    hasUint8Array: typeof Uint8Array,
    hasTextEncoder: typeof TextEncoder,
    hasTextDecoder: typeof TextDecoder,
    hasSpear: typeof Spear,
    hasUserStream: typeof Spear?.userStream,
    hasCtlOpen: typeof Spear?.userStream?.ctlOpen,
    hasOpen: typeof Spear?.userStream?.open,
    hasSleepMs: typeof Spear?.sleepMs,
  });

  let ctl = null;
  let streams = null;
  try {
    ctl = Spear.userStream.ctlOpen();
    streams = new Map();
  } catch (e) {
    console.log("init failed:", String(e));
    try {
      console.log("init failed (detail):", e);
    } catch (_) {}
    return "user_stream_echo(js) init failed";
  }

  let loop = 0;
  try {
    for (;;) {
      loop++;
      let didWork = false;

      if (loop <= 5) console.log("loop tick", loop);

      let evt = null;
      try {
        evt = ctl.readEvent();
      } catch (e) {
        console.log("ctl.readEvent threw:", String(e));
        try {
          console.log("ctl.readEvent threw (detail):", e);
        } catch (_) {}
        return "user_stream_echo(js) ctl.readEvent threw";
      }

      if (loop <= 5) console.log("ctl event:", evt);

      if (evt && typeof evt.streamId === "number" && typeof evt.kind === "number") {
        didWork = true;

        if (evt.kind === 1) {
          const streamId = evt.streamId >>> 0;
          if (!streams.has(streamId)) {
            let s = null;
            try {
              s = Spear.userStream.open(streamId, Spear.userStream.Direction.BIDIRECTIONAL);
            } catch (e) {
              console.log("userStream.open threw:", streamId, String(e));
              try {
                console.log("userStream.open threw (detail):", e);
              } catch (_) {}
              return "user_stream_echo(js) userStream.open threw";
            }
            streams.set(streamId, s);
            console.log("stream connected:", streamId);
          }
        } else if (evt.kind === 2) {
          console.log("session closed");
          break;
        } else {
          console.log("unknown ctl event:", evt.kind, evt.streamId);
        }
      }

      try {
        for (const [streamId, s] of streams.entries()) {
          let data = null;
          try {
            data = s.read();
          } catch (e) {
            console.log("stream.read threw:", streamId, String(e));
            try {
              console.log("stream.read threw (detail):", e);
            } catch (_) {}
            return "user_stream_echo(js) stream.read threw";
          }
          if (data) {
            didWork = true;
            try {
              s.write(data);
            } catch (e) {
              console.log("stream.write threw:", streamId, String(e));
              try {
                console.log("stream.write threw (detail):", e);
              } catch (_) {}
              return "user_stream_echo(js) stream.write threw";
            }
          }
        }
      } catch (e) {
        console.log("iterate streams failed:", String(e));
        try {
          console.log("iterate streams failed (detail):", e);
        } catch (_) {}
        return "user_stream_echo(js) iterate streams failed";
      }

      if (!didWork) {
        try {
          Spear.sleepMs(10);
        } catch (e) {
          console.log("sleepMs threw:", String(e));
          try {
            console.log("sleepMs threw (detail):", e);
          } catch (_) {}
          return "user_stream_echo(js) sleepMs threw";
        }
      }
    }
  } catch (e) {
    console.log("main loop threw:", String(e));
    try {
      console.log("main loop threw (detail):", e);
    } catch (_) {}
    return "user_stream_echo(js) main loop threw";
  } finally {
    try {
      for (const s of streams.values()) {
        try {
          s.close();
        } catch (_) {}
      }
      ctl.close();
    } catch (_) {}
  }

  return "user_stream_echo(js) done";
}
