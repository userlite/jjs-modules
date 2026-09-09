const mod=require('buffer');if(mod!==require('node:buffer'))throw new Error('alias');
const B=mod.Buffer;
function probe(){
 const trace=[];
 const b=B.from([0,255,128,65,256,-1,1.9]);
 trace.push([B.isBuffer(b),B.isBuffer([]),b.length,b.toString('hex')]);
 let key=2;b[key]=257;b[1]=-2;b[99]=1;b[-1]=1;b[1.5]=2;
 trace.push([b[0],b[1],b[key],b[99]===undefined,b[-1]===undefined,b[1.5]===undefined]);
 trace.push(B.alloc(6).toString('hex'),B.alloc(5,'ab').toString(),B.alloc(3,257).toString('hex'));
 trace.push(B.from('雪😀').toString('hex'),B.byteLength('雪😀'));
 trace.push(B.from('00ff80','hex').toString('base64'),B.from('AP+A','base64').toString('hex'));
 trace.push(B.from([255,128,65]).toString('latin1'),B.from([255,128,65]).toString('ascii'));
 trace.push(B.from([0xe2,0x82,0xac,0xff,0xe2,0x82]).toString());
 const copy=B.from(b);b[0]=3;trace.push(copy[0],B.concat([copy,B.from('!')]).toString('hex'));
 trace.push(B.concat([B.from('a')],3).toString('hex'),B.concat([B.from('abc')],1).toString());
 trace.push(JSON.stringify(B.from([0,255])));
 return JSON.stringify(trace);
}
return [0].map(probe)[0];
