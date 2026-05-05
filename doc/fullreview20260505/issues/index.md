# typingmp review issue index

generated_at: `2026-05-05T00:00:00Z`

## counts

| key | count |
|---|---:|
| total | 29 |
| open | 19 |
| resolved | 10 |
| P0 | 1 |
| P1 | 17 |
| P2 | 10 |
| P3 | 1 |

## by area

| area | count |
|---|---:|
| architecture | 5 |
| build | 2 |
| ci | 3 |
| core | 3 |
| docs | 3 |
| error-handling | 1 |
| mobile | 1 |
| performance | 3 |
| quality | 1 |
| security | 2 |
| static-safety | 1 |
| tui | 1 |
| uefi | 1 |
| wasm | 1 |
| web | 1 |

## issues

| priority | legacy id | area | type | title | file |
|---|---|---|---|---|---|
| P0 | TP-CI-001 | ci | bug | WASM debug buildがWEBSOCKET_ADDRESS未設定でcompile不能になる | [item](./items/ISS-20260505T000000Z-TP-CI-001-WASM-DEBUG-ENV.md) |
| P1 | TP-ARCH-004 | architecture | architecture | no_std typing coreとtarget UI adapterの境界が固定されていない | [item](./items/ISS-20260505T000000Z-TP-ARCH-004-NOSTD-CORE-UI-BOUNDARY.md) |
| P1 | TP-ARCH-005 | architecture | architecture | problem/font/storage/loggerのI/Oがprovider境界で仮想化されていない | [item](./items/ISS-20260505T000000Z-TP-ARCH-005-IO-PROVIDER-VFS.md) |
| P1 | TP-STATIC-001 | static-safety | type-safety | 状態とcommandがraw number/raw stringに依存しenumとmatchの網羅性検査が効いていない | [item](./items/ISS-20260505T000000Z-TP-STATIC-001-ENUM-MATCH-STATE.md) |
| P1 | TP-CI-002 | ci | quality | cargo fmtとclippy warningが品質gateになっていない | [item](./items/ISS-20260505T000000Z-TP-CI-002-FMT-CLIPPY-GATE.md) |
| P1 | TP-ARCH-001 | architecture | architecture | Appがscene/input/problem/scroll/fontを抱えすぎている | [item](./items/ISS-20260505T000000Z-TP-ARCH-001-APP-GOD-OBJECT.md) |
| P1 | TP-ARCH-002 | architecture | architecture | backendごとにrendering pipelineが重複している | [item](./items/ISS-20260505T000000Z-TP-ARCH-002-RENDERER-DUPLICATION.md) |
| P1 | TP-TYPE-001 | core | type-safety | typing stateがi32 indexとunchecked castに依存している | [item](./items/ISS-20260505T000000Z-TP-TYPE-001-I32-CURSOR-INDICES.md) |
| P1 | TP-PERF-003 | performance | performance | UEFI backendがframeごとに大きなbufferを確保している | [item](./items/ISS-20260505T000000Z-TP-PERF-003-UEFI-FRAME-ALLOC.md) |
| P1 | TP-ERR-001 | error-handling | bug | platform backendに環境依存unwrapが多い | [item](./items/ISS-20260505T000000Z-TP-ERR-001-BACKEND-UNWRAPS.md) |
| P1 | TP-TUI-001 | tui | bug | TUI raw modeとalternate screenの復旧がRAII化されていない | [item](./items/ISS-20260505T000000Z-TP-TUI-001-TERMINAL-GUARD.md) |
| P1 | TP-WASM-001 | wasm | bug | WASM/webがDOMとstorage失敗をunwrapまたはsilent fallbackにしている | [item](./items/ISS-20260505T000000Z-TP-WASM-001-DOM-STORAGE-ERRORS.md) |
| P1 | TP-WEB-001 | web | ux | web版が初回pointerdownでclipboardを書き換える | [item](./items/ISS-20260505T000000Z-TP-WEB-001-CLIPBOARD-SIDE-EFFECT.md) |
| P1 | TP-UEFI-001 | uefi | bug | UEFI backendがfirmware API unwrapと粗いtimestampに依存している | [item](./items/ISS-20260505T000000Z-TP-UEFI-001-FIRMWARE-PANIC-TIME.md) |
| P1 | TP-SEC-001 | security | security | serve.jsのpath traversal guardがprefix checkに依存している | [item](./items/ISS-20260505T000000Z-TP-SEC-001-DEV-SERVER-PATH.md) |
| P2 | TP-CI-003 | ci | quality | CI workflowがmulti-target feature matrixを十分に固定していない | [item](./items/ISS-20260505T000000Z-TP-CI-003-WORKFLOW-MATRIX-ACTIONS.md) |
| P2 | TP-ARCH-003 | architecture | architecture | public fieldがprivate型を露出しAPI境界が崩れている | [item](./items/ISS-20260505T000000Z-TP-ARCH-003-PUBLIC-PRIVATE-INTERFACE.md) |
| P2 | TP-PERF-002 | performance | performance | text measurementとrasterizationが再計算されやすい | [item](./items/ISS-20260505T000000Z-TP-PERF-002-TEXT-MEASURE-CACHE.md) |
| P2 | TP-MOBILE-001 | mobile | architecture | mobile backendがUI callback内でArc<Mutex<App>>とunwrapに依存している | [item](./items/ISS-20260505T000000Z-TP-MOBILE-001-MUTEX-UI-STATE.md) |
| P2 | TP-DOC-002 | docs | docs | 生成物とsource artifactの管理境界が曖昧 | [item](./items/ISS-20260505T000000Z-TP-DOC-002-GENERATED-ARTIFACTS.md) |
| P2 | TP-SEC-002 | security | security | debug logger serverにdev-only境界と上限がない | [item](./items/ISS-20260505T000000Z-TP-SEC-002-LOGGER-SERVER-DEV-ONLY.md) |
| P2 | TP-BUILD-001 | build | build | build.rsがunwrapとrerun-if-changed不足に依存している | [item](./items/ISS-20260505T000000Z-TP-BUILD-001-BUILD-RS-UNWRAP-RERUN.md) |
| P2 | TP-BUILD-002 | build | build | UEFI実行scriptsのpath解決と破壊的操作が安全境界として弱い | [item](./items/ISS-20260505T000000Z-TP-BUILD-002-UEFI-SCRIPTS-SAFETY.md) |
| P3 | TP-DOC-003 | docs | docs | Cargo.tomlやsource commentsがmojibakeしている | [item](./items/ISS-20260505T000000Z-TP-DOC-003-CARGO-COMMENT-MOJIBAKE.md) |

