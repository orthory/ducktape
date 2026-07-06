// Huddle audio worklet processors, loaded same-origin by voice-session.ts via
// AudioContext.audioWorklet.addModule("/voice-worklets.js"). This file runs in
// the AudioWorkletGlobalScope (its own realm), NOT the app bundle — it is kept
// out of src/ so the TS build never tries to typecheck WorkletGlobalScope
// globals (registerProcessor, AudioWorkletProcessor). Plain ES2020, no imports.
//
// Frame = 20 ms mono @ 48 kHz = 960 samples. Render quantum = 128 samples.

// capture: accumulate render quanta into 960-sample Float32 frames and post
// each completed frame (transferring its buffer) to the main thread, which
// converts to Int16 and sends it over the websocket.
class VoiceCaptureProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.frame = new Float32Array(960);
    this.filled = 0;
  }

  process(inputs) {
    const channel = inputs[0] && inputs[0][0];
    if (channel) {
      for (let i = 0; i < channel.length; i++) {
        this.frame[this.filled++] = channel[i];
        if (this.filled === 960) {
          const out = this.frame.slice(0);
          this.port.postMessage(out, [out.buffer]);
          this.filled = 0;
        }
      }
    }
    return true;
  }
}
registerProcessor("voice-capture", VoiceCaptureProcessor);

// playback: a small ring buffer of Float32 frames (from the main thread, one
// per mixed ws frame). Output 128 samples per quantum; an empty buffer plays
// silence (underrun), and the buffer is capped at ~10 frames (200 ms) by
// dropping the oldest — bounded latency over a growing backlog.
const MAX_FRAMES = 10;

class VoicePlaybackProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    this.frames = [];
    this.current = null;
    this.offset = 0;
    this.port.onmessage = (event) => {
      this.frames.push(event.data);
      if (this.frames.length > MAX_FRAMES) this.frames.shift();
    };
  }

  process(_inputs, outputs) {
    const out = outputs[0][0];
    if (!out) return true;
    for (let i = 0; i < out.length; i++) {
      if (!this.current || this.offset >= this.current.length) {
        this.current = this.frames.shift() || null;
        this.offset = 0;
      }
      out[i] = this.current ? this.current[this.offset++] : 0;
    }
    return true;
  }
}
registerProcessor("voice-playback", VoicePlaybackProcessor);
