const assert = require('assert/strict');
const os = require('os');
const path = require('path');
const test = require('node:test');

const {
    DEFAULT_PORT,
    decodeRequestPath,
    parsePort,
    resolveRequestPath,
} = require('./serve');

function requestPathFor(targetPath, root) {
    return `/${path.relative(root, targetPath).replaceAll(path.sep, '/')}`;
}

test('parsePort falls back to default for invalid input', () => {
    assert.equal(parsePort(undefined), DEFAULT_PORT);
    assert.equal(parsePort('invalid'), DEFAULT_PORT);
    assert.equal(parsePort('0'), DEFAULT_PORT);
    assert.equal(parsePort('70000'), DEFAULT_PORT);
    assert.equal(parsePort('3000'), 3000);
});

test('resolveRequestPath maps root to index under the server root', () => {
    const root = path.join(os.tmpdir(), 'typingmp-serve-root');
    assert.equal(resolveRequestPath('/', root), path.resolve(root, 'index.html'));
});

test('resolveRequestPath rejects parent traversal', () => {
    const root = path.join(os.tmpdir(), 'typingmp-serve-root');
    assert.equal(resolveRequestPath('/../secret.txt', root), null);
});

test('resolveRequestPath rejects encoded parent traversal', () => {
    const root = path.join(os.tmpdir(), 'typingmp-serve-root');
    const decodedPath = decodeRequestPath('/%2e%2e/secret.txt');
    assert.equal(decodedPath, '/../secret.txt');
    assert.equal(resolveRequestPath(decodedPath, root), null);
});

test('resolveRequestPath rejects sibling prefix paths', () => {
    const root = path.join(os.tmpdir(), 'typingmp-serve-root');
    const siblingPath = path.join(`${root}-sibling`, 'secret.txt');
    assert.equal(resolveRequestPath(requestPathFor(siblingPath, root), root), null);
});

test('decodeRequestPath rejects malformed percent escapes', () => {
    assert.equal(decodeRequestPath('/%E0%A4%A'), null);
});
