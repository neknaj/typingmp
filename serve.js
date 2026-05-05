const http = require('http');
const fs = require('fs');
const path = require('path');

const DEFAULT_PORT = 8080;
const ROOT = path.resolve(__dirname);

const MIME = {
    '.html': 'text/html; charset=utf-8',
    '.js': 'application/javascript',
    '.wasm': 'application/wasm',
    '.css': 'text/css',
    '.ntq': 'text/plain; charset=utf-8',
    '.ttf': 'font/ttf',
};

function parsePort(value) {
    if (value === undefined) {
        return DEFAULT_PORT;
    }
    const port = Number.parseInt(value, 10);
    return Number.isInteger(port) && port > 0 && port <= 65535 ? port : DEFAULT_PORT;
}

function decodeRequestPath(rawUrl) {
    if (typeof rawUrl !== 'string') {
        return null;
    }
    const pathname = rawUrl.split(/[?#]/, 1)[0];

    try {
        return decodeURIComponent(pathname);
    } catch {
        return null;
    }
}

function isPathInsideRoot(root, filePath) {
    const relative = path.relative(root, filePath);
    return relative === '' || (!relative.startsWith('..') && !path.isAbsolute(relative));
}

function resolveRequestPath(urlPath, root = ROOT) {
    if (urlPath === null || urlPath.includes('\0')) {
        return null;
    }

    const normalizedRoot = path.resolve(root);
    const requestPath = urlPath === '/' ? '/index.html' : urlPath;
    const filePath = path.resolve(normalizedRoot, `.${requestPath}`);
    return isPathInsideRoot(normalizedRoot, filePath) ? filePath : null;
}

function createServer(root = ROOT) {
    const normalizedRoot = path.resolve(root);
    return http.createServer((req, res) => {
        const urlPath = decodeRequestPath(req.url);
        const filePath = resolveRequestPath(urlPath, normalizedRoot);

        if (filePath === null) {
            res.writeHead(403);
            res.end('Forbidden');
            return;
        }

        fs.readFile(filePath, (err, data) => {
            if (err) {
                res.writeHead(404);
                res.end(`Not Found: ${urlPath}`);
                return;
            }
            const ext = path.extname(filePath).toLowerCase();
            const mime = MIME[ext] || 'application/octet-stream';
            res.writeHead(200, { 'Content-Type': mime });
            res.end(data);
        });
    });
}

if (require.main === module) {
    const port = parsePort(process.argv[2]);
    const server = createServer();
    server.listen(port, () => {
        console.log(`[+] Dev server running at http://localhost:${port}`);
        console.log('    Press Ctrl+C to stop');
    });
}

module.exports = {
    DEFAULT_PORT,
    ROOT,
    MIME,
    parsePort,
    decodeRequestPath,
    resolveRequestPath,
    createServer,
};
