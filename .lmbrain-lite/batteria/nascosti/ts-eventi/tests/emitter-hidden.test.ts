import { test } from "node:test";
import assert from "node:assert/strict";
import { Emitter } from "../src/emitter.ts";

type Events = { data: [number]; done: []; other: [string] };

test("hidden: listeners added during emit wait for the next emit", () => {
  const e = new Emitter<Events>();
  const seen: string[] = [];
  e.on("done", () => {
    seen.push("a");
    e.on("done", () => seen.push("late"));
  });
  assert.equal(e.emit("done"), 1);
  assert.deepEqual(seen, ["a"]);
  assert.equal(e.emit("done"), 2);
});

test("hidden: once is removed before its call, so re-entrant emit does not repeat it", () => {
  const e = new Emitter<Events>();
  let calls = 0;
  e.once("done", () => {
    calls++;
    e.emit("done");
  });
  e.emit("done");
  assert.equal(calls, 1);
});

test("hidden: once unsubscribe and argument passing", () => {
  const e = new Emitter<Events>();
  const got: number[] = [];
  const cancel = e.once("data", (n) => got.push(n));
  const keep = e.once("data", (n) => got.push(n * 10));
  cancel();
  assert.equal(e.emit("data", 3), 1);
  assert.deepEqual(got, [30]);
  keep();
  assert.equal(e.listenerCount("data"), 0);
});

test("hidden: off only touches the given event and function", () => {
  const e = new Emitter<Events>();
  const f = () => {};
  const g = () => {};
  e.on("done", f);
  e.on("done", g);
  e.on("done", f);
  e.on("other", f);
  assert.equal(e.off("done", f), 2);
  assert.equal(e.listenerCount("done"), 1);
  assert.equal(e.listenerCount("other"), 1);
  assert.equal(e.off("data", f), 0);
  assert.equal(e.listenerCount("data"), 0);
});

test("hidden: a once removed by off during emit of an earlier listener still runs in that emit", () => {
  const e = new Emitter<Events>();
  const seen: string[] = [];
  const f = () => seen.push("f");
  e.on("done", () => {
    seen.push("first");
    e.off("done", f);
  });
  e.once("done", f);
  assert.equal(e.emit("done"), 2);
  assert.deepEqual(seen, ["first", "f"]);
  assert.equal(e.emit("done"), 1);
});

test("hidden: the same function twice runs twice, and once order is preserved", () => {
  const e = new Emitter<Events>();
  const seen: number[] = [];
  const f = (n: number) => seen.push(n);
  e.on("data", f);
  e.once("data", (n) => seen.push(-n));
  e.on("data", f);
  e.emit("data", 1);
  e.emit("data", 2);
  assert.deepEqual(seen, [1, -1, 1, 2, 2]);
});
