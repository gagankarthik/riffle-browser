// Riffle's own versions of the page scripts that filter lists call by name, such as `##+js(set, ads, false)`.
// Lines starting `/// ` begin a script and give its names. The engine fills in {{1}}, {{2}} and so on.
// PRELUDE is placed before each script.

/// PRELUDE
const arg = s => /^\{\{\d+\}\}$/.test(s) ? '' : s.replace(/^(['"`])(.*)\1$/s, '$2');
const pattern = raw => {
  let s = arg(raw);
  const not = s.startsWith('!');
  if (not) s = s.slice(1);
  const m = s.match(/^\/(.+)\/([a-z]*)$/s);
  let re;
  try { re = m ? new RegExp(m[1], m[2]) : new RegExp(s === '*' ? '' : s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')); } catch { re = /^\b$/; }
  return text => re.test(String(text)) !== not;
};
const trap = (owner, parts, make) => {
  const [head, ...rest] = parts;
  if (!owner || !head) return;
  const own = Object.getOwnPropertyDescriptor(owner, head);
  if (own && !own.configurable) return;
  if (!rest.length) {
    try { Object.defineProperty(owner, head, {configurable: true, ...make(own ? (own.get ? own.get.call(owner) : own.value) : owner[head])}); } catch {}
    return;
  }
  let value = owner[head];
  if (value instanceof Object) trap(value, rest, make);
  try {
    Object.defineProperty(owner, head, {configurable: true, get: () => value, set(next) { value = next; if (next instanceof Object) trap(next, rest, make); }});
  } catch {}
};
const chain = raw => arg(raw).split('.').filter(Boolean);
const stop = () => { throw new ReferenceError('Blocked by Riffle'); };
const onDom = run => {
  run();
  const watch = new MutationObserver(run);
  watch.observe(document, {childList: true, subtree: true});
  addEventListener('load', () => setTimeout(() => watch.disconnect(), 3000), {once: true});
};
const props = raw => {
  const out = {};
  for (const part of arg(raw).split(/\s+(?=\S+:)|\s+(?=\/)/).filter(Boolean)) {
    const at = /^[a-zA-Z]+:/.test(part) ? part.indexOf(':') : -1;
    out[at > 0 ? part.slice(0, at) : 'url'] = pattern(at > 0 ? part.slice(at + 1) : part);
  }
  return out;
};
const matches = (wanted, seen) => Object.entries(wanted).every(([key, test]) => test(seen[key] ?? ''));
const asked = input => input instanceof Request ? {url: input.url, method: input.method} : {url: String(input)};

/// set-constant.js set.js
const known = {undefined: undefined, false: false, true: true, null: null, '': '', "''": '', noopFunc() {}, trueFunc: () => true, falseFunc: () => false, noopThis() { return this; }, yes: 'yes', no: 'no', '[]': [], '{}': {}};
const raw = arg('{{2}}');
const value = raw in known ? known[raw] : /^-?\d+(\.\d+)?$/.test(raw) ? Number(raw) : stop;
if (value !== stop) trap(window, chain('{{1}}'), () => ({get: () => value, set() {}}));

/// abort-on-property-read.js aopr.js
trap(window, chain('{{1}}'), () => ({get: stop, set() {}}));

/// abort-on-property-write.js aopw.js
trap(window, chain('{{1}}'), () => ({get: () => undefined, set: stop}));

/// abort-on-stack-trace.js aost.js
const needle = pattern('{{2}}');
const parts = chain('{{1}}');
trap(window, parts, current => {
  let value = current;
  const check = () => { if (needle(new Error().stack)) stop(); };
  return {get() { check(); return value; }, set(next) { check(); value = next; }};
});

/// abort-current-script.js acs.js abort-current-inline-script.js acis.js
const needle = pattern('{{2}}');
const hit = () => {
  const script = document.currentScript;
  return script && needle(script.src || script.textContent);
};
trap(window, chain('{{1}}'), current => {
  let value = current;
  return {get() { if (hit()) stop(); return value; }, set(next) { if (hit()) stop(); value = next; }};
});

/// no-window-open-if.js nowoif.js window.open-defuser.js
const needle = pattern('{{1}}');
const fake = new Proxy(function () {}, {get: (_, key) => key === 'closed' ? false : key === 'document' ? document.implementation.createHTMLDocument('') : function () {}, set: () => true});
window.open = new Proxy(window.open, {apply: (open, self, args) => needle(args[0] ?? '') ? fake : Reflect.apply(open, self, args)});

/// no-setTimeout-if.js nostif.js setTimeout-defuser.js
const needle = pattern('{{1}}');
const delay = arg('{{2}}');
window.setTimeout = new Proxy(window.setTimeout, {apply(timer, self, args) {
  if (needle(args[0]) && (!delay || Number(delay) === Number(args[1] ?? 0))) args[0] = function () {};
  return Reflect.apply(timer, self, args);
}});

/// no-setInterval-if.js nosiif.js setInterval-defuser.js
const needle = pattern('{{1}}');
const delay = arg('{{2}}');
window.setInterval = new Proxy(window.setInterval, {apply(timer, self, args) {
  if (needle(args[0]) && (!delay || Number(delay) === Number(args[1] ?? 0))) args[0] = function () {};
  return Reflect.apply(timer, self, args);
}});

/// adjust-setInterval.js nano-sib.js nano-setInterval-booster.js
const needle = pattern('{{1}}');
const delay = Number(arg('{{2}}') || 1000);
const boost = Math.min(50, Math.max(.001, Number(arg('{{3}}') || .05)));
window.setInterval = new Proxy(window.setInterval, {apply(timer, self, args) {
  if (needle(args[0]) && Number(args[1]) === delay) args[1] = delay * boost;
  return Reflect.apply(timer, self, args);
}});

/// adjust-setTimeout.js nano-stb.js nano-setTimeout-booster.js
const needle = pattern('{{1}}');
const delay = Number(arg('{{2}}') || 1000);
const boost = Math.min(50, Math.max(.001, Number(arg('{{3}}') || .05)));
window.setTimeout = new Proxy(window.setTimeout, {apply(timer, self, args) {
  if (needle(args[0]) && Number(args[1]) === delay) args[1] = delay * boost;
  return Reflect.apply(timer, self, args);
}});

/// noeval-if.js prevent-eval-if.js
const needle = pattern('{{1}}');
window.eval = new Proxy(window.eval, {apply: (run, self, args) => needle(args[0]) ? undefined : Reflect.apply(run, self, args)});

/// addEventListener-defuser.js aeld.js
const type = pattern('{{1}}');
const needle = pattern('{{2}}');
EventTarget.prototype.addEventListener = new Proxy(EventTarget.prototype.addEventListener, {apply(add, self, args) {
  if (type(args[0]) && needle(args[1])) return;
  return Reflect.apply(add, self, args);
}});

/// remove-node-text.js rmnt.js
const name = pattern('{{1}}');
const needle = pattern('{{2}}');
const seen = new WeakSet();
const clean = () => {
  for (const node of document.querySelectorAll('script,style,noscript')) {
    if (seen.has(node) || !name(node.nodeName.toLowerCase())) continue;
    seen.add(node);
    if (needle(node.textContent)) node.textContent = '';
  }
};
onDom(clean);

/// replace-node-text.js rpnt.js trusted-replace-node-text.js trusted-rpnt.js
const name = pattern('{{1}}');
const raw = arg('{{2}}');
const m = raw.match(/^\/(.+)\/([a-z]*)$/s);
const find = m ? new RegExp(m[1], m[2]) : raw;
const change = arg('{{3}}');
const seen = new WeakSet();
onDom(() => {
  for (const node of document.querySelectorAll('script,style,noscript')) {
    if (seen.has(node) || !name(node.nodeName.toLowerCase())) continue;
    seen.add(node);
    const text = node.textContent;
    const next = text.replace(find, change);
    if (next !== text) node.textContent = next;
  }
});

/// no-fetch-if.js prevent-fetch.js
const wanted = props('{{1}}');
if (Object.keys(wanted).length) window.fetch = new Proxy(window.fetch, {apply(fetch, self, args) {
  const seen = {...asked(args[0]), ...(args[1] || {})};
  seen.method ??= 'GET';
  return matches(wanted, seen) ? Promise.resolve(new Response('', {status: 200})) : Reflect.apply(fetch, self, args);
}});

/// no-xhr-if.js prevent-xhr.js
const wanted = props('{{1}}');
const stopped = new WeakSet();
if (Object.keys(wanted).length) {
  const xhr = XMLHttpRequest.prototype;
  xhr.open = new Proxy(xhr.open, {apply(open, self, args) {
    if (matches(wanted, {method: args[0], url: args[1]})) stopped.add(self);
    return Reflect.apply(open, self, args);
  }});
  xhr.send = new Proxy(xhr.send, {apply(send, self, args) {
    if (!stopped.has(self)) return Reflect.apply(send, self, args);
    setTimeout(() => {
      for (const [key, value] of Object.entries({readyState: 4, status: 200, statusText: 'OK', response: '', responseText: ''})) Object.defineProperty(self, key, {configurable: true, value});
      for (const type of ['readystatechange', 'load', 'loadend']) self.dispatchEvent(new Event(type));
    });
  }});
}

/// json-prune.js
const paths = arg('{{1}}').split(/\s+/).filter(Boolean).map(p => p.split('.'));
const needs = arg('{{2}}').split(/\s+/).filter(Boolean).map(p => p.split('.'));
const has = (o, parts) => { for (const p of parts) { if (o == null || typeof o !== 'object' || !(p in o)) return false; o = o[p]; } return true; };
const cut = (o, [head, ...rest]) => {
  if (o == null || typeof o !== 'object') return;
  const keys = head === '[]' || head === '[-]' || head === '*' ? Object.keys(o) : head in o ? [head] : [];
  for (const key of keys) rest.length ? cut(o[key], rest) : delete o[key];
};
const prune = data => {
  if (needs.every(n => has(data, n))) for (const p of paths) cut(data, p);
  return data;
};
if (paths.length) {
  JSON.parse = new Proxy(JSON.parse, {apply: (parse, self, args) => prune(Reflect.apply(parse, self, args))});
  Response.prototype.json = new Proxy(Response.prototype.json, {apply: (json, self, args) => Reflect.apply(json, self, args).then(prune)});
}

/// remove-attr.js ra.js
const names = arg('{{1}}').split(/\s*\|\s*/).filter(Boolean);
const selector = arg('{{2}}') || names.map(n => `[${CSS.escape(n)}]`).join(',');
if (names.length) onDom(() => {
  for (const el of document.querySelectorAll(selector)) for (const n of names) el.removeAttribute(n);
});

/// trusted-replace-fetch-response.js trusted-rpfr.js
const raw = arg('{{1}}');
const m = raw.match(/^\/(.+)\/([a-z]*)$/s);
const find = m ? new RegExp(m[1], m[2]) : raw;
const change = arg('{{2}}');
const wanted = props('{{3}}');
window.fetch = new Proxy(window.fetch, {apply(fetch, self, args) {
  const answer = Reflect.apply(fetch, self, args);
  if (!find || !matches(wanted, asked(args[0]))) return answer;
  return answer.then(async response => {
    const text = await response.clone().text();
    const next = text.replace(find, change);
    if (next === text) return response;
    const copy = new Response(next, {status: response.status, statusText: response.statusText, headers: response.headers});
    for (const key of ['url', 'type', 'redirected']) Object.defineProperty(copy, key, {value: response[key]});
    return copy;
  });
}});

/// trusted-replace-xhr-response.js
const raw = arg('{{1}}');
const m = raw.match(/^\/(.+)\/([a-z]*)$/s);
const find = m ? new RegExp(m[1], m[2]) : raw;
const change = arg('{{2}}');
const wanted = props('{{3}}');
const changing = new WeakSet();
const xhr = XMLHttpRequest.prototype;
xhr.open = new Proxy(xhr.open, {apply(open, self, args) {
  if (find && matches(wanted, {method: args[0], url: args[1]})) changing.add(self);
  return Reflect.apply(open, self, args);
}});
for (const key of ['response', 'responseText']) {
  const own = Object.getOwnPropertyDescriptor(xhr, key);
  Object.defineProperty(xhr, key, {configurable: true, enumerable: true, get() {
    const value = own.get.call(this);
    return changing.has(this) && typeof value === 'string' ? value.replace(find, change) : value;
  }});
}
