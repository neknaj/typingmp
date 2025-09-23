const { WebSocketServer } = require('ws');
const fs = require('fs');
const path = require('path');

const PORT = 8081;
const LOGS_DIR = path.join(__dirname, 'logs');

// 'logs' ディレクトリがなければ作成
if (!fs.existsSync(LOGS_DIR)) {
    fs.mkdirSync(LOGS_DIR, { recursive: true });
}

const wss = new WebSocketServer({ port: PORT });

console.log(`[+] WebSocket logger server started on port ${PORT}`);
console.log(`[+] Logs will be saved in: ${LOGS_DIR}`);

wss.on('connection', ws => {
    console.log('[+] Client connected.');

    ws.on('message', message => {
        try {
            const data = JSON.parse(message);
            const { id, message: logMessage } = data;

            if (!id || !logMessage) {
                console.error('[!] Invalid log format received.');
                return;
            }
            
            // セキュリティのため、IDから不正な文字を削除
            const sanitizedId = id.replace(/[^a-zA-Z0-9-]/g, '');
            if (sanitizedId.length === 0) return;

            const logFilePath = path.join(LOGS_DIR, `${sanitizedId}.log`);
            const timestamp = new Date().toISOString();
            const logLine = `${timestamp} - ${logMessage}\n`;

            // ログファイルに追記
            fs.appendFile(logFilePath, logLine, (err) => {
                if (err) {
                    console.error(`[!] Failed to write to log file: ${err}`);
                }
            });

        } catch (error) {
            console.error('[!] Error processing message:', error);
        }
    });

    ws.on('close', () => {
        console.log('[-] Client disconnected.');
    });
});

wss.on('error', (error) => {
    console.error('[!] Server error:', error);
});