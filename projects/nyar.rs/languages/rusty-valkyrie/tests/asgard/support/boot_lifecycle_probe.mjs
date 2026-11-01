// Node 侧真实执行探针：跑生成的真实 boot.js，用最小 DOM 仿真驱动
// mount → input 事件 → 信号 → DOM 更新 → unmount 解绑 → 重挂载。
//
// 只仿真宿主 DOM 与 wasm 导出（导出是固定桩，不代表 wasm 语义）；
// boot.js 的加载、订阅、事件绑定与 unmount 清理全部走真实代码路径。

import fs from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';

class SimNode {
  constructor(nodeType, tagName) {
    this.nodeType = nodeType;
    this.tagName = tagName || '';
    this.childNodes = [];
    this.parentNode = null;
    this.attributes = new Map();
    this.listeners = new Map();
    this._text = '';
    this.value = '';
    this.classList = {
      add: (token) => {
        const tokens = this._classTokens();
        tokens.add(String(token));
        this.attributes.set('class', [...tokens].join(' '));
      },
      remove: (token) => {
        const tokens = this._classTokens();
        tokens.delete(String(token));
        this.attributes.set('class', [...tokens].join(' '));
      },
      toggle: (token) => {
        const tokens = this._classTokens();
        const name = String(token);
        if (tokens.has(name)) tokens.delete(name);
        else tokens.add(name);
        this.attributes.set('class', [...tokens].join(' '));
      },
    };
  }

  _classTokens() {
    return new Set((this.attributes.get('class') || '').split(/\s+/).filter(Boolean));
  }

  get firstChild() {
    return this.childNodes[0] || null;
  }

  get textContent() {
    if (this.nodeType === 3) return this._text;
    if (this.childNodes.length === 0) return this._text;
    return this.childNodes.map((child) => child.textContent).join('');
  }

  set textContent(value) {
    const text = value == null ? '' : String(value);
    if (this.nodeType === 3) {
      this._text = text;
      return;
    }
    this.childNodes.forEach((child) => {
      child.parentNode = null;
    });
    this.childNodes = [];
    this._text = text;
    if (text !== '') {
      const node = new SimNode(3);
      node._text = text;
      this.appendChild(node);
    }
  }

  appendChild(child) {
    if (!child || child.nodeType == null) throw new TypeError('appendChild expects a node');
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    this.childNodes.push(child);
    return child;
  }

  removeChild(child) {
    const index = this.childNodes.indexOf(child);
    if (index < 0) throw new Error('removeChild: node is not a child');
    this.childNodes.splice(index, 1);
    child.parentNode = null;
    return child;
  }

  insertBefore(child, before) {
    if (!before) return this.appendChild(child);
    const index = this.childNodes.indexOf(before);
    if (index < 0) throw new Error('insertBefore: reference node is not a child');
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    this.childNodes.splice(index, 0, child);
    return child;
  }

  replaceChild(child, old) {
    const index = this.childNodes.indexOf(old);
    if (index < 0) throw new Error('replaceChild: old node is not a child');
    if (child.parentNode) child.parentNode.removeChild(child);
    child.parentNode = this;
    this.childNodes[index] = child;
    old.parentNode = null;
    return old;
  }

  setAttribute(name, value) {
    this.attributes.set(String(name), String(value));
  }

  getAttribute(name) {
    return this.attributes.has(name) ? this.attributes.get(name) : null;
  }

  addEventListener(type, handler) {
    const key = String(type);
    if (!this.listeners.has(key)) this.listeners.set(key, new Set());
    this.listeners.get(key).add(handler);
  }

  removeEventListener(type, handler) {
    const set = this.listeners.get(String(type));
    if (set) set.delete(handler);
  }

  listenerCount(type) {
    const set = this.listeners.get(String(type));
    return set ? set.size : 0;
  }

  dispatchEvent(event) {
    const set = this.listeners.get(String(event.type));
    if (set) {
      for (const handler of [...set]) handler(event);
    }
  }

  contains(node) {
    for (let current = node; current; current = current.parentNode) {
      if (current === this) return true;
    }
    return false;
  }
}

