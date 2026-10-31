import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import type { UpdateChannel, UpdateInfo } from "../types";

const DISMISS_KEY = "pill-dismissed-update";

/** One check per app run, shared by every component that asks for it. */
let pending: Promise<UpdateInfo | null> | null = null;
let settled = false;
let resolved: UpdateInfo | null = null;

/** The channel is fixed for the lifetime of the process. */
let channelPending: Promise<UpdateChannel | null> | null = null;

/**
 * Where this copy gets its updates from.
 *
 * The backend answers this at runtime (MSIX package identity, then the build
 * marker), so no component ever branches on "which build is this".
 */
export function getUpdateChannel(): Promise<UpdateChannel | null> {
  if (!channelPending) {
    channelPending = invoke<UpdateChannel>("update_channel").catch(() => null);
  }
  return channelPending;
}

export function checkForUpdate(): Promise<UpdateInfo | null> {
  if (!pending) {
    pending = invoke<UpdateInfo | null>("check_for_update")
      .catch(() => null)
      .then((info) => {
        resolved = info;
        settled = true;
        return info;
      });
  }
  return pending;
}

function dismissedVersion(): string {
  try {
    return localStorage.getItem(DISMISS_KEY) ?? "";
  } catch {
    return "";
  }
}

/**
 * Returns the newer release, if GitHub has one the user has not dismissed.
 *
 * The request is delayed so it never competes with start-up, runs at most once
 * per launch, and a dismissed version stays hidden until a newer one appears.
 */
export function useUpdateNotice() {
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    let disposed = false;
    const apply = (info: UpdateInfo | null) => {
      if (disposed || !info) return;
      if (info.latest_version === dismissedVersion()) return;
      setUpdate(info);
    };

    // The panel is opened and closed all the time; once the answer is known it
    // is shown immediately, otherwise the request waits for the island to settle.
    if (settled) {
      apply(resolved);
      return () => {
        disposed = true;
      };
    }

    const timer = window.setTimeout(() => {
      void checkForUpdate().then(apply);
    }, 5000);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
    };
  }, []);

  const dismiss = () => {
    if (!update) return;
    try {
      localStorage.setItem(DISMISS_KEY, update.latest_version);
    } catch {
      // Storage can be unavailable; the notice just reappears next launch.
    }
    setUpdate(null);
  };

  return { update, dismiss };
}

/** The install channel, for surfaces that describe how updates arrive. */
export function useUpdateChannel() {
  const [channel, setChannel] = useState<UpdateChannel | null>(null);

  useEffect(() => {
    let disposed = false;
    void getUpdateChannel().then((value) => {
      if (!disposed) setChannel(value);
    });
    return () => {
      disposed = true;
    };
  }, []);

  return channel;
}
