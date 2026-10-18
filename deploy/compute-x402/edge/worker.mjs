// Fixed-origin x402 transport only. Payment verification and recovery stay in the gateway.
export const PUBLIC_HOST = 'compute-inference.opencovenant.org';
export const ORIGIN = 'https://covenant-compute-inference.onrender.com';
export const MAX_BODY_BYTES = 64 * 1024;
export const MAX_PAYMENT_HEADER_BYTES = 32 * 1024;
const paymentHeaders = ['payment-signature', 'x-payment'];
const forwardedHeaders = ['accept', 'content-type', 'content-encoding', 'idempotency-key', ...paymentHeaders];
const encoder = new TextEncoder();
const noCache = headers => {
  headers.set('cache-control', 'no-store');
  headers.set('cdn-cache-control', 'no-store');
  headers.set('cloudflare-cdn-cache-control', 'no-store');
  return headers;
};
const refuse = (status, code) => new Response(JSON.stringify({error:{code}}), {
  status, headers: noCache(new Headers({'content-type':'application/json'})),
});
class IngressError extends Error {
  constructor(status, code) { super(code); this.status=status; }
}
async function boundedBody(request) {
  const declared = request.headers.get('content-length');
  if (declared !== null) {
    if (!/^\d+$/.test(declared)) throw new IngressError(400, 'invalid_content_length');
    if (BigInt(declared) > BigInt(MAX_BODY_BYTES)) throw new IngressError(413, 'body_too_large');
  }
  if (!request.body) return new Uint8Array();
  const reader = request.body.getReader(); const chunks=[]; let length=0;
  try {
    for (;;) {
      const {done,value}=await reader.read(); if (done) break;
      length+=value.byteLength;
      if (length>MAX_BODY_BYTES) {
        void reader.cancel().catch(()=>{});
        throw new IngressError(413, 'body_too_large');
      }
      chunks.push(value);
    }
  } finally { reader.releaseLock(); }
  const bytes=new Uint8Array(length); let offset=0;
  for (const chunk of chunks) { bytes.set(chunk,offset); offset+=chunk.byteLength; }
  return bytes;
}
export function createProxy(fetchOrigin = (...args) => fetch(...args)) {
  return async request => {
    const url=new URL(request.url);
    if (url.protocol!=='https:' || url.hostname!==PUBLIC_HOST || url.port || url.search || url.hash || url.username || url.password) return refuse(404,'route_not_found');
    const health=url.pathname==='/health' && ['GET','HEAD'].includes(request.method);
    const inference=url.pathname==='/v1/chat/completions' && request.method==='POST';
    if (!health && !inference) return refuse(404,'route_not_found');
    const paymentBytes=paymentHeaders.reduce((total,name)=>total+encoder.encode(request.headers.get(name)??'').byteLength,0);
    if (paymentBytes>MAX_PAYMENT_HEADER_BYTES) return refuse(431,'payment_headers_too_large');
    if (health && (request.body || ![null,'0'].includes(request.headers.get('content-length')))) return refuse(400,'health_body_not_allowed');
    let body;
    if (inference) {
      try { body=await boundedBody(request); }
      catch (error) { return refuse(error instanceof IngressError?error.status:400,error instanceof IngressError?error.message:'invalid_request_body'); }
    }
    const headers=new Headers();
    for (const name of forwardedHeaders) if (request.headers.has(name)) headers.set(name,request.headers.get(name));
    // Request uncompressed bytes; do not copy cookies, authorization, Host or forwarding headers.
    headers.set('accept-encoding','identity');
    headers.set('cache-control','no-store');
    try {
      const upstream=await fetchOrigin(ORIGIN+url.pathname,{
        method:request.method,headers,body,redirect:'manual',cache:'no-store',signal:request.signal,
      });
      // Pass the stream through without decoding JSON, signatures or compressed response bytes.
      return new Response(request.method==='HEAD'?null:upstream.body,{
        status:upstream.status,statusText:upstream.statusText,headers:noCache(new Headers(upstream.headers)),encodeBody:'manual',
      });
    } catch {
      // The origin might already have admitted/settled a payment. Never retry or claim a refund.
      return refuse(502,'origin_outcome_unknown');
    }
  };
}
const proxy=createProxy();
export default { fetch(request) { return proxy(request); } };
