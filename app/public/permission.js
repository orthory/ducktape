// The native permission consent window (opened by src-tauri/src/permissions.rs).
//
// It renders the pending request and sends back exactly one answer. It holds no
// state of its own: the request lives in the shell, so a page that manages to
// reload or race this window cannot change what is being asked, or answer for
// the user. Everything shown is set as text, never as markup.

const invoke = (command, args) => window.__TAURI_INTERNALS__.invoke(command, args);

/** Runtime-neutral permission names (tauri-runtime-cef's PermissionKind). */
const LABELS = {
  microphone: "Your microphone",
  camera: "Your camera",
  "screen-capture": "Your screen",
};

const render = (state) => {
  document.getElementById("site").textContent = state.site;
  document.getElementById("origin").textContent = state.origin;
  const list = document.getElementById("permissions");
  list.replaceChildren(
    ...state.permissions.map((permission) => {
      const item = document.createElement("li");
      item.textContent = LABELS[permission] ?? permission;
      return item;
    }),
  );
};

// The request can die without the user: it times out (30s in the runtime), or
// the webview that asked closes. The shell then reports no pending request and
// closes this window — so poll rather than assume the answer is still wanted.
const poll = async () => {
  try {
    const state = await invoke("permission_prompt_state");
    if (state) render(state);
  } catch (error) {
    console.error("permission prompt state", error);
  }
};

const answer = async (allow, session) => {
  for (const button of document.querySelectorAll("button")) button.disabled = true;
  try {
    await invoke("permission_prompt_decide", { allow, session });
  } catch (error) {
    console.error("permission prompt decide", error);
    for (const button of document.querySelectorAll("button")) button.disabled = false;
  }
};

document.getElementById("allow-session").addEventListener("click", () => void answer(true, true));
document.getElementById("allow-once").addEventListener("click", () => void answer(true, false));
document.getElementById("deny").addEventListener("click", () => void answer(false, true));

void poll();
setInterval(() => void poll(), 500);

