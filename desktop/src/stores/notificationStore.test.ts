import { beforeEach, describe, expect, it } from "vitest";
import { useNotificationStore } from "./notificationStore";
import type { Notification } from "../types";

function at(id: string, timestamp: number): Notification {
  return {
    id,
    appName: "Messages",
    packageName: "com.example.messages",
    sender: "A friend",
    message: "hello",
    timestamp,
    receivedAt: timestamp,
    status: "NEW",
    priority: 0,
    contentHidden: false,
  };
}

describe("the notification inbox", () => {
  beforeEach(() => {
    useNotificationStore.getState().clear();
  });

  it("shows yesterday's stored notifications alongside today's", () => {
    // The history is loaded when a phone attaches. It used to be loaded on
    // mount, before the vault was open, so the backend refused it and older
    // notifications never came back at all.
    const store = useNotificationStore.getState();
    store.upsert(at("today", 3_000));
    useNotificationStore.getState().mergeHistory([at("today", 3_000), at("yesterday", 1_000)]);

    expect(useNotificationStore.getState().items.map((it) => it.id)).toEqual([
      "today",
      "yesterday",
    ]);
  });

  it("does not let the stored copy overwrite one that just arrived", () => {
    const store = useNotificationStore.getState();
    store.upsert({ ...at("same", 2_000), status: "IMPORTANT" });
    useNotificationStore.getState().mergeHistory([at("same", 2_000)]);

    const items = useNotificationStore.getState().items;
    expect(items).toHaveLength(1);
    expect(items[0].status).toBe("IMPORTANT");
  });

  it("orders everything newest first, however it arrived", () => {
    const store = useNotificationStore.getState();
    store.upsert(at("older", 1_000));
    useNotificationStore.getState().mergeHistory([at("newest", 9_000), at("middle", 5_000)]);

    expect(useNotificationStore.getState().items.map((it) => it.id)).toEqual([
      "newest",
      "middle",
      "older",
    ]);
  });
});