class SimDocument {
  constructor() {
    this.nodeType = 9;
    this._html = new SimNode(1, 'html');
    this.head = new SimNode(1, 'head');
    this.body = new SimNode(1, 'body');
    this._html.appendChild(this.head);
    this._html.appendChild(this.body);
  }

  createElement(tagName) {
    return new SimNode(1, String(tagName));
  }

  createTextNode(text) {
    const node = new SimNode(3);
    node._text = String(text == null ? '' : text);
    return node;
  }

  querySelectorAll(selector) {
    const matches = [];
    const attributeOnly = selector === '[data-component]';
    const exact = /^\[data-component="(.*)"\]$/.exec(selector);
    const visit = (node) => {
      if (node.nodeType === 1) {
        const component = node.getAttribute('data-component');
        if ((attributeOnly && component !== null) || (exact && component === exact[1])) {
          matches.push(node);
        }
      }
      node.childNodes.forEach(visit);
    };
    visit(this._html);
    return matches;
  }

  contains(node) {
    for (let current = node; current; current = current.parentNode) {
      if (current === this._html) return true;
    }
    return false;
  }
}

globalThis.document = new SimDocument();
globalThis.CustomEvent = class CustomEvent {
  constructor(type, init) {
    this.type = type;
    this.detail = init && init.detail;
  }
};

// 页面级事件注册表：捕获 boot 登记的 `pagehide` 卸载监听，测试末尾真实派发。
const pageListeners = new Map();
globalThis.addEventListener = (type, handler) => {
  const key = String(type);
  if (!pageListeners.has(key)) pageListeners.set(key, new Set());
  pageListeners.get(key).add(handler);
};
globalThis.removeEventListener = (type, handler) => {
  const set = pageListeners.get(String(type));
  if (set) set.delete(handler);
};
globalThis.dispatchEvent = (event) => {
  const set = pageListeners.get(String(event.type));
  if (set) {
    for (const handler of [...set]) handler(event);
  }
};

globalThis.fetch = async (url) => {
  if (url === 'manifest.json' || url === '/manifest.json') {
    return { ok: true, json: async () => ({ wasm: [{ url: 'probe.wasm', glue: 'probe.mjs' }], components: [] }) };
  }
  if (url === 'probe.wasm') {
    // 8 字节 wasm 头足够让 boot 的自定义段扫描安全返回；导出由下面的桩提供。
    return { ok: true, arrayBuffer: async () => new Uint8Array(8) };
  }
  return { ok: false, status: 404 };
};

let exprCalls = 0;
let observeCalls = 0;
let probeSignalId = -1;
globalThis.WebAssembly = {
  instantiate: async () => ({
    instance: {
      exports: {
        probeExpr: () => {
          exprCalls += 1;
          return globalThis.__voa.host['__voa.sigGetUtf8'](probeSignalId);
        },
        probeObserve: () => {
          observeCalls += 1;
        },
      },
    },
  }),
};

const bootSource = fs.readFileSync(process.argv[2], 'utf8');
vm.runInThisContext(bootSource, { filename: 'boot.js' });

await globalThis.__voa.start('manifest.json');
assert.equal(globalThis.__voa.isLoaded(), true, 'boot 必须完成 wasm 装载');

const host = document.createElement('div');
host.setAttribute('data-component', 'probe');
document.body.appendChild(host);

let inputRef = null;
let firstMount = true;
globalThis.__voa.registerComponent('probe', (mountPoint) => {
  // 模块级状态按名寻址：重挂载必须复用同一信号，而不是每次挂载新建状态。
  const signalId = globalThis.__voa.host['__voa.sigNamed']('probe-value');
  if (firstMount) {
    globalThis.__voa.host['__voa.sigSetUtf8'](signalId, '');
    firstMount = false;
  }
  probeSignalId = signalId;
  const input = document.createElement('input');
  inputRef = input;
  const handle = globalThis.__voa.storeDomHandle(input);
  globalThis.__voa.host['__voa.rxBindValueUtf8'](handle, 'probeExpr', signalId, -1);
  globalThis.__voa.host['__voa.domBindInputUtf8'](signalId, handle, '');
  // `sigSubscribe` 与 `rxBindValueUtf8` 走同一激活期作用域；卸载后两条订阅都必须解绑。
  globalThis.__voa.host['__voa.sigSubscribe'](signalId, String(signalId), 'probeObserve');
  mountPoint.appendChild(input);
  return mountPoint;
});

