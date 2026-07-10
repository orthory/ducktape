import { fireEvent, render, screen } from "@testing-library/react";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";

import type { ConsoleActions } from "../../store/actions";
import { ConsoleContext } from "../../store/context";
import {
  DEFAULT_NOTIFY_PREFS,
  createInitialState,
  type ConsoleState,
  type NotifyPrefs,
} from "../../store/state";
import { PreferencesSection } from "./PreferencesSection";

const renderPreferences = (patch: Partial<ConsoleState> = {}) => {
  const initialState: ConsoleState = {
    ...createInitialState(),
    notifyPrefs: {
      ...DEFAULT_NOTIFY_PREFS,
      mutedChannels: [...DEFAULT_NOTIFY_PREFS.mutedChannels],
    },
    ...patch,
  };
  const setNotifyPrefs = vi.fn<(prefs: NotifyPrefs) => void>();
  const toggleChannelMute = vi.fn<(channelId: string) => void>();

  function Harness() {
    const [state, setState] = useState(initialState);
    const actions = {
      setNotifyPrefs: (prefs: NotifyPrefs) => {
        setNotifyPrefs(prefs);
        setState((previous) => ({ ...previous, notifyPrefs: prefs }));
      },
      toggleChannelMute: (channelId: string) => {
        toggleChannelMute(channelId);
        setState((previous) => {
          const prefs = previous.notifyPrefs;
          return {
            ...previous,
            notifyPrefs: {
              ...prefs,
              mutedChannels: prefs.mutedChannels.includes(channelId)
                ? prefs.mutedChannels.filter((id) => id !== channelId)
                : [...prefs.mutedChannels, channelId],
            },
          };
        });
      },
    } as unknown as ConsoleActions;

    return (
      <ConsoleContext.Provider value={{ state, actions }}>
        <PreferencesSection />
      </ConsoleContext.Provider>
    );
  }

  render(<Harness />);

  return { setNotifyPrefs, toggleChannelMute };
};

describe("PreferencesSection notifications", () => {
  it("flips the master notification preference with a fresh prefs object", () => {
    const notifyPrefs: NotifyPrefs = {
      ...DEFAULT_NOTIFY_PREFS,
      mutedChannels: ["quiet"],
    };
    const { setNotifyPrefs } = renderPreferences({ notifyPrefs });

    const master = screen.getByRole("switch", {
      name: "Toggle all notifications",
    });
    expect(master).toHaveAttribute("aria-checked", "true");

    fireEvent.click(master);

    expect(setNotifyPrefs).toHaveBeenCalledWith({
      ...notifyPrefs,
      enabled: false,
    });
    expect(setNotifyPrefs.mock.calls[0]?.[0]).not.toBe(notifyPrefs);
    expect(master).toHaveAttribute("aria-checked", "false");
  });

  it("flips only the selected notification category", () => {
    const notifyPrefs: NotifyPrefs = {
      ...DEFAULT_NOTIFY_PREFS,
      replies: false,
      mutedChannels: ["quiet"],
    };
    const { setNotifyPrefs } = renderPreferences({ notifyPrefs });

    fireEvent.click(
      screen.getByRole("switch", { name: "Toggle Huddles notifications" }),
    );

    expect(setNotifyPrefs).toHaveBeenCalledWith({
      ...notifyPrefs,
      huddles: false,
    });
    expect(setNotifyPrefs.mock.calls[0]?.[0]).not.toBe(notifyPrefs);
  });

  it("disables category toggles while notifications are off", () => {
    const notifyPrefs: NotifyPrefs = {
      ...DEFAULT_NOTIFY_PREFS,
      enabled: false,
      huddles: true,
      mutedChannels: [],
    };
    const { setNotifyPrefs } = renderPreferences({ notifyPrefs });
    const huddles = screen.getByRole("switch", {
      name: "Toggle Huddles notifications",
    });

    expect(huddles).toBeDisabled();
    fireEvent.click(huddles);

    expect(setNotifyPrefs).not.toHaveBeenCalled();
    expect(huddles).toHaveAttribute("aria-checked", "true");
  });

  it("toggles the active channel mute and reflects its muted state", () => {
    const { toggleChannelMute } = renderPreferences({ activeChannel: "general" });

    expect(screen.getByText("Mute #general")).toBeInTheDocument();
    const channel = screen.getByRole("switch", {
      name: "Toggle #general notifications",
    });
    expect(channel).toHaveAttribute("aria-checked", "true");

    fireEvent.click(channel);

    expect(toggleChannelMute).toHaveBeenLastCalledWith("general");
    expect(screen.getByText("Unmute #general")).toBeInTheDocument();
    expect(channel).toHaveAttribute("aria-checked", "false");

    fireEvent.click(channel);

    expect(toggleChannelMute).toHaveBeenCalledTimes(2);
    expect(toggleChannelMute).toHaveBeenLastCalledWith("general");
    expect(screen.getByText("Mute #general")).toBeInTheDocument();
    expect(channel).toHaveAttribute("aria-checked", "true");
  });
});
