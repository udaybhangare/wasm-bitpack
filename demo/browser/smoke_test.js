const m = require("./pkg-node-test/wasm_bitpack_browser_demo.js");

for (const numBits of [1, 11, 16, 32]) {
  const blockCount = 50;
  const compressed = m.generate_and_pack(numBits, blockCount);
  const cs = m.decode_scalar(compressed, blockCount, numBits);
  const cw = m.decode_wasm128(compressed, blockCount, numBits);
  console.log(`num_bits=${numBits} scalar=${cs >>> 0} wasm128=${cw >>> 0} match=${cs === cw}`);
  if (cs !== cw) process.exitCode = 1;
}