globalThis.__voa.mountIslands();
assert.equal(host.__voaMounted, true, 'mount 必须标记挂载点');
assert.equal(inputRef.listenerCount('input'), 1, 'mount 必须绑定 input 监听');
assert.equal(inputRef.value, '', '初始信号应把输入框同步为空串');
assert.equal(exprCalls, 1, '挂载时必须执行一次表达式');
assert.equal(observeCalls, 1, '`sigSubscribe` 必须在挂载时激活一次');

inputRef.value = 'hello';
inputRef.dispatchEvent({ type: 'input' });
assert.equal(inputRef.value, 'hello', 'input 事件必须写回信号并驱动 DOM 更新');
assert.equal(globalThis.__voa.hostResolve(globalThis.__voa.host['__voa.sigGetUtf8'](probeSignalId)), 'hello', '信号必须保存输入值');
assert.equal(exprCalls, 2, '信号变化必须触发一次订阅更新');
assert.equal(observeCalls, 2, '`sigSubscribe` 必须随信号变化触发');

const removed = globalThis.__voa.unmountIslands();
assert.equal(removed, 1, 'unmount 必须报告卸载的挂载点数');
assert.equal(host.__voaMounted, false, 'unmount 必须清掉挂载标记');
assert.equal(host.textContent, '', 'unmount 必须清空岛子树');
assert.equal(inputRef.listenerCount('input'), 0, 'unmount 必须解绑 input 监听');

const callsAfterUnmount = exprCalls;
const observesAfterUnmount = observeCalls;
inputRef.value = 'after-unmount';
inputRef.dispatchEvent({ type: 'input' });
assert.equal(globalThis.__voa.hostResolve(globalThis.__voa.host['__voa.sigGetUtf8'](probeSignalId)), 'hello', '卸载后的监听不得再写信号');
globalThis.__voa.host['__voa.sigSetUtf8'](probeSignalId, 'mutated');
assert.equal(globalThis.__voa.hostResolve(globalThis.__voa.host['__voa.sigGetUtf8'](probeSignalId)), 'mutated');
assert.equal(exprCalls, callsAfterUnmount, '卸载后的订阅不得再执行表达式');
assert.equal(observeCalls, observesAfterUnmount, '卸载后的 `sigSubscribe` 订阅不得再触发');
globalThis.__voa.host['__voa.sigSetUtf8'](probeSignalId, 'hello');

globalThis.__voa.mountIslands();
assert.equal(host.__voaMounted, true, '重挂载必须恢复挂载标记');
assert.equal(inputRef.listenerCount('input'), 1, '重挂载必须重新绑定监听');
assert.equal(inputRef.value, 'hello', '重挂载必须从当前信号恢复 DOM');
assert.equal(exprCalls, callsAfterUnmount + 1, '重挂载必须重新计算一次表达式');
assert.equal(observeCalls, observesAfterUnmount + 1, '重挂载必须重新激活 `sigSubscribe`');

// ── 页面卸载 dispose 链路 ──────────────────────────────────────────
assert.equal(pageListeners.has('pagehide'), true, 'boot 必须登记 pagehide 卸载监听');
const exprBeforeUnload = exprCalls;
const observeBeforeUnload = observeCalls;
globalThis.dispatchEvent({ type: 'pagehide' });
assert.equal(host.__voaMounted, false, 'pagehide 必须卸载全部 island');
assert.equal(inputRef.listenerCount('input'), 0, 'pagehide 必须解绑 input 监听');
assert.equal(globalThis.__voa.isLoaded(), false, 'pagehide 必须放下 wasm 实例引用');
globalThis.__voa.host['__voa.sigSetUtf8'](probeSignalId, 'after-pagehide');
assert.equal(exprCalls, exprBeforeUnload, 'pagehide 后的信号写入不得再执行表达式');
assert.equal(observeCalls, observeBeforeUnload, 'pagehide 后的信号写入不得再触发 `sigSubscribe`');
// 幂等：重复卸载事件不得重复清理或抛错。
globalThis.dispatchEvent({ type: 'pagehide' });
globalThis.__voa.disposePage();

process.stdout.write('BOOT_LIFECYCLE_OK\n');
