// Run with NODE_PATH pointing to the pinned Express 5.1.0 installation.
const assert=require('node:assert/strict');
const {Readable}=require('node:stream');
const express=require('express');
assert.equal(process.version,'v22.22.0');
assert.equal(require('express/package.json').version,'5.1.0');
async function parse(body,options={extended:false},contentType='application/x-www-form-urlencoded') {
 const req=Readable.from([Buffer.from(body)]);req.headers={'content-type':contentType,'content-length':String(Buffer.byteLength(body))};
 return new Promise((resolve,reject)=>express.urlencoded(options)(req,{},err=>err?reject(err):resolve(req.body)));
}
(async()=>{
 const cases=[['a=1&a=2&title=hello+world&u=%E2%82%AC&empty=&flag&nested[x]=3&__proto__=no&constructor=yes',{a:['1','2'],title:'hello world',u:'€',empty:'',flag:'','nested[x]':'3',constructor:'yes'}],['a=%ZZ&b=%E0%A4&c=%FF',{a:'%ZZ',b:'%E0%A4',c:'%FF'}],['',{}]];
 for(const [body,expected] of cases)assert.deepEqual(JSON.parse(JSON.stringify(await parse(body))),expected);
 assert.deepEqual(JSON.parse(JSON.stringify(await parse('a=1',{}))),{a:'1'});
 await assert.rejects(parse('a=12',{extended:false,limit:3}),e=>e.status===413);
 console.log(JSON.stringify({node:process.version,express:require('express/package.json').version,cases:cases.length+2,status:'passed'}));
})().catch(e=>{console.error(e);process.exitCode=1;});