## resolved

| status | legacy id | title | file |
|---|---|---|---|
| verified | TP-CI-001 | WASM debug buildがWEBSOCKET_ADDRESS未設定でcompile不能になる | [item](./items/ISS-20260505T000000Z-TP-CI-001-WASM-DEBUG-ENV.md) |
| verified | TP-ARCH-001 | Appがscene/input/problem/scroll/fontを抱えすぎている | [item](./items/ISS-20260505T000000Z-TP-ARCH-001-APP-GOD-OBJECT.md) |
| verified | TP-ARCH-003 | public fieldがprivate型を露出しAPI境界が崩れている | [item](./items/ISS-20260505T000000Z-TP-ARCH-003-PUBLIC-PRIVATE-INTERFACE.md) |
| verified | TP-CORE-003 | parser以外のcore regression testが不足している | [item](./items/ISS-20260505T000000Z-TP-CORE-003-TEST-COVERAGE-GAP.md) |
| verified | TP-CORE-001 | parserがmalformed inputをdiagnosticとして返せない | [item](./items/ISS-20260505T000000Z-TP-CORE-001-PARSER-DIAGNOSTICS.md) |
| verified | TP-CORE-002 | typing inputがmodel値渡しと入力logに依存している | [item](./items/ISS-20260505T000000Z-TP-CORE-002-TYPING-MODEL-MOVE-LOGGING.md) |
| verified | TP-DOC-001 | READMEの問題記法が実装とずれている | [item](./items/ISS-20260505T000000Z-TP-DOC-001-README-NTQ-SYNTAX.md) |
| verified | TP-PERF-001 | Layout mappingがsessionごとに再構築される | [item](./items/ISS-20260505T000000Z-TP-PERF-001-LAYOUT-REBUILD.md) |
| verified | TP-STATIC-001 | 状態とcommandがraw number/raw stringに依存しenumとmatchの網羅性検査が効いていない | [item](./items/ISS-20260505T000000Z-TP-STATIC-001-ENUM-MATCH-STATE.md) |
| verified | TP-TYPE-001 | typing stateがi32 indexとunchecked castに依存している | [item](./items/ISS-20260505T000000Z-TP-TYPE-001-I32-CURSOR-INDICES.md) |
