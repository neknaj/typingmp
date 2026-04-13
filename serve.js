// ローカル開発用 HTTP サーバー
// WASM は file:// では動作しないため、必ずこのサーバー経由でテストする
// 使い方: node serve.js [port]  (デフォルト: 8080)
//         ブラウザで http://localhost:8080 を開く

const http = require('http');
const fs = require('fs');
const path = require('path');

const PORT = parseInt(process.argv[2]) || 8080;
const ROOT = __dirname;

const MIME = {
    '.html': 'text/html; charset=utf-8',
    '.js':   'application/javascript',
    '.wasm': 'application/wasm',
    '.css':  'text/css',
    '.ntq':  'text/plain; charset=utf-8',
    '.ttf':  'font/ttf',
};

const server = http.createServer((req, res) => {
    let urlPath = req.url.split('?')[0];
    if (urlPath === '/') urlPath = '/index.html';

    const filePath = path.join(ROOT, urlPath);

    // ROOT の外へのパストラバーサルを防ぐ
    if (!filePath.startsWith(ROOT)) {
        res.writeHead(403);
        res.end('Forbidden');
        return;
    }

    fs.readFile(filePath, (err, data) => {
        if (err) {
            res.writeHead(404);
            res.end('Not Found: ' + urlPath);
            return;
        }
        const ext = path.extname(filePath).toLowerCase();
        const mime = MIME[ext] || 'application/octet-stream';
        res.writeHead(200, { 'Content-Type': mime });
        res.end(data);
    });
});

server.listen(PORT, () => {
    console.log(`[+] Dev server running at http://localhost:${PORT}`);
    console.log(`    Press Ctrl+C to stop`);
});
