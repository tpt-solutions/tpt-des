# tpt-des-wasm

WebAssembly bindings for [`tpt-des`](../tpt-des/README.md), with a static browser playground. Use it to run a queue simulation, check it against theory, or size a station from JavaScript, with no server.

This crate is not published. Build it from the workspace.

## Functions

Each function takes plain numbers and returns a JSON string, so a page needs no types from this crate:

| Function | Arguments | Returns |
|---|---|---|
| `run_queue_json(lambda, mu, servers, horizon, warmup, replications, seed)` | rates, server count, horizon, warm-up, replication count, `u32` seed | Mean `wq`, `w`, `l`, and the `replications` count. |
| `validate_mmc_json(lambda, mu, servers, horizon, warmup, replications, seed)` | as above | The report rows, with a `passes` flag. |
| `size_json(lambda, mu, max_wait)` | rates and the wait target | The fewest servers whose analytic mean wait is at most `max_wait`. |

Bad input returns `{"error": "..."}`. A function never panics, because a panic in WebAssembly aborts the page.

Example, from JavaScript:

```js
import init, { run_queue_json } from "./pkg/tpt_des_wasm.js";

await init();
const result = JSON.parse(run_queue_json(2.4, 1.0, 3, 10000, 1000, 6, 3));
console.log(result.wq, result.w, result.l);
```

## Build the playground

You need `wasm-bindgen-cli` at the same version as the crate's `wasm-bindgen` dependency:

```sh
cargo build -p tpt-des-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/tpt_des_wasm.wasm \
  --target web --out-dir crates/tpt-des-wasm/playground/pkg
```

Then serve `crates/tpt-des-wasm/playground/` over HTTP and open `index.html`. Browsers do not load WebAssembly modules from `file://`, so a plain double-click will not work.

Any static file server works, for example `python -m http.server` run from that directory.

## Notes and limits

- The seed is a `u32`, so JavaScript passes it as an ordinary number.
- Results depend on the seed, as they do in native code. The same arguments give the same JSON.
- The bindings were exercised under Node. The playground page has not been run in a browser from CI, so check it locally after a change.

## Testing

```sh
cargo build -p tpt-des-wasm --target wasm32-unknown-unknown
```

CI builds the crate for `wasm32-unknown-unknown`.

## Changelog and contributing

See [CHANGELOG.md](CHANGELOG.md) for release notes. Contributions are taken as issues only. See [CONTRIBUTING.md](../../CONTRIBUTING.md).

---

Part of the [tpt-des](../../README.md) workspace. Licensed under MIT OR Apache-2.0.
