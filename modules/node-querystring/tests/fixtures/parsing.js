const q = require('querystring');
if (q !== require('node:querystring') || q.parse !== q.decode || q.stringify !== q.encode) throw new Error('aliases');
function probe() {
  const trace = [];
  const cases = ['', 'a=1&a=2&b=&empty&=v&&', 's=a+b&p=a%2Bb&amp=%26&eq=%3D', 'café=雪&%E9%9B%AA=%F0%9F%8C%8D', 'x=%ZZ&y=%E0%A4%A&z=%FF&x=%', '__proto__=x&constructor=y&toString=z', '2=b&10=c&1=a', 'x=a=b=c', 'x=%C0%AF%ED%A0%80%F4%90%80%80'];
  for (let i=0; i<cases.length; i++) {
    const parsed=q.parse(cases[i]);
    if (Object.getPrototypeOf(parsed) !== null) throw new Error('null prototype');
    trace.push(parsed);
    trace.push(q.stringify(parsed));
  }
  trace.push(q.parse('a:1||a:2||b:3', '||', ':'));
  trace.push(q.stringify({a:['1','2'],b:'a b+c'}, '||', ':'));
  trace.push(q.parse('&&a=1&b=2', '&', '=', {maxKeys:2}));
  trace.push(q.parse('a=1&a=2&b=3', '&', '=', {maxKeys:2}));
  trace.push(q.parse('a=1&a=2&b=3', undefined, undefined, {maxKeys:0}));
  trace.push(q.parse(null));
  trace.push(q.stringify({s:"!'()~* ",n:42,b:true,empty:null,u:undefined,o:{x:1},a:[1,false,null,{}],skip:[],nan:NaN,inf:Infinity}));
  trace.push(q.stringify(q.parse('__proto__=a&__proto__=b&constructor=c')));
  trace.push(q.stringify(['a','b']));
  const sparse=[];sparse[2]='c';sparse[3]=undefined;trace.push(q.stringify(sparse));
  const hidden=Object.create({secret:'no'}); hidden.a='yes'; trace.push(q.stringify(hidden));
  return JSON.stringify(trace);
}
return [0].map(probe)[0];
