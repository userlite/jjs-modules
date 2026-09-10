// Pinned semantic reference for the byte APIs used by Iteration 6.
const assert=require('node:assert/strict');
const fs=require('node:fs');const os=require('node:os');const path=require('node:path');const http=require('node:http');
const express=require('express');
assert.equal(process.version,'v22.22.0');assert.equal(require('express/package.json').version,'5.1.0');
(async()=>{
 const dir=fs.mkdtempSync(path.join(os.tmpdir(),'iteration6-'));let server;
 try{
  const payload=Buffer.from(Array.from({length:20000},(_,i)=>i%256));const file=path.join(dir,'object');
  fs.writeFileSync(file,payload);assert.deepEqual(fs.readFileSync(file),payload);
  await new Promise((resolve,reject)=>fs.writeFile(file,payload,e=>e?reject(e):resolve()));
  assert.deepEqual(await new Promise((resolve,reject)=>fs.readFile(file,(e,b)=>e?reject(e):resolve(b))),payload);
  await fs.promises.writeFile(file,payload);assert.deepEqual(await fs.promises.readFile(file),payload);
  let closed=0;const app=express();app.get('/:mode',(req,res)=>{res.on('close',()=>closed++);if(req.params.mode==='send')res.send(payload);else if(req.params.mode==='end')res.end(payload);else{res.write(payload.subarray(0,8192));res.end(payload.subarray(8192));}});
  server=await new Promise(resolve=>{const s=app.listen(0,'127.0.0.1',()=>resolve(s));});
  for(const mode of ['send','end','write']){
   const bytes=await new Promise((resolve,reject)=>http.get({hostname:'127.0.0.1',port:server.address().port,path:'/'+mode},res=>{let chunks=[];res.on('data',b=>chunks.push(b));res.on('end',()=>resolve(Buffer.concat(chunks)));}).on('error',reject));assert.deepEqual(bytes,payload);
  }
  assert.equal(closed,3);console.log(JSON.stringify({node:process.version,express:'5.1.0',fsModes:3,responseModes:3,closeCallbacks:closed,status:'passed'}));
 }finally{if(server)await new Promise(resolve=>server.close(resolve));fs.rmSync(dir,{recursive:true});}
})().catch(e=>{console.error(e);process.exitCode=1;});
