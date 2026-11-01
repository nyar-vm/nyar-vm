//! 最小浏览器启动脚本（无框架 runtime；逻辑在 WASM，JS 仅加载与绑定）。
//!
//! 页面 HTML 只允许 `<script src="boot.js">`；`start(manifest)` 由本文件自动执行，禁止内联业务 JS。
//!
//! 运行时契约（与 nyar-emitter `wasm_js_glue` 的 Node 壳同构，浏览器自包含版）：
//! - wasm 导入面为 module `env`，field 为点分名（`__voa.*` / `document.*` / `Element.*`）与合成原语
//!   （`const_utf8` / `utf8_*` / `host_intern_utf8` / `host_utf8_extract`）。
//! - utf8 跨界面为 i32 句柄：宿主维护 `hostValues` 表（index ↔ JS string）；字符串字面量表在
//!   wasm custom section `nyar.strings`，`const_utf8(index)` 把字面量登记进句柄表。
//! - DOM 节点经句柄表进出：`document.createElement` 返回句柄，`Element.*` 原语按句柄操作真实节点。
//! - 响应式状态驻留宿主信号表，按名寻址（`__voa.sigNamed`）：生成源码没有模块级 `let`，
//!   wasm 侧一切状态读写都经 `sig_named(key)` 取回稳定句柄 id。

/// Manifest URL baked into auto-start (`manifest.json` or `/manifest.json`).
pub fn manifest_url_for_mode(relative: bool) -> &'static str {
    if relative { "manifest.json" } else { "/manifest.json" }
}

/// Markup-only script tag for boot.js (no inline code).
pub fn asgard_boot_script_tag(relative: bool) -> &'static str {
    if relative { r#"<script src="boot.js"></script>"# } else { r#"<script src="/boot.js"></script>"# }
}

