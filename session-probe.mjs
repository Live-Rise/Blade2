// 会话创建探针：走壳同款握手（GET token URL 取 dsh-auth cookie）后 POST api/session/create，
// 复现用户「新建会话失败 / mcp-client(playwright-mcp) initial connection failed」的最小操作。
import http from 'node:http';

const base = process.argv[2]; // http://127.0.0.1:2046
const tokenUrl = process.argv[3]; // http://127.0.0.1:2046/?token=...
const u = new URL(base);

function req(method, path, body, cookie) {
  return new Promise((resolve, reject) => {
    const r = http.request({
      hostname: u.hostname, port: u.port, path, method,
      headers: {
        'content-type': 'application/json',
        ...(cookie ? { cookie } : {}),
        ...(body ? { 'content-length': Buffer.byteLength(body) } : {}),
      },
    }, (res) => {
      let data = '';
      res.on('data', (c) => { data += c; });
      res.on('end', () => resolve({ status: res.statusCode, headers: res.headers, data }));
    });
    r.on('error', reject);
    if (body) r.write(body);
    r.end();
  });
}

const auth = await req('GET', new URL(tokenUrl).pathname + new URL(tokenUrl).search, null, null);
const setCookie = auth.headers['set-cookie'] ?? [];
console.log('auth GET status =', auth.status, 'cookies =', setCookie.map((c) => c.split(';')[0]).join('; '));
const cookie = setCookie.map((c) => c.split(';')[0]).join('; ');

const envelope = JSON.stringify({
  type: 'client-request',
  rpcId: 'probe-1',
  method: 'session/create',
  payload: { args: { request: { cwd: process.env.USERPROFILE + '\\Documents' } } },
});
const t0 = Date.now();
const res = await req('POST', '/api/session/create', envelope, cookie);
console.log('session/create status =', res.status, `(${((Date.now() - t0) / 1000).toFixed(1)}s)`);
console.log(res.data.slice(0, 2000));
