import { test } from "node:test";
import assert from "node:assert/strict";
import { Emitter } from "../src/emitter.ts";

type Events = { data: [number]; done: [] };

test("on and emit keep registration order", () => {
  const e = new Emitter<Events>();
  const seen: string[] = [];
  e.on("data", (n) => seen.push(`a${n}`));
  e.on("data", (n) => seen.push(`b${n}`));
  assert.equal(e.emit("data", 1), 2);
  assert.deepEqual(seen, ["a1", "b1"]);
});

test("once runs a single time", () => {
  const e = new Emitter<Events>();
  let calls = 0;
  e.once("done", () => calls++);
  e.emit("done");
  e.emit("done");
  assert.equal(calls, 1);
  assert.equal(e.listenerCount("done"), 0);
});

test("off removes every registration of a function", () => {
  const e = new Emitter<Events>();
  const fn = () => {};
  e.on("done", fn);
  e.once("done", fn);
  assert.equal(e.listenerCount("done"), 2);
  assert.equal(e.off("done", fn), 2);
  assert.equal(e.emit("done"), 0);
});

test("a listener removed during emit still runs in that emit", () => {
  const e = new Emitter<Events>();
  const seen: string[] = [];
  let offB = () => {};
  e.on("done", () => {
    seen.push("a");
    offB();
  });
  offB = e.on("done", () => seen.push("b"));
  e.emit("done");
  e.emit("done");
  assert.deepEqual(seen, ["a", "b", "a"]);
});
