const mod = require('url');
if (mod !== require('node:url')) throw new Error('aliases');
const URL = mod.URL, Params = mod.URLSearchParams;
  let invalid=0; try {new URL('/no-base');} catch(e) {if(e.name==='TypeError') invalid++;}
  try {new URL('https://ok.example','bad base');} catch(e) {if(e.name==='TypeError') invalid++;}
function probe() {
  const trace = [];
  const inputs = ['https://User:pw@EXAMPLE.com:443/a/../b?q=a+b&q=a%2Bb#h', '../c?x=%26%3D#frag', '//other.example/a', '?x=%FF&x=%ZZ', '#', 'https://例え.テスト/雪?café=☃', 'http://[::1]:8080/?', 'mailto:hello@example.com'];
  for (let i=0;i<inputs.length;i++) {
    const u=new URL(inputs[i],'https://example.com/a/b');
    trace.push([u.href,u.origin,u.protocol,u.username,u.password,u.host,u.hostname,u.port,u.pathname,u.search,u.hash,u.toString(),u.toJSON(),u.searchParams.toString()]);
  }
  trace.push(JSON.stringify(new URL('https://example.com')));
  const p=new Params('?a=1&a=2&empty&plus=a+b&p=%2B&__proto__=ok&constructor=yes');
  trace.push([p.size,p.get('a'),p.getAll('a'),p.get('missing'),p.has('empty'),p.has('missing')]);
  p.append('a','3'); p.set('plus','x y+'); p.delete('empty'); p.sort();
  trace.push(p.toString()); p.set('new','雪'); p.set('a','one'); trace.push(p.toString());
  const order=new Params('z=1&😀=2&\uE000=3&z=4'); order.sort(); trace.push(order.toString());
  trace.push(new Params().toString());
  trace.push(new Params('x=%ZZ&y=%E0%A4%A&z=%FF&x=%').toString());
  const u=new URL('/a?x=1','https://EXAMPLE.com:443'); const saved=u.searchParams;
  saved.append('x','2'); trace.push(u.href);
  u.search='?space=a%20b&tilde=~'; trace.push(saved.toString()); saved.sort(); trace.push(u.href);
  u.pathname='/c d'; u.hash='#雪'; trace.push(u.href);
  u.href='https://other.example/x?b=2'; trace.push(saved===u.searchParams); saved.delete('b'); trace.push(u.href);
  u.search='?';u.hash='#';trace.push([u.href,u.search,u.hash]);u.search='';u.hash='';trace.push(u.href);
  trace.push(invalid);
  const legacy=['/p?a=1&a=2#f','//Host/p?q=x','a b?x=%ZZ','?','/x#','', '/a/../b?x=+&p=%2B', '/雪?café=☃', 'https://EXAMPLE.com:80/a/../b?x=1#f', 'https://example.com', '/x?__proto__=a&constructor=b', '/x?x=%FF&y=%E0%A4%A'];
  for (let i=0;i<legacy.length;i++){trace.push(mod.parse(legacy[i],false));const r=mod.parse(legacy[i],true);if(Object.getPrototypeOf(r.query)!==null)throw new Error('legacy query prototype');trace.push(r);}
  return JSON.stringify(trace);
}
return [0].map(probe)[0];