/// Markup-only stylesheet link.
pub fn asgard_boot_stylesheet_tag(css_href: &str) -> String {
    format!(r#"<link rel="stylesheet" href="{css_href}">"#)
}

/// 生成 `boot.js`：自 fetch WASM 字节、自实现 `env` 运行时（utf8 句柄 + DOM 句柄 + 信号表），
/// 按 manifest 加载组件胶水并挂载 island。
pub fn generate_boot_script(wasm_stem: &str, manifest_url: &str) -> String {
    BOOT_JS_TEMPLATE.replace("__VOA_MANIFEST_URL__", manifest_url.trim()).replace("__VOA_WASM_STEM__", wasm_stem.trim())
}

const BOOT_JS_TEMPLATE: &str = r##"(function (global) {
  'use strict';

  var MANIFEST_URL = '__VOA_MANIFEST_URL__';
  var WASM_STEM = '__VOA_WASM_STEM__';

  // ── utf8 句柄表（index ↔ JS string）──────────────────────────────
  var hostValues = [''];
  var hostValueIds = new Map([['', 0]]);
  var UTF8_LITERALS = [];

  function hostIntern(value) {
    var text = String(value == null ? '' : value);
    var known = hostValueIds.get(text);
    if (known !== undefined) return known;
    var handle = hostValues.length;
    hostValues.push(text);
    hostValueIds.set(text, handle);
    return handle;
  }

  function hostResolve(handle) {
    return (Number.isInteger(handle) && handle >= 0 && handle < hostValues.length) ? hostValues[handle] : '';
  }

  function hostText(ref) {
    return typeof ref === 'string' ? ref : hostResolve(ref);
  }

  // ── wasm 线性内存访问（memory.grow 后必须重取 buffer）────────────
  function hostLinearMemory() {
    var memory = wasmExports ? wasmExports.memory : undefined;
    if (!memory) throw new Error('[boot] wasm module does not export memory');
    return memory;
  }

  function hostInternUtf8Bytes(ptr, len) {
    var bytes = new Uint8Array(hostLinearMemory().buffer, ptr >>> 0, len >>> 0);
    return hostIntern(new TextDecoder('utf-8').decode(bytes));
  }

  function hostUtf8Extract(handle) {
    var bytes = new TextEncoder().encode(hostResolve(handle));
    var ptr = Number(wasmExports.cabi_realloc(0, 0, 1, bytes.length));
    new Uint8Array(hostLinearMemory().buffer, ptr >>> 0, bytes.length).set(bytes);
    return [ptr, bytes.length];
  }

  // ── utf8 标量语义（Unicode scalars，非 UTF-16 code units）────────
  function hostUtf8Scalars(s) { return [...hostText(s)]; }
  function hostUtf8Concat(a, b) { return hostIntern(hostText(a) + hostText(b)); }
  function hostUtf8Length(s) { return hostUtf8Scalars(s).length; }
  function hostUtf8Trim(s) { return hostIntern(hostText(s).trim()); }
  function hostUtf8ToLower(s) { return hostIntern(hostText(s).toLowerCase()); }
  function hostUtf8ToUpper(s) { return hostIntern(hostText(s).toUpperCase()); }
  function hostUtf8Replace(s, oldValue, newValue) {
    var needle = hostText(oldValue);
    if (hostUtf8Scalars(needle).length === 0) return hostIntern(hostText(s));
    return hostIntern(hostText(s).split(needle).join(hostText(newValue)));
  }
  function hostUtf8StartsWith(s, prefix) { return hostText(s).startsWith(hostText(prefix)) ? 1 : 0; }
  function hostUtf8EndsWith(s, suffix) { return hostText(s).endsWith(hostText(suffix)) ? 1 : 0; }
  function hostUtf8Contains(s, other) { return hostText(s).includes(hostText(other)) ? 1 : 0; }
  function hostUtf8Equals(a, b) { return hostText(a) === hostText(b) ? 1 : 0; }
  function hostUtf8IndexOf(s, other) {
    var chars = hostUtf8Scalars(s);
    var needle = hostUtf8Scalars(other);
    if (needle.length === 0) return 0;
    if (needle.length > chars.length) return -1;
    for (var i = 0; i <= chars.length - needle.length; i++) {
      var ok = true;
      for (var j = 0; j < needle.length; j++) {
        if (chars[i + j] !== needle[j]) { ok = false; break; }
      }
      if (ok) return i;
    }
    return -1;
  }
  function hostUtf8Slice(s, start, count) {
    var chars = hostUtf8Scalars(s);
    var from = Math.max(0, start | 0);
    var len = Math.max(0, count | 0);
    return hostIntern(chars.slice(from, from + len).join(''));
  }

  // ── DOM 句柄表 ───────────────────────────────────────────────────
  var domHandles = [null];

  function storeDomHandle(node) {
    var id = domHandles.length;
    domHandles.push(node);
    return id;
  }

  function getDomHandle(id) {
    return domHandles[id] || null;
  }

  // ── 信号表（按名寻址；响应式状态的唯一事实源）────────────────────
  var signals = [];
  var signalSubscribers = [];
  var namedSignals = new Map();
  var storeSubscribers = [];
  var batchDepth = 0;
  var pendingSignals = new Set();

  function newSignal(value) {
    var id = signals.length;
    signals.push({ value: value });
    signalSubscribers[id] = [];
    return id;
  }

  function sigNamed(name) {
    var key = hostText(name);
    var existing = namedSignals.get(key);
    if (existing !== undefined) return existing;
    var id = newSignal(0);
    namedSignals.set(key, id);
    return id;
  }

  function scheduleNotify(sigId) {
    if (sigId === -2) {
      if (batchDepth > 0) return;
      flushStore();
      return;
    }
    if (batchDepth > 0) {
      pendingSignals.add(sigId);
      return;
    }
    runSignalSubscribers(sigId);
  }

  function runSignalSubscribers(sigId) {
    var subs = signalSubscribers[sigId];
    if (!subs) return;
    subs.slice().forEach(function (fn) { fn(); });
  }

  function flushStore() {
    storeSubscribers.slice().forEach(function (fn) { fn(); });
  }

  function flushBatch() {
    pendingSignals.forEach(function (sigId) { runSignalSubscribers(sigId); });
    pendingSignals.clear();
    flushStore();
  }

  function sigSet(id, value) {
    if (!signals[id]) return;
    signals[id].value = value;
    scheduleNotify(id);
  }

  // ── 订阅与表达式绑定 ─────────────────────────────────────────────
  function parseDeps(csv, sigIds) {
    if (!csv) return sigIds || [];
    if (csv === 'store') return ['store'];
    return csv.split(',').map(function (part) { return part.trim(); }).filter(Boolean);
  }

  function resolveSigRef(name) {
    if (typeof name === 'number') return name;
    var parsed = parseInt(name, 10);
    return isNaN(parsed) ? -1 : parsed;
  }

  function subscribeDeps(depIds, fn) {
    var deps = (depIds || []).filter(function (dep) { return dep !== null && dep !== undefined && typeof dep === 'number' && dep >= 0; });
    var storeBound = depIds && depIds.indexOf(-2) >= 0;
    if (storeBound) storeSubscribers.push(fn);
    if (deps.length === 0 && !storeBound) {
      fn();
      return;
    }
    deps.forEach(function (depId) {
      signalSubscribers[depId] = signalSubscribers[depId] || [];
      signalSubscribers[depId].push(fn);
    });
    // 激活期内建立的订阅登记到当前 island 作用域，`unmountIslands` 逐条解绑。
    registerCleanup(function () {
      deps.forEach(function (depId) {
        removeFromList(signalSubscribers[depId], fn);
      });
      if (storeBound) removeFromList(storeSubscribers, fn);
    });
    fn();
  }

  function removeFromList(list, item) {
    if (!list) return;
    var index = list.indexOf(item);
    if (index >= 0) list.splice(index, 1);
  }

  function callWasm(name) {
    if (!wasmExports || typeof wasmExports[name] !== 'function') {
      throw new Error('WASM export not found: ' + name);
    }
    return wasmExports[name].apply(null, Array.prototype.slice.call(arguments, 1));
  }

  function bindExpr(handle, exprExport, dep1, dep2, apply, resolveUtf8) {
    var update = function () {
      var raw = callWasm(exprExport);
      var value = resolveUtf8 ? hostResolve(raw) : raw;
      if (handle === null || handle === undefined) {
        apply(null, value);
        return;
      }
      var node = getDomHandle(handle);
      if (node) apply(node, value);
    };
    subscribeDeps([dep1, dep2], update);
  }

  // ── wasm 实例与组件注册 ──────────────────────────────────────────
  var wasmExports = null;
  var componentFactories = new Map();
  var eventBindings = new Map();
  // 每个 island 的挂载作用域：激活期内建立的订阅、监听与 memo 都登记在这里，
  // `unmountIslands` 用同一份清单解绑，重载后不会留下指向旧 DOM 的回调。
  var mountScopes = [];

  function registerCleanup(fn) {
    if (mountScopes.length === 0) return;
    mountScopes[mountScopes.length - 1].push(fn);
  }

  function isLoaded() {
    return wasmExports !== null;
  }

  function registerComponent(name, factory) {
    componentFactories.set(name, factory);
  }

  // ── env：wasm 导入面（点分名，与 codegen prelude 一一对应）───────
  var env = {
    const_utf8: function (index) { return hostIntern(UTF8_LITERALS[index] ?? ''); },
    host_intern_utf8: hostInternUtf8Bytes,
    host_utf8_extract: hostUtf8Extract,
    utf8_concat: hostUtf8Concat,
    utf8_length: hostUtf8Length,
    utf8_trim: hostUtf8Trim,
    utf8_to_lower: hostUtf8ToLower,
    utf8_to_upper: hostUtf8ToUpper,
    utf8_replace: hostUtf8Replace,
    utf8_starts_with: hostUtf8StartsWith,
    utf8_ends_with: hostUtf8EndsWith,
    utf8_contains: hostUtf8Contains,
    utf8_equals: hostUtf8Equals,
    utf8_index_of: hostUtf8IndexOf,
    utf8_slice: hostUtf8Slice,

    'document.createElement': function (tag) { return storeDomHandle(document.createElement(hostText(tag))); },
    'document.createTextNode': function (text) { return storeDomHandle(document.createTextNode(hostText(text))); },
    'Element.setAttribute': function (handle, name, value) {
      var node = getDomHandle(handle);
      if (node) node.setAttribute(hostText(name), hostText(value));
    },
    'Element.appendChild': function (parent, child) {
      var p = getDomHandle(parent);
      var c = getDomHandle(child);
      if (p && c) p.appendChild(c);
    },
    'Element.textContent': function (handle, text) {
      var node = getDomHandle(handle);
      if (node) node.textContent = hostText(text);
    },
    'Element.classList.add': function (handle, token) {
      var node = getDomHandle(handle);
      if (node && node.classList) node.classList.add(hostText(token));
    },
    'Element.classList.remove': function (handle, token) {
      var node = getDomHandle(handle);
      if (node && node.classList) node.classList.remove(hostText(token));
    },
    'Element.classList.toggle': function (handle, token) {
      var node = getDomHandle(handle);
      if (node && node.classList) node.classList.toggle(hostText(token));
    },
    'Element.addEventListener': function (handle, event, callback) {
      var node = getDomHandle(handle);
      if (node) node.addEventListener(hostText(event), function () {});
    },
    'Element.removeEventListener': function () {},
    'Element.removeChild': function (parent, child) {
      var p = getDomHandle(parent);
      var c = getDomHandle(child);
      if (p && c) p.removeChild(c);
    },
    'Element.insertBefore': function (parent, child, before) {
      var p = getDomHandle(parent);
      var c = getDomHandle(child);
      var b = getDomHandle(before);
      if (p && c) p.insertBefore(c, b || null);
    },
    'Element.replaceChild': function (parent, child, old) {
      var p = getDomHandle(parent);
      var c = getDomHandle(child);
      var o = getDomHandle(old);
      if (p && c && o) p.replaceChild(c, o);
    },

    '__voa.sigNamed': sigNamed,
    '__voa.sigCreateI32': function (v) { return newSignal(v | 0); },
    '__voa.sigGetI32': function (id) { return (signals[id] && signals[id].value) | 0; },
    '__voa.sigSetI32': function (id, v) { sigSet(id, v | 0); },
    '__voa.sigCreateUtf8': function (v) { return newSignal(hostText(v)); },
    '__voa.sigGetUtf8': function (id) { return hostIntern(signals[id] ? String(signals[id].value == null ? '' : signals[id].value) : ''); },
    '__voa.sigSetUtf8': function (id, v) { sigSet(id, hostText(v)); },
    '__voa.sigCreateBool': function (v) { return newSignal(!!v); },
    '__voa.sigGetBool': function (id) { return !!(signals[id] && signals[id].value); },
    '__voa.sigSetBool': function (id, v) { sigSet(id, !!v); },
    '__voa.sigCreateList': function (v) { return newSignal(v); },
    '__voa.sigGetList': function (id) { return signals[id] ? signals[id].value : 0; },
    '__voa.sigSetList': function (id, v) { sigSet(id, v); },
    '__voa.sigSubscribe': function (sigId, depsCsv, updateExport) {
      var deps = parseDeps(hostText(depsCsv), [sigId]);
      var fn = function () {
        if (wasmExports && typeof wasmExports[hostText(updateExport)] === 'function') callWasm(hostText(updateExport));
      };
      var detachers = [];
      deps.forEach(function (dep) {
        if (dep === 'store') {
          storeSubscribers.push(fn);
          detachers.push(function () { removeFromList(storeSubscribers, fn); });
          return;
        }
        var depId = resolveSigRef(dep);
        if (depId >= 0) {
          signalSubscribers[depId] = signalSubscribers[depId] || [];
          signalSubscribers[depId].push(fn);
          detachers.push(function () { removeFromList(signalSubscribers[depId], fn); });
        }
      });
      // 与 `storeSubscribe` 同一生命周期合同：激活期订阅随 island 卸载解绑。
      registerCleanup(function () {
        detachers.forEach(function (detach) { detach(); });
      });
      fn();
    },
    '__voa.rxBindTextUtf8': function (handle, exprExport, dep1, dep2) {
      bindExpr(handle, hostText(exprExport), dep1, dep2, function (node, value) { node.textContent = value; }, true);
    },
    '__voa.rxBindAttrUtf8': function (handle, attr, exprExport, dep1, dep2) {
      bindExpr(handle, hostText(exprExport), dep1, dep2, function (node, value) { node.setAttribute(hostText(attr), value); }, true);
    },
    '__voa.rxBindClassUtf8': function (handle, exprExport, dep1, dep2) {
      bindExpr(handle, hostText(exprExport), dep1, dep2, function (node, value) { node.setAttribute('class', value); }, true);
    },
    '__voa.rxBindValueUtf8': function (handle, exprExport, dep1, dep2) {
      // `value` 是元素属性而非 HTML 属性：写属性才能让输入框在信号变化后同步。
      bindExpr(handle, hostText(exprExport), dep1, dep2, function (node, value) { node.value = value == null ? '' : String(value); }, true);
    },
    '__voa.domBindInputUtf8': function (signalId, handle, updateExport) {
      var node = getDomHandle(handle);
      if (!node) return;
      var name = hostText(updateExport);
      var fn = function () {
        sigSet(signalId, String(node.value == null ? '' : node.value));
        if (name && wasmExports && typeof wasmExports[name] === 'function') callWasm(name);
      };
      node.addEventListener('input', fn);
      registerCleanup(function () { node.removeEventListener('input', fn); });
    },
    '__voa.rxBindPropI32': function (sigId, exprExport, dep1, dep2) {
      bindExpr(null, hostText(exprExport), dep1, dep2, function (_node, value) { sigSet(sigId, value | 0); }, false);
    },
    '__voa.rxBindPropUtf8': function (sigId, exprExport, dep1, dep2) {
      bindExpr(null, hostText(exprExport), dep1, dep2, function (_node, value) { sigSet(sigId, value); }, true);
    },
    '__voa.rxBindPropBool': function (sigId, exprExport, dep1, dep2) {
      bindExpr(null, hostText(exprExport), dep1, dep2, function (_node, value) { sigSet(sigId, !!value); }, false);
    },
    '__voa.rxBindIf': function (container, condExport, mountExport, dep1, dep2) {
      var mounted = false;
      var update = function () {
        var show = callWasm(hostText(condExport));
        var node = getDomHandle(container);
        if (!node) return;
        if (show && !mounted) {
          callWasm(hostText(mountExport), container);
          mounted = true;
        } else if (!show && mounted) {
          node.textContent = '';
          mounted = false;
        }
      };
      subscribeDeps([dep1, dep2], update);
    },
    '__voa.rxBindLoop': function (container, itemsExport, mountExport, keyExport, dep1, dep2) {
      var update = function () {
        var node = getDomHandle(container);
        if (!node) return;
        while (node.firstChild) node.removeChild(node.firstChild);
        var items = callWasm(hostText(itemsExport)) || [];
        if (!Array.isArray(items)) items = [];
        for (var i = 0; i < items.length; i++) {
          callWasm(hostText(mountExport), container, i);
        }
      };
      subscribeDeps([dep1, dep2], update);
    },
    '__voa.rxMemoI32': function (memoId, computeExport, dep1, dep2) {
      return memoCompute(memoId, hostText(computeExport), [dep1, dep2], false);
    },
    '__voa.rxMemoUtf8': function (memoId, computeExport, dep1, dep2) {
      return memoCompute(memoId, hostText(computeExport), [dep1, dep2], true);
    },
    '__voa.componentEmit': function (event) {
      global.dispatchEvent(new CustomEvent('asgard:emit', { detail: { event: hostText(event) } }));
    },
    '__voa.componentEmitUtf8': function (event, arg) {
      global.dispatchEvent(new CustomEvent('asgard:emit', { detail: { event: hostText(event), arg: hostText(arg) } }));
    },
    '__voa.componentEmitI32': function (event, arg) {
      global.dispatchEvent(new CustomEvent('asgard:emit', { detail: { event: hostText(event), arg: arg } }));
    },
    '__voa.domAddEventExportUtf8': function (handle, event, handlerExport, arg) {
      var node = getDomHandle(handle);
      if (!node) return;
      var name = hostText(handlerExport);
      var captured = hostIntern(hostText(arg));
      var fn = function () { if (wasmExports && typeof wasmExports[name] === 'function') callWasm(name, captured); };
      var eventName = hostText(event);
      node.addEventListener(eventName, fn);
      eventBindings.set(name + ':' + hostText(arg), fn);
      registerCleanup(function () { node.removeEventListener(eventName, fn); });
    },
    '__voa.domAddEventExport': function (handle, event, handlerExport) {
      var node = getDomHandle(handle);
      if (!node) return;
      var name = hostText(handlerExport);
      var fn = function () { if (wasmExports && typeof wasmExports[name] === 'function') callWasm(name); };
      var eventName = hostText(event);
      node.addEventListener(eventName, fn);
      eventBindings.set(name, fn);
      registerCleanup(function () { node.removeEventListener(eventName, fn); });
    },
    '__voa.domAddEventExportI32': function (handle, event, handlerExport, arg) {
      var node = getDomHandle(handle);
      if (!node) return;
      var name = hostText(handlerExport);
      var captured = arg | 0;
      var fn = function () { if (wasmExports && typeof wasmExports[name] === 'function') callWasm(name, captured); };
      var eventName = hostText(event);
      node.addEventListener(eventName, fn);
      eventBindings.set(name + ':' + String(captured), fn);
      registerCleanup(function () { node.removeEventListener(eventName, fn); });
    },
    '__voa.mountIslands': function () { mountIslands(); },
    '__voa.unmountIslands': function () { return unmountIslands(); },
    '__voa.rxBatchBegin': function () { batchDepth += 1; },
    '__voa.rxBatchEnd': function () {
      batchDepth = Math.max(0, batchDepth - 1);
      if (batchDepth === 0) flushBatch();
    },
    '__voa.storeBump': function () { scheduleNotify(-2); },
    '__voa.storeSubscribe': function (exportName) {
      var name = hostText(exportName);
      var fn = function () {
        if (wasmExports && typeof wasmExports[name] === 'function') callWasm(name);
      };
      storeSubscribers.push(fn);
      registerCleanup(function () { removeFromList(storeSubscribers, fn); });
    },
    '__voa.styleCollectorPush': function (classes) {
      var text = hostText(classes);
      if (!text) return;
      global.__voaStyleUtilities = global.__voaStyleUtilities || new Set();
      text.split(/\s+/).forEach(function (token) {
        if (token) global.__voaStyleUtilities.add(token);
      });
    }
  };

  // ── memo：按 dep 失效重算 ────────────────────────────────────────
  var memos = new Map();

  function memoCompute(memoId, exportName, depIds, resolveUtf8) {
    var entry = memos.get(memoId);
    if (!entry) {
      entry = { value: resolveUtf8 ? hostIntern('') : 0, exportName: exportName };
      memos.set(memoId, entry);
      var recompute = function () {
        var raw = callWasm(entry.exportName);
        entry.value = resolveUtf8 ? hostIntern(hostResolve(raw)) : raw;
      };
      subscribeDeps(depIds, recompute);
      // memo 缓存与 island 生命周期绑定：卸载后丢弃，重挂载时重新计算。
      registerCleanup(function () { memos.delete(memoId); });
    }
    return entry.value;
  }

  // ── custom section 解析：nyar.strings 字面量表 ───────────────────
  function parseCustomSections(bytes) {
    var pos = 8;
    function leb() {
      var result = 0, shift = 0;
      for (;;) {
        var b = bytes[pos++];
        result |= (b & 0x7f) << shift;
        if (!(b & 0x80)) break;
        shift += 7;
      }
      return result >>> 0;
    }
    while (pos < bytes.length) {
      var id = bytes[pos++];
      var size = leb();
      var end = pos + size;
      if (id === 0) {
        var nlen = leb();
        var name = new TextDecoder().decode(bytes.subarray(pos, pos + nlen));
        pos += nlen;
        if (name === 'nyar.strings') {
          try {
            UTF8_LITERALS = JSON.parse(new TextDecoder().decode(bytes.subarray(pos, end))).strings || [];
          } catch (e) {
            console.error('asgard boot: nyar.strings parse failed', e);
          }
        }
      }
      pos = end;
    }
  }

  // ── island 挂载与卸载 ────────────────────────────────────────────
  function mountIslands() {
    componentFactories.forEach(function (factory, name) {
      var nodes = document.querySelectorAll('[data-component="' + name + '"]');
      nodes.forEach(function (host) {
        if (host.__voaMounted) return;
        var cleanups = [];
        mountScopes.push(cleanups);
        var node;
        try {
          node = factory(host);
        } finally {
          mountScopes.pop();
        }
        if (node && node.nodeType && node !== host) {
          // 静态页面在挂载点内留有预渲染占位内容（无 JS 时的可读回退）；
          // hydration 由 WASM 重建 DOM，追加前清空占位，避免两份 DOM 并存。
          if (host.getAttribute('data-island') === 'hydrated') {
            host.textContent = '';
          }
          host.appendChild(node);
        }
        host.__voaDispose = function () { cleanups.forEach(function (fn) { fn(); }); };
        host.__voaMounted = true;
      });
    });
  }

  // 释放落在 host 子树内的 DOM 句柄，避免句柄表持有已卸载节点。
  function releaseDomHandles(host) {
    for (var id = 1; id < domHandles.length; id++) {
      var node = domHandles[id];
      if (node && host.contains && host.contains(node)) domHandles[id] = null;
    }
  }

  // 卸载 island：解绑监听与订阅、丢弃 memo、释放 DOM 句柄并清空子树。
  // `target` 缺省为整页；返回卸载的挂载点数，重挂载走 `mountIslands()`。
  function unmountIslands(target) {
    var scope = target || document;
    var hosts = scope.querySelectorAll ? scope.querySelectorAll('[data-component]') : [];
    var count = 0;
    hosts.forEach(function (host) {
      if (!host.__voaMounted) return;
      if (host.__voaDispose) host.__voaDispose();
      releaseDomHandles(host);
      host.__voaDispose = null;
      host.__voaMounted = false;
      host.textContent = '';
      count += 1;
    });
    return count;
  }

  function loadScript(url) {
    return new Promise(function (resolve, reject) {
      var script = document.createElement('script');
      script.src = url;
      script.async = false;
      script.onload = function () { resolve(); };
      script.onerror = function () { reject(new Error('script load failed: ' + url)); };
      document.head.appendChild(script);
    });
  }

  async function start(manifestUrl) {
    var manifest = await fetch(manifestUrl).then(function (r) {
      if (!r.ok) throw new Error('manifest fetch failed: ' + r.status);
      return r.json();
    });

    var entry = (manifest.wasm && manifest.wasm.length > 0) ? manifest.wasm[0] : null;
    if (entry) {
      var bytes = new Uint8Array(await fetch(entry.url).then(function (r) {
        if (!r.ok) throw new Error('wasm fetch failed: ' + r.status);
        return r.arrayBuffer();
      }));
      parseCustomSections(bytes);
      var result = await WebAssembly.instantiate(bytes, { env: env });
      wasmExports = result.instance ? result.instance.exports : result.exports;
    }

    if (manifest.components) {
      for (var i = 0; i < manifest.components.length; i++) {
        await loadScript(manifest.components[i].js);
      }
    }

    mountIslands();
    return manifest;
  }

  function hydrateFromDom(manifestUrl) {
    return start(manifestUrl || MANIFEST_URL);
  }

  global.__voa = {
    storeDomHandle: storeDomHandle,
    getDomHandle: getDomHandle,
    isLoaded: isLoaded,
    callExport: callWasm,
    registerComponent: registerComponent,
    mountIslands: mountIslands,
    unmountIslands: unmountIslands,
    hydrate: hydrateFromDom,
    start: start,
    wasmModule: WASM_STEM,
    host: env,
    hostIntern: hostIntern,
    hostResolve: hostResolve
  };

  // Auto glue: page must not inline start(); boot self-starts on load.
  start(MANIFEST_URL).catch(function (e) {
    console.error('asgard start failed:', e);
  });
})(typeof globalThis !== 'undefined' ? globalThis : window);
"##;
