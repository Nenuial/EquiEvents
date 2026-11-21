// Boot Rosson's own renderer code headless, so its scoring and ranking services can be
// called on real data: the "oracle" that tells us what Rosson 3.3.15 computes.
//
// - Loads the built bundle (out/js/*.js) from vendor/rosson-<version>/ into jsdom.
// - IndexedDB is fake-indexeddb, loaded *inside* the page (in memory, fresh per boot), so the
//   objects it hands back belong to the page like in a browser. Rosson relies on that:
//   it tests `x instanceof Array`, `x instanceof Date` and `x.constructor === Array`.
// - Electron/Node modules the renderer `require`s are replaced by inert stubs; real
//   libraries Rosson bundles (xlsx, jsonlint) are loaded from its own node_modules.
// - No UI is rendered: we create an Angular injector for the 'RossonApp' module and call
//   services directly, replaying what the controllers do (see recompute.mjs).

import { existsSync, readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { dirname, join } from 'node:path';
import { JSDOM } from 'jsdom';

const nodeRequire = createRequire(import.meta.url);
const fakeIndexedDbDir = dirname(nodeRequire.resolve('fake-indexeddb')); // build/cjs

// structuredClone for the page, written as page code so clones are page objects.
// Covers what Rosson stores (plain data, Dates); like the real one it rejects functions.
const PAGE_STRUCTURED_CLONE = `(function () {
  var toString = Object.prototype.toString;
  window.structuredClone = function structuredClone(value) {
    var seen = new Map();
    function clone(v) {
      if (typeof v === 'function' || typeof v === 'symbol') throw new DOMException('value could not be cloned', 'DataCloneError');
      if (v === null || typeof v !== 'object') return v;
      if (seen.has(v)) return seen.get(v);
      var tag = toString.call(v), out;
      if (tag === '[object Date]') { out = new Date(v.getTime()); seen.set(v, out); return out; }
      if (tag === '[object RegExp]') { out = new RegExp(v.source, v.flags); seen.set(v, out); return out; }
      if (Array.isArray(v)) { out = []; seen.set(v, out); for (var i = 0; i < v.length; i++) out[i] = clone(v[i]); return out; }
      if (tag === '[object Map]') { out = new Map(); seen.set(v, out); v.forEach(function (x, k) { out.set(clone(k), clone(x)); }); return out; }
      if (tag === '[object Set]') { out = new Set(); seen.set(v, out); v.forEach(function (x) { out.add(clone(x)); }); return out; }
      out = {}; seen.set(v, out);
      Object.keys(v).forEach(function (k) { out[k] = clone(v[k]); });
      return out;
    }
    return clone(value);
  };
})();`;

/** CommonJS loader that evaluates modules inside the page (for fake-indexeddb). */
function pageRequire(w, baseDir) {
  const cache = new Map();
  const resolve = (dir, spec) => {
    if (!spec.startsWith('.')) throw new Error(`page require: unsupported module ${spec}`);
    const p = join(dir, spec);
    if (existsSync(`${p}.js`)) return `${p}.js`;
    if (existsSync(join(p, 'index.js'))) return join(p, 'index.js');
    return p;
  };
  const load = file => {
    if (cache.has(file)) return cache.get(file).exports;
    const module = { exports: {} };
    cache.set(file, module);
    w.Function('module', 'exports', 'require', readFileSync(file, 'utf8'))
      .call(module.exports, module, module.exports, spec => load(resolve(dirname(file), spec)));
    return module.exports;
  };
  return spec => load(resolve(baseDir, spec));
}

export function bootRosson(vendorDir, { log = () => {} } = {}) {
  const pkg = JSON.parse(readFileSync(join(vendorDir, 'package.json'), 'utf8'));
  const outJs = join(vendorDir, 'out/js');

  const dom = new JSDOM('<!doctype html><html><body><div id="oracle"></div></body></html>', {
    url: 'https://rosson.oracle.invalid/index.html', // http(s) origin: jsdom refuses localStorage for file://
    runScripts: 'outside-only',
    pretendToBeVisual: true,
  });
  const w = dom.window;

  w.eval(PAGE_STRUCTURED_CLONE);
  // fake-indexeddb schedules its callbacks with setImmediate. In jsdom it tries to borrow
  // Node's through a sandbox leak (`Node.constructor`) that jsdom 30 no longer has, and every
  // callback would be lost. Forward to Node's setImmediate explicitly.
  w.setImmediate = (fn, ...args) => setImmediate(fn, ...args);
  w.clearImmediate = handle => clearImmediate(handle); // pdfmake's bundled polyfills expect the pair
  const fakeIndexedDb = pageRequire(w, fakeIndexedDbDir)('./index.js');
  w.indexedDB = new fakeIndexedDb.IDBFactory();
  w.IDBKeyRange = fakeIndexedDb.IDBKeyRange;

  w.process = { env: { NODE_ENV: 'development' }, platform: 'darwin', versions: {} }; // dev: no Rollbar
  w.matchMedia ||= () => ({ matches: false, addListener() {}, removeListener() {}, addEventListener() {}, removeEventListener() {} });
  w.Worker = class { postMessage() {} terminate() {} }; // print workers are not used
  // StorageFactory exports `find: find` without defining it; in Chromium that resolves to the
  // browser's window.find() (find text in page). Never called, but must exist.
  w.find = () => false;
  w.console = { ...console, log: (...a) => log('log', ...a), warn: (...a) => log('warn', ...a), error: (...a) => log('error', ...a), info: () => {}, debug: () => {}, dir: () => {} };

  const vendorRequire = createRequire(join(vendorDir, 'package.json'));
  const files = new Map();
  const ipcRenderer = {
    send() {}, on() {}, once() {}, removeListener() {},
    invoke: async () => ({}),
    sendSync: channel => (channel === 'version' ? pkg.version : undefined),
  };
  const stubs = {
    electron: { ipcRenderer, shell: { openPath() {} } },
    '@electron/remote': {},
    // Files Rosson writes are captured in `files` (path → bytes) instead of touching the disk.
    fs: {
      writeFile: (path, data, opts, cb) => { files.set(path, Buffer.from(data)); (typeof opts === 'function' ? opts : cb)?.(null); },
      writeFileSync: (path, data) => { files.set(path, Buffer.from(data)); },
      existsSync: () => false, readFileSync: () => '',
    },
    macaddress: { one: cb => cb(null, '00:00:00:00:00:00') },
  };
  // Files Rosson loads with require('./js/...') are plain scripts next to rosson.js.
  const localScript = name => {
    const module = { exports: {} };
    const file = join(outJs, name.replace(/^\.\/js\//, '').replace(/(\.js)?$/, '.js'));
    // Like Node: top-level `this` is module.exports (encoding-indexes.js relies on it).
    w.Function('module', 'exports', 'require', 'global', readFileSync(file, 'utf8'))
      .call(module.exports, module, module.exports, w.require, w);
    return module.exports;
  };
  w.require = name => {
    if (name in stubs) return stubs[name];
    // UMD wrappers of Angular plugins ask for the page's Angular (and its sub-modules).
    if (name === 'angular' || name.startsWith('angular-')) return w.angular;
    if (name.startsWith('./js/')) return localScript(name);
    return vendorRequire(name);
  };
  // With nodeIntegration, Electron renderers also have CommonJS globals; the shipped bundle
  // relies on that (it ends a debug helper with `module.exports = DebugStore`).
  w.module = { exports: {} };
  w.exports = w.module.exports;
  w.Buffer = Buffer; // also a Node global in Electron renderers (used by the start-list export)

  for (const file of ['vendor.min.js', 'libs.js', 'templates.js', 'rosson.js']) {
    w.eval(readFileSync(join(outJs, file), 'utf8'));
  }

  // The shipped rosson.js wrongly contains the print web-worker scripts (gulpfile.js lists its
  // exclusions before the includes, so gulp ignores them). Their global `onmessage` handler
  // then fires on every window message and calls importScripts, which only exists in workers.
  w.onmessage = null;

  const angular = w.angular;
  angular.module('oracleShim', []).value('$rootElement', angular.element(w.document.getElementById('oracle')));
  const injector = angular.injector(['ng', 'RossonApp', 'oracleShim']);

  // Angular resolves $q promises in a digest scheduled with the page's setTimeout, and jsdom's
  // timers do not keep Node's event loop alive. Hold it open until close().
  const keepAlive = setInterval(() => {}, 1 << 30);

  return {
    version: pkg.version,
    window: w,
    angular,
    get: name => injector.get(name),
    // Data we hand to Rosson must be page objects too (same reason as above).
    toPage: value => w.JSON.parse(JSON.stringify(value)),
    fromPage: value => JSON.parse(w.JSON.stringify(value)),
    files,
    close: () => { clearInterval(keepAlive); w.close(); },
  };
}
