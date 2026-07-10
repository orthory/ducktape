#!/usr/bin/env bash
# Give a headless app a working microphone. The native app captures audio via
# getUserMedia({audio}); on a headless box there is NO audio input device, so
# capture fails with `mic-missing`. This sets up a PulseAudio null-sink whose
# monitor is exposed as a capture source, and (optionally) feeds it a tone so
# the "mic" is non-silent. Run it INSIDE the app container (or any box) BEFORE
# launching the app, with the same PULSE_SERVER the app process sees.
#
# Requires: pulseaudio (+ optionally gstreamer1.0-tools for the tone).
set -euo pipefail
export PULSE_SERVER="${PULSE_SERVER:-unix:/tmp/pulse-callbed/native}"
SOCK_DIR="$(dirname "${PULSE_SERVER#unix:}")"
mkdir -p "$SOCK_DIR"

if ! pactl info >/dev/null 2>&1; then
  echo "[vmic] starting user pulseaudio on $PULSE_SERVER"
  pulseaudio --daemonize=no --exit-idle-time=-1 \
    --load="module-native-protocol-unix socket=${PULSE_SERVER#unix:}" \
    --load="module-null-sink sink_name=vmic sink_properties=device.description=Ducktape_VMic" \
    --load="module-virtual-source source_name=vsource master=vmic.monitor" &
  for _ in $(seq 1 20); do pactl info >/dev/null 2>&1 && break; sleep 0.5; done
fi

# make the null-sink monitor the default capture device the app will grab.
pactl set-default-source vmic.monitor 2>/dev/null || true
echo "[vmic] ready — capture source: vmic.monitor  (PULSE_SERVER=$PULSE_SERVER)"

# OPTIONAL: feed a 440Hz tone into the sink so the far side hears something.
# Needs gstreamer1.0-tools (gst-launch-1.0) or swap for sox/ffmpeg.
if [ "${VMIC_TONE:-0}" = "1" ] && command -v gst-launch-1.0 >/dev/null 2>&1; then
  echo "[vmic] feeding a 440Hz tone into vmic"
  gst-launch-1.0 -q audiotestsrc freq=440 is-live=true ! audioconvert ! \
    audioresample ! pulsesink device=vmic >/dev/null 2>&1 &
fi
