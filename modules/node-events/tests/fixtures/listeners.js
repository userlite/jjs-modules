const E = require('events');
function assert(value, label) { if (!value) throw new Error(label); }
assert(E === require('node:events') && E.EventEmitter === E, 'exports');
const trace = [];
let e = new E();
assert(e.on === e.addListener && e.off === e.removeListener, 'method aliases');
function a(n) { trace.push('a' + n); e.off('x', b); e.on('x', d); }
function b(n) { trace.push('b' + n); }
function d(n) { trace.push('d' + n); }
e.on('x', a).on('x', b);
assert(e.emit('x', 1), 'emit found');
e.emit('x', 2);
assert(!e.emit('absent'), 'emit absent');
e.removeAllListeners();
e.once('r', function(n) { trace.push('once-a' + n); e.emit('r', 2); });
e.once('r', function(n) { trace.push('once-b' + n); });
e.emit('r', 1);
assert(e.listenerCount('r') === 0, 'once removed before nested emission');
e.on('dup', b).once('dup', b).on('dup', b);
e.removeListener('dup', b);
assert(e.listeners('dup').length === 2 && e.listeners('dup')[1] === b, 'remove last duplicate and unwrap once');
e.emit('dup', 3);
assert(e.listenerCount('dup') === 1, 'persistent duplicate survives');
const copy = e.listeners('dup'); copy[0] = a;
e.emit('dup', 4);
e.removeAllListeners();
function make() { return function twin() { trace.push('twin'); }; }
const first = make(), second = make();
assert(typeof first === "function" && typeof second === "function", "factory callables");
e.on('identity', first).on('identity', second).off('identity', second);
assert(e.listeners('identity')[0] === first && e.listenerCount('identity') === 1, 'distinct closure identity');
const bound = first.bind(null);
e.on('identity', bound).off('identity', first);
assert(e.listeners('identity')[0] === bound, 'bound callback identity');
e.on('native', Math.floor).on('native', Math.ceil).off('native', Math.ceil);
assert(e.listeners('native')[0] === Math.floor && e.listenerCount('native') === 1, 'native identity');
e.removeAllListeners();
const owner = {tag:'owner'};
function arrowFactory() { return () => { assert(this === owner, 'arrow this'); trace.push('arrow'); }; }
e.on('this', function() { assert(this === e, 'ordinary this'); trace.push('ordinary'); });
e.on('this', arrowFactory.call(owner));
e.emit('this');
e.removeAllListeners();
const error = new Error('sentinel');
e.once('throws', function() { throw error; });
e.on('throws', function() { trace.push('after-throw'); });
let caught = false;
try { e.emit('throws'); } catch (value) { caught = value === error; }
assert(caught && e.listenerCount('throws') === 1, 'exact throw and once removal');
e.emit('throws');
caught = false;
try { e.emit('error', error); } catch (value) { caught = value === error; }
assert(caught, 'unhandled error identity');
e.on('error', function(value) { assert(value === error, 'handled error argument'); });
e.emit('error', error);
caught = false;
try { e.on('bad', 42); } catch (value) { caught = value.name === 'TypeError'; }
assert(caught, 'bad listener');
e.removeAllListeners();
e.on('x', a).on('y', b).on('x', d).off('x', a);
assert(JSON.stringify(e.eventNames()) === '["x","y"]', 'event insertion order survives removal');
e.on('10', b).on('2', b).on('__proto__', b).on('constructor', b);
trace.push(JSON.stringify(e.eventNames()));
e.removeAllListeners('x');
assert(e.listenerCount('x') === 0 && e.listenerCount('y') === 1, 'remove event only');
e.removeAllListeners();
assert(e.eventNames().length === 0, 'remove all');
return JSON.stringify(trace);
