// Exercises the real compiled WASM UI with a test-only IPC service and real HTML audio ended events.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('../dist/', import.meta.url)));
const siteRoot = resolve(fileURLToPath(new URL('../../../site/', import.meta.url)));
const config = JSON.parse(await readFile(new URL('../../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
const index = await readFile(resolve(root,'index.html'),'utf8');
// Tauri adds hashes for bundled inline bootstrap scripts when it serves the app assets.
const hashes = [...index.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)]
  .filter(match => match[1].trim()).map(match => "'sha256-"+createHash('sha256').update(match[1]).digest('base64')+"'");
const productionCsp = config.app.security.csp.replace("script-src 'self'","script-src 'self' "+hashes.join(' '));
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, 'http://localhost').pathname;
  if (pathname === '/favicon.ico') { response.writeHead(204).end(); return; }
  const isSite = pathname.startsWith('/download/');
  const source = isSite ? siteRoot : root;
  const suffix = isSite ? pathname.slice('/download'.length) : pathname;
  const path = resolve(source, '.' + suffix);
  if (path !== source && !path.startsWith(source + '/')) { response.writeHead(403).end(); return; }
  try {
    const file = (await stat(path)).isDirectory() ? resolve(path, 'index.html') : path;
    const types = { '.html': 'text/html', '.js': 'application/javascript', '.wasm': 'application/wasm', '.css': 'text/css', '.webp': 'image/webp' };
    response.setHeader('Content-Type', types[extname(file)] || 'application/octet-stream');
    if (extname(file) === '.html' && !isSite) response.setHeader('Content-Security-Policy',productionCsp);
    response.end(await readFile(file));
  } catch { response.writeHead(404).end(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
const url = `http://127.0.0.1:${server.address().port}`;
const wav = Buffer.alloc(44 + 6400 * 2);
wav.write('RIFF'); wav.writeUInt32LE(wav.length - 8, 4); wav.write('WAVEfmt ', 8);
wav.writeUInt32LE(16, 16); wav.writeUInt16LE(1, 20); wav.writeUInt16LE(1, 22);
wav.writeUInt32LE(16000, 24); wav.writeUInt32LE(32000, 28); wav.writeUInt16LE(2, 32); wav.writeUInt16LE(16, 34);
wav.write('data', 36); wav.writeUInt32LE(wav.length - 44, 40);
const browser = await chromium.launch({
  ...(process.env.SEAMONE_CHROME ? { executablePath: process.env.SEAMONE_CHROME } : {}),
  args: ['--autoplay-policy=no-user-gesture-required'],
});
let completed = 0;
async function captureReview(page, state) {
  if (process.env.SEAMONE_UI_SCREENSHOTS) {
    await page.screenshot({ path:resolve(process.env.SEAMONE_UI_SCREENSHOTS,`seamone-${state}.png`) });
  }
}
async function fixture({ infoSeen = true, invalidFirst = false, quick = false, long = false, branch = false, microphone = false, phone = false, openRecording = true, firstTranscription = false, emptyLibrary = false, inferenceAvailable = false, failInitialLibrary = false, holdInitialLibrary = false } = {}) {
  const page = await browser.newPage(phone ? {viewport:{width:390,height:844},hasTouch:true,isMobile:true} : firstTranscription ? {viewport:{width:800,height:600}} : {});
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type()==='error') errors.push(message.text()); });
  await page.addInitScript(({ wav, infoSeen, invalidFirst, quick, long, branch, microphone, firstTranscription, emptyLibrary, inferenceAvailable, failInitialLibrary, holdInitialLibrary }) => {
    const recording = { id: 'r', name: '시험 녹음.wav', samples: long ? 512000 : quick ? 102400 : 19200, revision: 1, engine: 'test', model: 'fixture' };
    const segments = ['첫 번째 자동 전사문', '두 번째 자동 전사문', '세 번째 자동 전사문'].map((text, i) => ({
      id: `s${i + 1}`, recording_id: 'r', recording_revision: 1, revision: 1, range: { start: i * 6400, end: (i + 1) * 6400 },
      machine_text: text, text, verdict: null, needs_correction: false, verified: null,
    }));
    if (invalidFirst) segments[0].range.end = 0;
    if (quick) {
      segments[0].range = {start:0,end:24000};
      segments[0].text = '첫 번째 최신전사문';
      segments[0].needs_correction = true;
      segments[1].range = {start:27200,end:52800};
      segments[2].range = long ? {start:473600,end:505600} : {start:64000,end:80000};
    }
    if (branch) {
      // The latest line starts 20000 samples in, so the 1-second lookback starts at 4000.
      segments[1].range.start = 20000;
      segments[1].machine_timing = [
        { text:'두 번째 ', range:{start:20000,end:22400} },
        { text:'자동 전사문', range:{start:22400,end:52800} },
      ];
      // The edited first line must never use the machine's token locations.
      segments[0].machine_timing = [
        { text:segments[0].machine_text, range:{start:1600,end:23000} },
      ];
    }
    if (firstTranscription) {
      recording.engine = '아직 받아쓰지 않음'; recording.model = '';
      segments.splice(0);
    }
    const restored = firstTranscription && sessionStorage.getItem('first-transcription');
    if (restored) {
      const saved = JSON.parse(restored);
      Object.assign(recording, saved.recording);
      segments.splice(0, segments.length, ...saved.segments);
    }
    const proofs = new Map(); const events = new Map(); const batches = new Map();
    const state = { segments, recordings: emptyLibrary && !restored ? [] : [recording], saves: 0, delayNextSave: 0, failNextSave: false, infoSeen, exports: 0,
      microphone: { state: microphone ? 'idle' : 'unavailable' }, finishCalls: [], failMicSave: false, nextImport: null,
      inferenceAvailable, modelReady: false, nextModel: 'cancel', modelCalls: 0, transcribeCalls: [],
      holdTranscription: false, failTranscription: null, failImport: null,
      failLibrary: failInitialLibrary ? '보관함 초기 조회 실패' : null, holdLibrary: holdInitialLibrary,
      marks: [], markCalls: [], rangeCalls: [], reviewCalls: 0, endedCalls: 0, holdNextRange: false, failNextMark: false, lostMarkReply: false };
    window.__REVIEW_TEST__ = state;
    const copy = value => JSON.parse(JSON.stringify(value));
    const persist = () => {
      if (firstTranscription) sessionStorage.setItem('first-transcription', JSON.stringify({
        recording: state.recordings.find(r => r.id === segments[0]?.recording_id) || state.recordings[0],
        segments,
      }));
    };
    const rangeWav = range => {
      const wave = new Uint8Array(44 + (range.end - range.start) * 2);
      wave.set(Uint8Array.from(atob(wav.slice(0,60)).slice(0,44), char => char.charCodeAt(0)));
      const header = new DataView(wave.buffer);
      header.setUint32(4, wave.length - 8, true); header.setUint32(40, wave.length - 44, true);
      if (branch) {
        const pattern = [0, 24000, 0, -24000]; // Tone only in unit 4, 4 kHz at 16 kHz.
        for (let i = Math.max(range.start,20000); i < Math.min(range.end,24000); i++) {
          header.setInt16(44 + (i-range.start)*2,pattern[i%4],true);
        }
      }
      let binary = '';
      for (let i = 0; i < wave.length; i += 8192) binary += String.fromCharCode(...wave.subarray(i,i+8192));
      return btoa(binary);
    };
    window.__TAURI__ = { core: { invoke: async (command, args) => {
      if (args instanceof Map) throw 'IPC arguments must be plain JSON objects';
      if (command === 'get_library') {
        if (state.holdLibrary) await new Promise(resolve => { state.releaseLibrary = resolve; });
        if (state.failLibrary) { const error=state.failLibrary; state.failLibrary=null; throw error; }
        return { recordings: copy(state.recordings), info_seen: state.infoSeen, pending_training: segments.filter(s => s.verified).length, retry_training:0, inference_available: state.inferenceAvailable };
      }
      if (command === 'get_segments') {
        if (state.failSegments) { const error=state.failSegments; state.failSegments=null; throw error; }
        return { segments: copy(segments.filter(s => s.recording_id === args.recordingId)), next_offset: null };
      }
      if (command === 'get_capture_status') return copy(state.microphone);
      if (command === 'start_recording') {
        if (Object.keys(args).length) throw 'start takes no arguments';
        state.microphone = { state: 'starting', session_id: 'microphone-session' };
        return copy(state.microphone);
      }
      if (command === 'finish_recording') {
        if (args.sessionId !== state.microphone.session_id) throw 'stale microphone session';
        state.finishCalls.push(copy(args));
        await new Promise(resolve => { state.releaseMicFinish = resolve; });
        if (state.failMicSave) {
          state.failMicSave = false;
          state.microphone = { state: 'interrupted', session_id: args.sessionId, elapsed_seconds: 12, reason: '시험 저장 실패' };
          throw '시험 저장 실패';
        }
        state.microphone = { state: 'idle' };
        if (!args.save) return null;
        const saved = { ...recording, id: 'captured', name: '마이크 녹음.wav', engine: '아직 받아쓰지 않음', model: '' };
        state.recordings.unshift(saved);
        return saved;
      }
      if (command === 'import_audio') {
        if (Object.keys(args).length) throw 'import takes no path or audio payload';
        if (state.failImport) { const error = state.failImport; state.failImport = null; throw error; }
        const filename = state.nextImport;
        state.nextImport = null;
        if (!filename) return null;
        const saved = { ...recording, id: 'imported', name: filename, engine: '아직 받아쓰지 않음', model: '' };
        state.recordings.unshift(saved);
        return saved;
      }
      if (command === 'select_model') {
        state.modelCalls++;
        if (state.holdModel) await new Promise(resolve => { state.releaseModel = resolve; });
        if (Object.keys(args).length) throw 'model selection takes no path or payload';
        if (state.nextModel === 'cancel') return false;
        if (state.nextModel === 'invalid') throw 'Whisper small 모델의 SHA-256이 맞지 않습니다.';
        state.modelReady = true;
        return true;
      }
      if (command === 'transcribe_recording') {
        state.transcribeCalls.push(copy(args));
        if (!state.inferenceAvailable) throw '이 설치본에는 Whisper 엔진이 없습니다.';
        if (!state.modelReady) throw "먼저 'Whisper 모델 선택'에서 공식 ggml-small.bin을 고르세요. 앱은 모델을 내려받지 않습니다.";
        const selected = state.recordings.find(r => r.id === args.recordingId);
        if (!selected || selected.engine !== '아직 받아쓰지 않음') throw '이미 받아쓴 녹음입니다. 기존 수정본을 그대로 사용하세요.';
        if (state.holdTranscription) await new Promise(resolve => { state.releaseTranscription = resolve; });
        if (state.failTranscription) { const error = state.failTranscription; state.failTranscription = null; throw error; }
        const result = state.emptyTranscription ? [] : ['첫 번째 자동 전사문', '두 번째 자동 전사문', '세 번째 자동 전사문'].map((text, i) => ({
          id: `s${i + 1}`, recording_id: args.recordingId, recording_revision: selected.revision,
          revision: 1, range: {start:i * 6400, end:(i + 1) * 6400},
          machine_text:text, text, verdict:null, needs_correction:false, verified:null,
        }));
        segments.splice(0, segments.length, ...result);
        selected.engine = 'whisper-rs 0.16'; selected.model = 'fixture';
        persist();
        return copy(result);
      }
      if (command === 'get_error_marks') {
        if (state.failMarks) { const error=state.failMarks; state.failMarks=null; throw error; }
        return { marks: copy(state.marks), next_offset: null };
      }
      if (command === 'get_recording_playback') {
        state.rangeCalls.push(copy(args.range));
        if (state.holdNextRange) {
          state.holdNextRange = false;
          await new Promise(resolve => { state.releaseRange = resolve; });
          state.delayedRangeReturned = true;
        }
        return rangeWav(args.range);
      }
      if (command === 'save_error_marks') {
        const batch = copy(args.batch);
        state.markCalls.push(batch);
        if (batches.has(batch.id)) {
          if (JSON.stringify(batch) !== batches.get(batch.id)) throw '충돌';
          return;
        }
        if (state.failNextMark) { state.failNextMark = false; throw '시험 오류 구간 저장 실패'; }
        batches.set(batch.id, JSON.stringify(batch));
        batch.ranges.toSorted((a,b) => a.start-b.start).forEach((range, i) => state.marks.push({
          id:`${batch.id}-${i}`, range, recording_id:batch.recording_id,
          recording_revision:batch.recording_revision,
        }));
        for (const s of segments.filter(s => batch.ranges.some(range => s.range.start < range.end && range.start < s.range.end))) {
          s.error_marked_text = s.text; s.verified = null; s.revision++;
        }
        if (state.lostMarkReply) { state.lostMarkReply = false; throw '시험 응답 유실'; }
        return;
      }
      if (command === 'dismiss_info') { state.infoSeen = true; return; }
      if (command === 'export_text') { state.exports++; return segments.map(s => s.text).join('\n'); }
      if (command === 'save_text_file') { state.exports++; return true; }
      if (command === 'copy_saved_text') { state.exports++; return; }
      if (command === 'get_playback') {
        const s = segments.find(s => s.id === args.segmentId);
        if (s.revision !== args.revision) throw '다른 수정본이 저장됐습니다';
        if (s.range.start >= s.range.end) throw '재생 구간이 비었습니다';
        const token = crypto.randomUUID(); proofs.set(token, { id:s.id, revision:s.revision, complete:false });
        return { segment_id:s.id, revision:s.revision, wav_base64:rangeWav(s.range), token };
      }
      if (command === 'playback_ended') { state.endedCalls++; proofs.get(args.token).complete = true; return; }
      if (command === 'save_review') {
        state.reviewCalls++;
        const req = args.request;
        const canonical = JSON.stringify(req);
        if (events.has(req.event_id)) {
          const old = events.get(req.event_id); if (old.canonical !== canonical) throw '충돌'; return copy(old.saved);
        }
        const delay = state.delayNextSave; state.delayNextSave = 0;
        if (delay) await new Promise(resolve => setTimeout(resolve,delay));
        if (state.failNextSave) { state.failNextSave = false; throw '시험 저장 실패'; }
        const s = segments.find(s => s.id === req.segment_id);
        if (s.revision !== req.expected_revision) throw '다른 수정본이 저장됐습니다';
        if (['judge','confirm_correction','correct_and_confirm'].includes(req.action.kind)) {
          const p = proofs.get(args.proof);
          if (!p?.complete || p.id !== s.id || p.revision !== s.revision) throw '끝까지 들으세요';
        }
        const confirming=['confirm_correction','correct_and_confirm'].includes(req.action.kind);
        const corrected=req.action.kind==='correct_and_confirm' ? req.action.text : s.text;
        if (confirming && s.verdict==='no' && corrected.trim()===s.machine_text.trim()) throw '처음 판정을 취소하고 다시 검수하세요';
        if (confirming && s.error_marked_text?.trim()===corrected.trim()) throw '오류를 표시할 때 저장된 글과 같습니다';
        if (req.action.kind === 'save_draft' && req.action.text === s.text) return copy(s);
        s.revision++; s.verified = null;
        if (req.action.kind === 'judge') {
          s.verdict = req.action.verdict; s.needs_correction = s.verdict === 'no';
          if (s.verdict === 'yes') s.verified = { revision:s.revision,text:s.text,source:'accepted' };
        } else if (req.action.kind === 'save_draft') { s.text = req.action.text; s.needs_correction = true; }
        else if (req.action.kind === 'correct_and_confirm') { s.text=req.action.text; s.needs_correction = true; s.verified={ revision:s.revision,text:s.text,source:'corrected' }; }
        else if (req.action.kind === 'confirm_correction') s.verified = { revision:s.revision,text:s.text,source:'corrected' };
        else if (req.action.kind === 'dismiss_error_mark') delete s.error_marked_text;
        else if (req.action.kind === 'invalidate') { const unchanged=s.verdict==='no' && s.text.trim()===s.machine_text.trim(); s.verdict=null; if (unchanged) { s.text=s.machine_text; s.needs_correction=false; } else s.needs_correction ||= s.text !== s.machine_text; }
        persist(); state.saves++; events.set(req.event_id,{ canonical,saved:copy(s) }); return copy(s);
      }
      throw `Unexpected test command: ${command}`;
    } } };
  }, { wav: wav.toString('base64'), infoSeen, invalidFirst, quick, long, branch, microphone, firstTranscription, emptyLibrary, inferenceAvailable, failInitialLibrary, holdInitialLibrary });
  await page.goto(url);
  if (!openRecording) return { page, errors };
  try { await page.getByRole('button', { name: '시험 녹음.wav', exact: true }).click({timeout:10000}); }
  catch (error) { throw new Error([...errors,String(error)].join('\n')); }
  await expect(page.getByText(quick ? '첫 번째 최신전사문' : '첫 번째 자동 전사문', { exact: true })).toBeVisible();
  return { page, errors };
}
async function finish(page, errors, name) {
  expect(errors, name).toEqual([]); await page.close(); completed++; console.log(`PASS ${name}`);
}
async function startBinary(page) {
  await page.getByText('조각별 검증', {exact:true}).click();
  await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
}
const quickAudio = page => page.locator('audio[aria-label="빠른 검수 음성"]');
const startQuick = page => page.getByRole('button',{ name:'빠른 검수 시작',exact:true }).click();
const bar = (page, kind, start, end) => page.locator(
  `.timeline-bar.${kind}[data-start="${start}"][data-end="${end}"]`);
const clipRanges = (page, kind) => page.locator(`.timeline-bar.${kind}`).evaluateAll(nodes =>
  nodes.map(node => [Number(node.dataset.start), Number(node.dataset.end)]));
const readState = (page, key) => page.evaluate(key => window.__REVIEW_TEST__[key],key);
async function seekAndPause(page, seconds) {
  const audio = quickAudio(page);
  await expect.poll(() => audio.evaluate(node => node.readyState)).toBeGreaterThan(0);
  await audio.evaluate((node, at) => { node.pause(); node.currentTime = at; },seconds);
}
try {
  for (const inferenceAvailable of [true,false]) {
    const {page, errors} = await fixture({firstTranscription:true,emptyLibrary:true,openRecording:false,inferenceAvailable,failInitialLibrary:true,holdInitialLibrary:true});
    await expect(page.getByRole('heading',{name:'보관함을 불러오는 중입니다.',exact:true})).toBeVisible();
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releaseLibrary)).toBe('function');
    await page.evaluate(() => { const s=window.__REVIEW_TEST__; s.holdLibrary=false; s.releaseLibrary(); });
    await expect(page.getByRole('heading',{name:'보관함을 불러오지 못했습니다',exact:true})).toBeVisible();
    const libraryError=page.getByRole('alert',{name:'보관함 조회 오류',exact:true});
    await expect(libraryError).toContainText('보관함 초기 조회 실패');
    await page.getByRole('button',{name:'개인정보와 학습 안내',exact:true}).click();
    await expect(page.getByText(/보관함 정보를 확인한 뒤 이 기기의 받아쓰기 지원 여부/)).toBeVisible();
    await expect(page.getByText(/새 받아쓰기는 Whisper 엔진이 포함된 데스크톱/)).toHaveCount(0);
    await page.getByRole('button',{name:'안내 닫기',exact:true}).click();
    await page.evaluate(() => { const s=window.__REVIEW_TEST__; s.nextImport='초기 조회 뒤 가져온 음성.wav'; s.failLibrary='가져온 뒤 보관함 조회 실패'; });
    await page.getByRole('button',{name:'음성 파일 가져오기',exact:true}).click();
    await expect(libraryError).toContainText('가져온 뒤 보관함 조회 실패');
    const prepare=page.getByRole('region',{name:'받아쓰기 준비',exact:true});
    await expect(prepare.getByText(/엔진 지원 여부를 확인하지 못했습니다/)).toBeVisible();
    await expect(page.getByText(/새 받아쓰기는 Whisper 엔진이 포함된 데스크톱/)).toHaveCount(0);
    await expect(page.getByText('선택한 녹음: 초기 조회 뒤 가져온 음성.wav',{exact:true})).toBeVisible();
    await expect(page.getByRole('status',{name:'녹음 저장 안내',exact:true})).toHaveText('초기 조회 뒤 가져온 음성.wav · 녹음 보관함에 저장했습니다.');
    await expect(page.getByRole('button',{name:'저장한 녹음 열기',exact:true})).toHaveCount(0);
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.holdLibrary=true; });
    await page.getByRole('button',{name:'보관함 다시 불러오기',exact:true}).click();
    await expect(page.getByRole('status',{name:'보관함 조회 안내',exact:true})).toContainText('불러오는 중');
    await expect(libraryError).toHaveCount(0);
    await expect(prepare.getByText(/보관함을 불러온 뒤 엔진 지원 여부/)).toBeVisible();
    await page.evaluate(() => { const s=window.__REVIEW_TEST__; s.holdLibrary=false; s.releaseLibrary(); });
    await expect(page.getByRole('status',{name:'보관함 조회 안내',exact:true})).toHaveCount(0);
    await expect(page.getByRole('button',{name:'초기 조회 뒤 가져온 음성.wav',exact:true})).toHaveAttribute('aria-pressed','true');
    if (inferenceAvailable) {
      await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeEnabled();
      await expect(prepare.getByRole('button',{name:'Whisper 모델 선택',exact:true})).toBeEnabled();
      await expect(page.getByText(/새 받아쓰기는 Whisper 엔진이 포함된 데스크톱/)).toHaveCount(0);
    } else {
      await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
      await expect(prepare.getByText(/새 받아쓰기는 Whisper 엔진이 포함된 데스크톱 설치본/)).toBeVisible();
    }
    await finish(page,errors,`initial library failure, import with unknown capability and explicit recovery: engine=${inferenceAvailable}`);
  }
  for (const failure of ['failSegments','failMarks']) {
    const {page, errors} = await fixture({openRecording:false,inferenceAvailable:true});
    await page.evaluate(key => { window.__REVIEW_TEST__[key] = '저장한 전사를 불러오지 못했습니다.'; },failure);
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await expect(page.getByRole('alert')).toHaveText('저장한 전사를 불러오지 못했습니다.');
    await expect(page.getByText('받아쓰기를 마쳤지만 인식된 글이 없습니다.',{exact:true})).toHaveCount(0);
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
    await page.getByRole('button',{name:'수정본 새로 불러오기',exact:true}).click();
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(page.getByRole('alert')).toBeEmpty();
    await finish(page,errors,`saved transcript load failure recovers without presenting an empty result: ${failure}`);
  }
  for (const failed of [false,true]) {
    const {page, errors} = await fixture({firstTranscription:true,inferenceAvailable:true,microphone:true,openRecording:false});
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await page.evaluate(() => { window.__REVIEW_TEST__.nextModel='valid'; });
    await page.getByRole('region',{name:'받아쓰기 준비',exact:true}).getByRole('button',{name:'Whisper 모델 선택',exact:true}).click();
    await page.getByRole('button',{name:'녹음 시작',exact:true}).click();
    await page.evaluate(failed => { const s=window.__REVIEW_TEST__; s.microphone={state:'recording',session_id:'microphone-session',elapsed_seconds:1}; s.holdTranscription=true; if (failed) s.failTranscription='시험 전사 실패'; },failed);
    await page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true}).click();
    await page.getByRole('button',{name:'멈추고 저장',exact:true}).click();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('status',{name:'작업 안내',exact:true})).toContainText('기기 안에서 받아쓰는 중');
    await expect(page.getByRole('status',{name:'녹음 저장 안내',exact:true})).toContainText('마이크 녹음.wav · 녹음 보관함에 저장했습니다.');
    await expect(page.getByRole('button',{name:'시험 녹음.wav',exact:true})).toHaveAttribute('aria-pressed','true');
    await expect(page.getByRole('button',{name:'마이크 녹음.wav',exact:true})).toHaveAttribute('aria-pressed','false');
    await page.evaluate(() => window.__REVIEW_TEST__.releaseTranscription());
    if (failed) {
      await expect(page.getByRole('alert')).toHaveText('시험 전사 실패');
      await expect(page.getByRole('status',{name:'작업 안내',exact:true})).toContainText('오류 안내를 확인하세요');
    } else {
      await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
      await expect(page.getByRole('status',{name:'작업 안내',exact:true})).toContainText('받아쓰기를 마쳤습니다.');
    }
    await expect(page.getByRole('status').filter({hasText:'마이크 녹음.wav · 녹음 보관함에 저장했습니다.'})).toBeVisible();
    await page.getByRole('button',{name:'저장한 녹음 열기',exact:true}).click();
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeEnabled();
    await finish(page,errors,`saved microphone notice survives the transcription result and opens the saved recording: failed=${failed}`);
  }
  for (const outcome of ['valid','cancel','invalid']) {
    const {page, errors} = await fixture({firstTranscription:true,inferenceAvailable:true,microphone:true,openRecording:false});
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await page.getByRole('button',{name:'녹음 시작',exact:true}).click();
    await page.evaluate(outcome => {
      const s=window.__REVIEW_TEST__;
      s.microphone={state:'recording',session_id:'microphone-session',elapsed_seconds:1};
      s.holdModel=true; s.nextModel=outcome;
    },outcome);
    await page.getByRole('region',{name:'받아쓰기 준비',exact:true}).getByRole('button',{name:'Whisper 모델 선택',exact:true}).click();
    const operation=page.getByRole('status',{name:'작업 안내',exact:true});
    await expect(operation).toHaveText('모델 파일을 확인하는 중입니다.');
    await page.getByRole('button',{name:'멈추고 저장',exact:true}).click();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    const saved=page.getByRole('status',{name:'녹음 저장 안내',exact:true});
    await expect(saved).toContainText('마이크 녹음.wav · 녹음 보관함에 저장했습니다.');
    const openSaved=page.getByRole('button',{name:'저장한 녹음 열기',exact:true});
    await expect(openSaved).toBeDisabled();
    await expect(operation).toHaveText('모델 파일을 확인하는 중입니다.');
    await page.evaluate(() => window.__REVIEW_TEST__.releaseModel());
    if (outcome==='valid') await expect(operation).toContainText('무결성을 확인했습니다.');
    else if (outcome==='cancel') await expect(operation).toContainText('모델 파일 선택을 취소했습니다.');
    else await expect(page.getByRole('alert')).toContainText('SHA-256');
    await expect(saved).toContainText('마이크 녹음.wav');
    await expect(openSaved).toBeEnabled();
    await expect(page.getByRole('button',{name:'시험 녹음.wav',exact:true})).toHaveAttribute('aria-pressed','true');
    await openSaved.click();
    await expect(page.getByRole('button',{name:'마이크 녹음.wav',exact:true})).toHaveAttribute('aria-pressed','true');
    await expect(saved).toHaveText('마이크 녹음.wav · 녹음 보관함에 저장했습니다.');
    await expect(openSaved).toHaveCount(0);
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await expect(openSaved).toBeEnabled();
    await expect(saved).toContainText('저장한 녹음 열기로 확인하세요.');
    await page.getByRole('button',{name:'저장 안내 닫기',exact:true}).click();
    await expect(saved).toHaveCount(0);
    await finish(page,errors,`saved microphone notice survives the model picker result: ${outcome}`);
  }
  for (const emptyResult of [false,true]) {
    const {page, errors} = await fixture({firstTranscription:true,inferenceAvailable:true,openRecording:false});
    await page.evaluate(empty => { window.__REVIEW_TEST__.emptyTranscription=empty; window.__REVIEW_TEST__.nextImport='목록 갱신 시험.wav'; },emptyResult);
    await page.getByRole('button',{name:'음성 파일 가져오기',exact:true}).click();
    const imported=page.getByRole('button',{name:'목록 갱신 시험.wav',exact:true});
    await expect(imported).toHaveAttribute('aria-pressed','true');
    await page.evaluate(() => { window.__REVIEW_TEST__.nextModel='valid'; });
    await page.getByRole('region',{name:'받아쓰기 준비',exact:true}).getByRole('button',{name:'Whisper 모델 선택',exact:true}).click();
    await page.evaluate(() => { window.__REVIEW_TEST__.failLibrary='보관함 목록을 불러오지 못했습니다.'; });
    await page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true}).click();
    await expect(page.getByRole('alert',{name:'보관함 조회 오류',exact:true})).toHaveText('보관함 목록을 불러오지 못했습니다.');
    await expect(page.getByText('기존 보관함 목록은 그대로 표시합니다.',{exact:true})).toBeVisible();
    await expect(imported).toHaveAttribute('aria-pressed','true');
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await expect(page.getByRole('region',{name:'받아쓰기 준비',exact:true}).getByRole('button',{name:'Whisper 모델 선택',exact:true})).toBeEnabled();
    await imported.click();
    await expect(page.getByText(emptyResult ? '받아쓰기를 마쳤지만 인식된 글이 없습니다.' : '첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
    if (emptyResult) await expect(page.getByRole('button',{name:'빠른 검수 시작',exact:true})).toBeDisabled();
    else await expect(page.getByRole('button',{name:'빠른 검수 시작',exact:true})).toBeEnabled();
    await finish(page,errors,`completed recording survives a failed library refresh and navigating away: empty=${emptyResult}`);
  }
  {
    const {page, errors} = await fixture({firstTranscription:true,emptyLibrary:true,inferenceAvailable:true,openRecording:false});
    await expect(page.getByRole('heading',{name:'첫 녹음을 추가하세요',exact:true})).toBeVisible();
    await expect(page.getByText('녹음과 전사는 이 기기에 저장합니다.',{exact:true})).toBeVisible();
    const importAudio = page.getByRole('button',{name:'음성 파일 가져오기',exact:true});
    await page.evaluate(() => { window.__REVIEW_TEST__.failImport = '음성 파일을 읽지 못했습니다.'; });
    await importAudio.click();
    await expect(page.getByRole('alert')).toHaveText('음성 파일을 읽지 못했습니다.');
    await expect(page.getByText(/WAV, MP3, AAC-LC M4A 파일을 다시 선택/)).toBeVisible();
    await importAudio.click(); // Native picker cancelled.
    await expect(page.getByRole('heading',{name:'첫 녹음을 추가하세요',exact:true})).toBeVisible();
    await page.evaluate(() => { window.__REVIEW_TEST__.nextImport = '첫 파일.wav'; });
    await importAudio.click();
    const recording = page.getByRole('button',{name:'첫 파일.wav',exact:true});
    await expect(recording).toHaveAttribute('aria-pressed','true');
    const prepare = page.getByRole('region',{name:'받아쓰기 준비',exact:true});
    await expect(prepare.getByText(/Whisper small 검증용 모델 ggml-small.bin/)).toBeVisible();
    const transcribe = page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true});
    await transcribe.click();
    await expect(page.getByRole('alert')).toContainText('먼저');
    await expect(recording).toHaveAttribute('aria-pressed','true');
    const choose = prepare.getByRole('button',{name:'Whisper 모델 선택',exact:true});
    await page.evaluate(() => { window.__REVIEW_TEST__.nextModel = 'invalid'; });
    await choose.click();
    await expect(page.getByRole('alert')).toContainText('SHA-256');
    await expect(recording).toHaveAttribute('aria-pressed','true');
    await page.evaluate(() => { window.__REVIEW_TEST__.nextModel = 'cancel'; });
    await choose.click();
    await expect(choose).toBeEnabled();
    expect(await readState(page,'transcribeCalls')).toEqual([{recordingId:'imported'}]);
    await page.evaluate(() => { window.__REVIEW_TEST__.nextModel = 'valid'; });
    await choose.click();
    await expect(page.getByRole('status',{name:'작업 안내',exact:true})).toHaveText(/무결성/);
    await expect(page.getByRole('alert')).toBeEmpty();
    await expect(recording).toHaveAttribute('aria-pressed','true');
    await page.evaluate(() => { window.__REVIEW_TEST__.holdTranscription = true; });
    const retry = page.getByRole('button',{name:'같은 녹음 다시 받아쓰기',exact:true});
    await retry.click();
    const pending = page.getByRole('button',{name:'기기 안에서 받아쓰는 중',exact:true});
    await expect(pending).toBeDisabled();
    await expect(page.getByRole('region',{name:'저장한 글',exact:true})).toHaveAttribute('aria-busy','true');
    await expect(page.getByRole('status',{name:'작업 안내',exact:true})).toContainText('기기 안에서 받아쓰는 중');
    await expect(page.getByRole('status',{name:'작업 안내',exact:true})).not.toContainText('%');
    await expect(recording).toBeDisabled();
    await expect(choose).toBeDisabled();
    await pending.dispatchEvent('click'); await pending.dispatchEvent('click');
    expect(await readState(page,'transcribeCalls')).toEqual([{recordingId:'imported'},{recordingId:'imported'}]);
    await captureReview(page,'first-transcription-pending');
    await page.evaluate(() => window.__REVIEW_TEST__.releaseTranscription());
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
    await expect(page.getByRole('button',{name:'빠른 검수 시작',exact:true})).toBeEnabled();
    await expect(page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true})).toBeEnabled();
    await expect(recording.getByText(/whisper-rs 0.16/)).toBeVisible();
    await captureReview(page,'first-transcription-result');
    await page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true}).dispatchEvent('click');
    expect(await readState(page,'transcribeCalls')).toEqual([{recordingId:'imported'},{recordingId:'imported'}]);
    await startQuick(page);
    await expect(quickAudio(page)).toBeAttached();
    await page.keyboard.press('Escape');
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await page.getByRole('button',{name:'텍스트 파일로 저장',exact:true}).click();
    expect(await readState(page,'exports')).toBe(1);
    await page.getByRole('button',{name:'이 조각 재생, 편집',exact:true}).first().click();
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await page.getByLabel('수정 초안').fill('첫 전사를 고친 초안');
    await page.evaluate(() => { const s=window.__REVIEW_TEST__; s.holdLibrary=true; s.releaseLibrary=null; });
    await page.getByRole('button',{name:'초안 저장',exact:true}).click();
    await expect.poll(async () => (await readState(page,'segments'))[0].text).toBe('첫 전사를 고친 초안');
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releaseLibrary)).toBe('function');
    await expect(page.getByRole('status',{name:'보관함 조회 안내',exact:true})).toHaveCount(0);
    await expect(recording).toBeVisible();
    await expect(page.getByLabel('수정 초안')).toHaveValue('첫 전사를 고친 초안');
    await page.evaluate(() => { const s=window.__REVIEW_TEST__; s.holdLibrary=false; s.releaseLibrary(); });
    expect((await readState(page,'segments'))[0].verified).toBeNull();
    await page.reload();
    await page.getByRole('button',{name:'첫 파일.wav',exact:true}).click();
    await expect(page.getByText('첫 전사를 고친 초안',{exact:true})).toBeVisible();
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(page.getByLabel('수정 초안')).toHaveValue('첫 전사를 고친 초안');
    expect((await readState(page,'segments'))[0].verified).toBeNull();
    await finish(page,errors,'first import, missing/corrupt/cancelled model selection, duplicate guard, result and reopened draft');
  }
  {
    const {page, errors} = await fixture({firstTranscription:true,emptyLibrary:true,inferenceAvailable:true,microphone:true,openRecording:false});
    await page.getByRole('button',{name:'녹음 시작',exact:true}).click();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'recording',session_id:'microphone-session',elapsed_seconds:1}; });
    await page.getByRole('button',{name:'멈추고 저장',exact:true}).click();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    const saved = page.getByRole('button',{name:'마이크 녹음.wav',exact:true});
    await expect(saved).toHaveAttribute('aria-pressed','true');
    await page.evaluate(() => { const s = window.__REVIEW_TEST__; s.nextModel = 'valid'; s.holdTranscription = true; s.failTranscription = '엔진이 받아쓰기를 마치지 못했습니다.'; });
    await page.getByRole('region',{name:'받아쓰기 준비',exact:true}).getByRole('button',{name:'Whisper 모델 선택',exact:true}).click();
    await page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true}).click();
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰는 중',exact:true})).toBeDisabled();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseTranscription());
    await expect(page.getByRole('alert')).toHaveText('엔진이 받아쓰기를 마치지 못했습니다.');
    await expect(saved).toHaveAttribute('aria-pressed','true');
    await page.evaluate(() => { window.__REVIEW_TEST__.holdTranscription = false; });
    await page.getByRole('button',{name:'같은 녹음 다시 받아쓰기',exact:true}).click();
    await expect(page.getByRole('alert')).toBeEmpty();
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    expect(await readState(page,'transcribeCalls')).toEqual([{recordingId:'captured'},{recordingId:'captured'}]);
    expect(await readState(page,'finishCalls')).toEqual([{sessionId:'microphone-session',save:true}]);
    await finish(page,errors,'saved microphone recording survives engine failure and retries without recording again');
  }
  {
    const {page, errors} = await fixture({firstTranscription:true,inferenceAvailable:true,openRecording:false});
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await page.evaluate(() => { const s=window.__REVIEW_TEST__; s.nextModel='valid'; s.emptyTranscription=true; });
    await page.getByRole('region',{name:'받아쓰기 준비',exact:true}).getByRole('button',{name:'Whisper 모델 선택',exact:true}).click();
    await page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true}).click();
    await expect(page.getByText('받아쓰기를 마쳤지만 인식된 글이 없습니다.',{exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
    await expect(page.getByRole('button',{name:'빠른 검수 시작',exact:true})).toBeDisabled();
    await page.reload();
    await page.getByRole('button',{name:'시험 녹음.wav',exact:true}).click();
    await expect(page.getByText('받아쓰기를 마쳤지만 인식된 글이 없습니다.',{exact:true})).toBeVisible();
    await finish(page,errors,'completed empty transcription stays distinct from a recording waiting for its first transcription');
  }
  {
    const {page, errors} = await fixture({microphone: true});
    const start = page.getByRole('button', {name:'녹음 시작', exact:true});
    const stop = page.getByRole('button', {name:'멈추고 저장', exact:true});
    const cancel = page.getByRole('button', {name:'녹음 취소', exact:true});
    await start.click();
    await expect(stop).toBeDisabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'failed', message:'마이크 접근이 허용되지 않았습니다.'}; });
    await expect(page.getByText('마이크 접근이 허용되지 않았습니다.', {exact:true})).toBeVisible();
    await expect(page.getByText(/마이크 권한과 연결을 확인한 뒤 녹음 시작/)).toBeVisible();

    await start.click();
    await cancel.click();
    await expect(page.getByText('녹음을 취소하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByText('녹음을 취소했습니다.')).toBeVisible();
    await expect(page.getByRole('button', {name:'마이크 녹음.wav'})).toHaveCount(0);

    await start.click();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'recording', session_id:'microphone-session', elapsed_seconds:12}; });
    await expect(page.getByText('녹음 중 00:12 / 30:00')).toBeVisible();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'interrupted', session_id:'microphone-session', elapsed_seconds:12, reason:'마이크 연결이 끊겼습니다.'}; });
    await expect(page.getByText(/마이크 연결이 끊겼습니다.*저장하거나 취소한 뒤 창을 닫을 수 있습니다/)).toBeVisible();
    await page.evaluate(() => { window.__REVIEW_TEST__.failMicSave = true; });
    await stop.click();
    await expect(page.getByText('녹음을 저장하는 중입니다.')).toBeVisible();
    await expect(cancel).toBeDisabled();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('alert').filter({hasText:'시험 저장 실패'})).toBeVisible();
    await stop.click();
    await expect(page.getByText('녹음을 저장하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('button', {name:'마이크 녹음.wav'})).toHaveAttribute('aria-pressed','true');

    await start.click();
    await expect(cancel).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'limit_reached', session_id:'microphone-session'}; });
    await expect(page.getByText(/30분 제한에 도달했습니다/)).toBeVisible();
    await cancel.click();
    await expect(page.getByText('녹음을 취소하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(start).toBeEnabled();
    expect(await page.evaluate(() => window.__REVIEW_TEST__.finishCalls.map(c => c.save))).toEqual([false,true,true,false]);
    await finish(page,errors,'desktop microphone controls, interruption, retry and duration limit');
  }
  {
    const {page, errors} = await fixture({microphone: true});
    await page.getByRole('button', {name:'녹음 시작', exact:true}).click();
    await page.evaluate(() => { window.__REVIEW_TEST__.microphone = {state:'recording', session_id:'microphone-session', elapsed_seconds:12}; });
    const stop = page.getByRole('button', {name:'멈추고 저장', exact:true});
    await expect(stop).toBeEnabled();
    await page.getByRole('button', {name:'이 조각 재생, 편집'}).first().click();
    const draft = page.getByLabel('수정 초안');
    await draft.fill('아직 저장하지 않은 편집');
    await stop.click();
    await expect(page.getByText('녹음을 저장하는 중입니다.')).toBeVisible();
    await page.evaluate(() => window.__REVIEW_TEST__.releaseMicFinish());
    await expect(page.getByRole('status').filter({hasText:'녹음 보관함에 저장했습니다.'})).toBeVisible();
    await expect(draft).toHaveValue('아직 저장하지 않은 편집');
    await draft.press('Escape');
    await expect(page.getByRole('button', {name:'마이크 녹음.wav'})).toBeVisible();
    await finish(page,errors,'saving a recording leaves the open transcript draft in place');
  }
  {
    const {page, errors} = await fixture();
    await expect(page.getByRole('heading', {name:'마이크 녹음'})).toHaveCount(0);
    const importButton = page.getByRole('button', {name:'음성 파일 가져오기'});
    await page.evaluate(() => { window.__REVIEW_TEST__.failImport = '음성 파일을 읽지 못했습니다.'; });
    await importButton.click();
    await expect(page.getByRole('alert')).toHaveText('음성 파일을 읽지 못했습니다.');
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await importButton.click();
    await expect(importButton).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.nextImport = '선택한 음성.m4a'; });
    await importButton.click();
    await expect(page.getByRole('button', {name:'선택한 음성.m4a'})).toHaveAttribute('aria-pressed','true');
    await expect(page.getByRole('button', {name:'기기 안에서 받아쓰기',exact:true})).toBeDisabled();
    await expect(page.getByText(/새 받아쓰기는 Whisper 엔진이 포함된 데스크톱 설치본/)).toBeVisible();
    await finish(page,errors,'file picker cancellation and successful import with microphone unavailable');
  }
  {
    const { page, errors } = await fixture({ infoSeen:false });
    await expect(page.getByRole('heading', { name:'기기에 남는 음성과 글' })).toBeVisible();
    await page.getByRole('button',{ name:'텍스트 파일로 저장',exact:true }).click();
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.exports)).toBe(1);
    await page.getByRole('button',{ name:'저장한 글 복사',exact:true }).click();
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.exports)).toBe(2);
    await page.getByRole('button',{ name:'안내 닫기',exact:true }).click();
    await page.getByRole('button',{ name:'개인정보와 학습 안내',exact:true }).click();
    await expect(page.getByRole('heading',{ name:'기기에 남는 음성과 글' })).toBeVisible();
    await finish(page,errors,'first-launch information is revisitable and does not gate export');
  }
  {
    const { page, errors } = await fixture({infoSeen:false,microphone:true,openRecording:false});
    const intro = page.getByRole('region',{name:'기기에 남는 음성과 글'});
    await expect(intro.getByRole('listitem')).toHaveCount(3);
    await expect(intro.getByText('기기 안 학습과 모델 변경분 전송은 준비 중입니다.')).toBeVisible();
    await captureReview(page,'onboarding');
    await intro.getByRole('button',{name:'첫 녹음 시작'}).click();
    await expect(intro).toHaveCount(0);
    await expect.poll(async () => (await readState(page,'microphone')).state).toBe('starting');
    await expect.poll(() => readState(page,'infoSeen')).toBe(true);
    await finish(page,errors,'onboarding opens the native microphone and keeps privacy information accessible');
  }
  {
    const { page, errors } = await fixture({infoSeen:false,openRecording:false,phone:true});
    const intro = page.getByRole('region',{name:'기기에 남는 음성과 글'});
    await expect(intro.getByRole('button',{name:'첫 녹음 시작'})).toHaveCount(0);
    await page.evaluate(() => {window.__REVIEW_TEST__.nextImport = '첫 음성.wav';});
    await intro.getByRole('button',{name:'첫 음성 파일 가져오기'}).tap();
    await expect(page.getByRole('button',{name:'첫 음성.wav',exact:true})).toHaveAttribute('aria-pressed','true');
    await expect(intro).toHaveCount(0);
    await expect.poll(() => readState(page,'infoSeen')).toBe(true);
    await finish(page,errors,'mobile onboarding imports from the file picker without offering an unavailable microphone');
  }
  {
    const { page, errors } = await fixture();
    await startBinary(page);
    await expect(page.locator('textarea')).toHaveCount(0);
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await expect(yes).toBeDisabled(); await page.keyboard.press('1');
    expect(await page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(0);
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.keyboard.down('1'); await page.keyboard.down('1'); await page.keyboard.up('1');
    await expect(page.getByText('두 번째 자동 전사문',{ exact:true })).toBeVisible();
    await expect(yes).toBeDisabled(); expect(await page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(1);
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.getByRole('button',{ name:'No, 다름',exact:true }).click();
    await expect(page.getByText('세 번째 자동 전사문',{ exact:true })).toBeVisible();
    await page.keyboard.press('Escape');
    const saved = await page.evaluate(() => window.__REVIEW_TEST__.segments);
    expect(saved[0].verified.source).toBe('accepted'); expect(saved[1].verdict).toBe('no'); expect(saved[1].verified).toBeNull(); expect(saved[2].verdict).toBeNull();
    await page.getByRole('button',{ name:'오류와 수정본 교정 목록',exact:true }).click();
    await expect(page.getByText('두 번째 자동 전사문',{ exact:true })).toBeVisible();
    await page.getByLabel('수정 초안').fill('사람이 들은 그대로의 수정본');
    await page.getByRole('button',{ name:'초안 저장',exact:true }).click();
    const confirm = page.getByRole('button',{ name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true });
    await expect(confirm).toBeDisabled();
    expect((await page.evaluate(() => window.__REVIEW_TEST__.segments))[1].verified).toBeNull();
    await page.getByRole('button',{ name:'구간 재생 (Space)',exact:true }).click(); await expect(confirm).toBeEnabled();
    await page.keyboard.press('Control+Enter'); await expect(page.getByText('이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다.')).toBeVisible();
    expect((await page.evaluate(() => window.__REVIEW_TEST__.segments))[1].verified.source).toBe('corrected');
    await finish(page,errors,'explicit Yes/No, held-key guard, separate draft and exact-revision correction confirmation');
  }
  {
    const { page, errors } = await fixture();
    await startBinary(page);
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.failNextSave = true; }); await yes.click();
    await expect(page.getByRole('alert')).toHaveText('시험 저장 실패');
    await expect(page.getByText('첫 번째 자동 전사문',{exact:true})).toBeVisible();
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.evaluate(() => { window.__REVIEW_TEST__.delayNextSave = 250; }); await yes.click();
    await page.keyboard.press('Escape');
    await expect.poll(() => page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(1);
    await expect(page.getByRole('heading',{name:'녹음 보관함'})).toBeVisible();
    await startBinary(page);
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(yes).toBeDisabled();
    await finish(page,errors,'failed write stays on frame; late success persists without reopening the exited pass');
  }
  {
    const { page, errors } = await fixture({ invalidFirst:true });
    await startBinary(page);
    await page.keyboard.press('Space');
    await expect(page.getByRole('alert')).toHaveText('재생 구간이 비었습니다');
    await page.keyboard.press('n');
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.keyboard.press('1');
    const saved = await page.evaluate(() => window.__REVIEW_TEST__.segments);
    expect(saved[0].verdict).toBeNull(); expect(saved[0].verified).toBeNull(); expect(saved[1].verified.source).toBe('accepted');
    await page.keyboard.press('Escape');
    await page.getByRole('button',{ name:'텍스트 파일로 저장',exact:true }).click();
    expect(await page.evaluate(() => window.__REVIEW_TEST__.exports)).toBe(1);
    await finish(page,errors,'invalid first audio can be skipped by keyboard without a verdict and later audio remains reachable');
  }
  {
    const { page, errors } = await fixture();
    await startBinary(page);
    await page.keyboard.press('Space');
    await expect(page.getByRole('button',{name:'No, 다름',exact:true})).toBeEnabled();
    await page.getByRole('button',{ name:'No, 다름',exact:true }).click(); await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    const confirm=page.getByRole('button',{name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true});
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(confirm).toBeDisabled();
    await page.getByLabel('수정 초안').fill('한 번 듣고 고친 말 그대로');
    await expect(confirm).toBeEnabled();
    await confirm.click();
    await expect(page.getByText('이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다.')).toBeVisible();
    const saved=await page.evaluate(() => window.__REVIEW_TEST__.segments.find(s=>s.id==='s1'));
    expect(saved.text).toBe('한 번 듣고 고친 말 그대로'); expect(saved.verified.source).toBe('corrected'); expect(saved.verified.revision).toBe(saved.revision);
    await page.keyboard.press('Escape'); await expect(page.getByRole('heading',{name:'녹음 보관함'})).toBeVisible();
    await finish(page,errors,'one completed playback permits an atomic edited-text confirmation and completed pass exits with Escape');
  }
  {
    const { page,errors }=await fixture();
    await startBinary(page);
    await page.keyboard.press('Space'); await expect(page.getByRole('button',{name:'No, 다름',exact:true})).toBeEnabled();
    await page.getByRole('button',{ name:'No, 다름',exact:true }).click(); await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    const confirm=page.getByRole('button',{name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true});
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await page.keyboard.press('1'); await page.keyboard.press('Control+Enter');
    expect(await page.evaluate(()=>window.__REVIEW_TEST__.segments.find(s=>s.id==='s1').verified)).toBeNull();
    await page.getByLabel('수정 초안').fill('  첫 번째 자동 전사문  ');
    await expect(confirm).toBeDisabled();
    await page.getByRole('button',{name:'처음 판정 취소하고 다시 검수',exact:true}).click();
    await expect(page.getByRole('heading',{name:'자동 전사문 검증',exact:true})).toBeVisible();
    const reset=await page.evaluate(()=>window.__REVIEW_TEST__.segments.find(s=>s.id==='s1'));
    expect(reset.verdict).toBeNull(); expect(reset.verified).toBeNull(); expect(reset.needs_correction).toBe(false);
    const yes=page.getByRole('button',{name:'Yes, 그대로 맞음 (1)',exact:true});
    await expect(yes).toBeDisabled(); await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.keyboard.press('1');
    await expect.poll(()=>page.evaluate(()=>window.__REVIEW_TEST__.segments.find(s=>s.id==='s1').verified?.source)).toBe('accepted');
    await finish(page,errors,'unchanged or whitespace-only No cannot confirm; explicit reset requires a new binary playback and Yes');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,1.6);
    const context = page.locator('.transcript-context');
    await expect(context.getByText('첫 번째 최신전사문',{exact:true})).toBeVisible();
    await expect(context.getByText('0:00.000 ~ 0:01.500',{exact:true})).toBeVisible();
    await expect(page.locator('.scan-timeline .active-token')).toHaveText('자동 전사문');
    await expect(page.locator('.scan-timeline .timeline-playhead')).toHaveAttribute('data-sample','25600');
    await expect(page.locator('.scan-timeline .timeline-line')).toHaveCount(2);
    await expect(page.locator('.scan-timeline .timeline-tick').nth(1)).toHaveText('0:03.200');
    const scanTop = (await page.locator('.scan-timeline').boundingBox()).y;
    await expect(page.getByRole('heading',{name:'교정 대기',exact:true})).toBeVisible();
    await captureReview(page,'listen');
    await seekAndPause(page,2.25);
    const flag = page.getByRole('button',{name:'오류 표시 (Space)',exact:true});
    await flag.focus();
    await page.keyboard.down('Space'); await page.keyboard.down('Space'); await page.keyboard.up('Space');
    const timeline = page.locator('.sample-timeline');
    await expect(timeline).toHaveAttribute('data-start','4000');
    await expect(timeline).toHaveAttribute('data-end','36000');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    const fallback = timeline.locator('.line-time');
    await expect(fallback).toHaveText('첫 번째 최신전사문');
    await expect(fallback).toHaveAttribute('data-start','0');
    const timed = timeline.locator('.token-time');
    await expect(timed).toHaveCount(2);
    await expect(timed.filter({hasText:'두 번째'})).toHaveAttribute('data-start','20000');
    await expect(timed.filter({hasText:'자동 전사문'})).toHaveAttribute('data-start','22400');
    for (const width of [1280, 560]) {
      await page.setViewportSize({width,height:900});
      expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(width);
      const coordinates = await timeline.evaluate(node => {
        const area = node.getBoundingClientRect();
        const box = selector => node.querySelector(selector).getBoundingClientRect();
        return [
          (box('.token-time[data-start="20000"]').left-area.left)/area.width,
          (box('.token-time[data-start="22400"]').left-area.left)/area.width,
          (box('.timeline-bar.candidate').right-area.left)/area.width,
          box('canvas').width/area.width,
        ];
      });
      [0.5,0.575,0.5,1].forEach((expected,i) => expect(coordinates[i]).toBeCloseTo(expected,3));
    }
    await page.setViewportSize({width:1280,height:900});
    await expect(page.locator('.timeline-context').getByText('두 번째 자동 전사문')).toBeVisible();

    const canvas = timeline.getByRole('img',{name:/원본 음성 스펙트로그램/});
    await expect.poll(() => canvas.evaluate(node => {
      const context = node.getContext('2d');
      const pixel = (time, lowToHighBin) => context.getImageData(Math.floor(node.width*time),node.height-1-lowToHighBin,1,1).data[0];
      return [pixel(0.2,128), pixel(0.55,128)>200, pixel(0.55,32)];
    })).toEqual([18,true,18]);
    await captureReview(page,'narrow');
    await page.getByRole('button',{name:'뒤 절반 듣기',exact:true}).click();
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)).toEqual({start:20000,end:36000});
    await seekAndPause(page,0.125);
    await expect(timeline.locator('.timeline-playhead')).toHaveAttribute('data-sample','22000');
    expect(await quickAudio(page).evaluate(node => {
      const wav = Uint8Array.from(atob(node.src.split(',')[1]), c => c.charCodeAt(0));
      const pcm = new DataView(wav.buffer);
      return [node.duration, pcm.getInt16(46,true), pcm.getInt16(44+4001*2,true)];
    })).toEqual([1,24000,0]);

    const chooseRight = page.getByRole('button',{name:'뒤 절반 선택 (→)',exact:true});
    await chooseRight.click();
    await expect(bar(page,'candidate',20000,36000)).toBeVisible();
    expect(await clipRanges(page,'locked')).toEqual([]);
    await chooseRight.focus();
    await page.keyboard.down('Enter'); await page.keyboard.down('Enter'); await page.keyboard.up('Enter');
    await expect(bar(page,'locked',20000,36000)).toBeVisible();
    await expect(bar(page,'candidate',20000,28000)).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(bar(page,'locked',20000,28000)).toBeVisible();
    await expect(bar(page,'candidate',20000,24000)).toBeVisible();
    await page.keyboard.press('ArrowUp'); await page.keyboard.press('ArrowUp');
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    expect(await clipRanges(page,'locked')).toEqual([[20000,36000],[20000,28000]]);
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Enter');
    await expect(bar(page,'candidate',4000,12000)).toBeVisible();
    expect(await readState(page,'markCalls')).toEqual([]);
    expect(await readState(page,'endedCalls')).toBe(0);

    await page.evaluate(() => { window.__REVIEW_TEST__.lostMarkReply = true; });
    await page.keyboard.down('Escape'); await page.keyboard.down('Escape'); await page.keyboard.up('Escape');
    await expect(page.getByRole('alert')).toHaveText('시험 응답 유실');
    expect(await readState(page,'markCalls')).toHaveLength(1);
    await expect(chooseRight).toBeDisabled();
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    await expect(bar(page,'candidate',4000,12000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)?.start).toBe(36000);
    const attempts = await readState(page,'markCalls');
    expect(attempts).toHaveLength(2); expect(attempts[0]).toEqual(attempts[1]);
    expect(attempts[0].ranges).toEqual([{start:20000,end:24000},{start:4000,end:12000}]);
    expect(await readState(page,'marks')).toHaveLength(2);
    const marked = await readState(page,'segments');
    expect(marked.map(s => [s.error_marked_text,s.verified,s.verdict])).toEqual([['첫 번째 최신전사문',null,null],['두 번째 자동 전사문',null,null],[undefined,null,null]]);

    const queue = page.getByRole('complementary',{name:'교정 대기'});
    await expect(queue.getByRole('listitem')).toHaveCount(2);
    await expect(page.locator('.scan-timeline .timeline-bar.retained')).toHaveCount(2);
    expect((await page.locator('.scan-timeline').boundingBox()).y).toBeCloseTo(scanTop, 0);
    const openQueue = queue.getByRole('button',{name:'교정 목록 열기'});
    await expect(openQueue).toBeEnabled();
    await captureReview(page,'pending');
    await openQueue.focus();
    await page.keyboard.press('Enter');
    await expect(page.getByRole('heading',{name:'수정본 그대로 확인',exact:true})).toBeVisible();
    const correction = page.locator('.correction-timeline');
    await expect(correction).toHaveAttribute('data-start','0');
    await expect(correction).toHaveAttribute('data-end','24000');
    await expect(correction.locator('.retained')).toHaveCount(2);
    await expect(page.getByRole('region',{name:'자동 전사 원문'})).toHaveText(/첫 번째 자동 전사문/);
    await expect(page.getByLabel('수정 초안')).toHaveValue('첫 번째 최신전사문');
    await captureReview(page,'correction-ready');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('heading',{name:'표시한 오류 구간',exact:true})).toBeVisible();
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    await expect(page.getByText('표시한 오류 구간 0:00.250 ~ 0:00.750, 0:01.250 ~ 0:01.500',{exact:true})).toBeVisible();
    const confirm = page.getByRole('button',{name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true});
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await page.locator('audio[aria-label="현재 조각의 음성"]').evaluate(node => node.dispatchEvent(new Event('ended')));
    await expect(confirm).toBeDisabled();
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await page.getByLabel('수정 초안').fill('  첫 번째 최신전사문  ');
    await expect(confirm).toBeDisabled();
    await page.keyboard.press('Control+Enter');
    expect(await readState(page,'reviewCalls')).toBe(0);
    await page.getByRole('button',{name:'초안 저장',exact:true}).click();
    await expect.poll(() => readState(page,'saves')).toBe(1);
    await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'오류와 수정본 교정 목록',exact:true}).click();
    await page.getByRole('button',{name:'구간 재생 (Space)',exact:true}).click();
    await expect(page.getByText('이 수정본의 음성을 끝까지 들었습니다.',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await page.keyboard.press('Control+Enter');
    expect(await readState(page,'reviewCalls')).toBe(1);
    await page.getByLabel('수정 초안').fill('첫 번째 최신 전사문');
    await captureReview(page,'correction-edit');
    await expect(confirm).toBeEnabled(); await confirm.click();
    await expect.poll(async () => (await readState(page,'segments'))[0].verified?.text).toBe('첫 번째 최신 전사문');
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(page.getByText('표시한 오류 구간 0:01.250 ~ 0:01.500',{exact:true})).toBeVisible();
    await expect(confirm).toBeDisabled();
    await expect(page.getByText('교정을 확정했습니다.',{exact:true})).toBeVisible();
    await expect(page.getByRole('complementary',{name:'교정할 조각'}).getByRole('listitem')).toHaveCount(1);
    await captureReview(page,'correction-next');
    await page.getByRole('button',{name:'이 조각의 오류 판정 해제',exact:true}).click();
    await expect(page.getByText(/이 목록에 남은 조각이 없습니다/)).toBeVisible();
    const dismissed = (await readState(page,'segments'))[1];
    expect(dismissed.error_marked_text).toBeUndefined();
    expect(dismissed.text).toBe('두 번째 자동 전사문');
    expect(dismissed.verified).toBeNull();
    expect(await readState(page,'marks')).toHaveLength(2);
    await finish(page,errors,'PCM time axis, keyboard selection of 0–1 and 4, retained ancestors, idempotent batch and separate full correction');
  }
  {
    const { page, errors } = await fixture({ quick:true, long:true });
    await startQuick(page);
    await expect.poll(() => quickAudio(page).evaluate(node => node.readyState)).toBeGreaterThan(0);
    await quickAudio(page).evaluate(node => { node.currentTime = 29.85; });
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)).toEqual({start:480000,end:512000});
    await seekAndPause(page,0.2);
    await page.keyboard.press('Space');
    await expect(page.locator('.sample-timeline')).toHaveAttribute('data-start','457600');
    await expect(page.locator('.timeline-line .line-time')).toHaveText('세 번째 자동 전사문');
    await page.evaluate(() => { window.__REVIEW_TEST__.holdNextRange = true; });
    await page.getByRole('button',{name:'앞 절반 듣기',exact:true}).click();
    await expect.poll(() => page.evaluate(() => typeof window.__REVIEW_TEST__.releaseRange)).toBe('function');
    await page.keyboard.press('Backspace');
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)?.start).toBe(483200);
    await seekAndPause(page,0.25);
    await page.evaluate(() => window.__REVIEW_TEST__.releaseRange());
    await expect.poll(() => readState(page,'delayedRangeReturned')).toBe(true);
    await quickAudio(page).evaluate(node => node.dispatchEvent(new Event('ended')));
    await page.keyboard.press('Space');
    await expect(bar(page,'candidate',483200,487200)).toBeVisible();
    await expect(page.getByText(/현재 범위가 마지막 후보입니다/)).toBeVisible();
    expect(await readState(page,'markCalls')).toEqual([]);
    await page.keyboard.press('Enter');
    await expect(bar(page,'locked',483200,487200)).toBeVisible();
    expect(await readState(page,'markCalls')).toEqual([]);
    await page.keyboard.press('Escape');
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)?.start).toBe(487200);
    expect((await readState(page,'markCalls'))[0].ranges).toEqual([{start:483200,end:487200}]);
    await seekAndPause(page,0.2);
    await page.keyboard.press('n');
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)).toEqual({start:505600,end:512000});
    await expect(page.getByText('녹음 끝',{exact:true})).toBeVisible();
    expect(await readState(page,'endedCalls')).toBe(0);
    expect(await readState(page,'markCalls')).toHaveLength(1);
    expect((await readState(page,'segments')).every(s => !s.verified && !s.verdict)).toBe(true);
    await finish(page,errors,'chunk transition, stale preview cancellation and explicit selection at minimum size');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,2.25);
    await page.keyboard.press('Space');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('Enter');
    await expect(bar(page,'locked',4000,20000)).toBeVisible();
    await page.keyboard.press('Backspace');
    await expect(bar(page,'locked',4000,20000)).toHaveCount(0);
    await page.keyboard.press('Backspace');
    expect(await clipRanges(page,'candidate')).toEqual([]);
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    expect(await readState(page,'markCalls')).toEqual([]);
    await page.keyboard.press('Escape');
    await expect.poll(() => readState(page,'markCalls')).toHaveLength(1);
    expect((await readState(page,'markCalls'))[0].ranges).toEqual([{start:20000,end:24000}]);
    await finish(page,errors,'Backspace at root discards the active candidate and Escape keeps the retained branch');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,2.25);
    await page.keyboard.press('Space');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(page.locator('.candidate-timeline')).toHaveCount(0);
    expect(await readState(page,'markCalls')).toEqual([]);

    // Restart at sample zero so the same original candidate is available again.
    await page.keyboard.press('Escape');
    await startQuick(page);
    await seekAndPause(page,2.25);
    await page.keyboard.press('Space');
    await page.keyboard.press('ArrowRight');
    await page.keyboard.press('Enter');
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowUp');
    await page.keyboard.press('ArrowUp');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(() => readState(page,'markCalls')).toHaveLength(1);
    expect((await readState(page,'markCalls'))[0].ranges).toEqual([{start:20000,end:24000}]);
    await finish(page,errors,'Escape saves only explicitly selected audio when the root shows its default left half');
  }
  {
    const { page, errors } = await fixture({ quick:true, branch:true });
    await startQuick(page);
    await seekAndPause(page,2.25);
    for (const key of ['Space','ArrowRight','Enter','Enter','ArrowUp','ArrowUp','Enter']) {
      await page.keyboard.press(key);
    }
    await page.evaluate(() => { window.__REVIEW_TEST__.failNextMark = true; });
    await page.keyboard.press('Escape');
    await expect(page.getByRole('alert')).toHaveText('시험 오류 구간 저장 실패');
    expect(await clipRanges(page,'retained')).toEqual([[20000,24000]]);
    await page.keyboard.press('Backspace');
    await expect(bar(page,'candidate',4000,20000)).toBeVisible();
    await page.keyboard.press('Enter');
    await page.keyboard.press('ArrowRight');
    await expect(bar(page,'candidate',12000,20000)).toBeVisible();
    await page.keyboard.press('Escape');
    await expect.poll(() => readState(page,'markCalls')).toHaveLength(2);
    const [failed, edited] = await readState(page,'markCalls');
    expect(edited.id).not.toEqual(failed.id);
    expect(edited.ranges).toEqual([{start:20000,end:24000},{start:12000,end:20000}]);
    expect(await readState(page,'marks')).toHaveLength(2);
    await finish(page,errors,'Backspace after a failed write keeps retained branches and allows a different request');
  }
  {
    const { page, errors } = await fixture({quick:true,branch:true,phone:true});
    const tap = name => page.getByRole('button',{name,exact:true}).tap();
    await tap('빠른 검수 시작');
    await seekAndPause(page,2.25);
    await expect(page.getByRole('button',{name:'오류 표시',exact:true})).toBeVisible();
    await expect(page.getByRole('button',{name:'오류 표시 (Space)',exact:true})).toHaveCount(0);
    await captureReview(page,'mobile-listen');
    await tap('오류 표시');
    await expect(page.getByRole('button',{name:'보관 후 올라가기'})).toBeDisabled();
    await tap('이어듣기');
    expect(await readState(page,'markCalls')).toEqual([]);
    await tap('검수 나가기');
    await tap('빠른 검수 시작');
    await seekAndPause(page,2.25);
    await tap('오류 표시');
    await tap('뒤 절반 듣기');
    await expect.poll(async () => (await readState(page,'rangeCalls')).at(-1)).toEqual({start:20000,end:36000});
    await tap('뒤 절반 선택');
    await tap('더 좁히기');
    await tap('더 좁히기');
    await tap('보관 후 올라가기');
    await tap('보관 후 올라가기');
    await tap('더 좁히기');
    await tap('뒤 절반 선택');
    await tap('선택 취소');
    await tap('더 좁히기');
    await expect(bar(page,'candidate',4000,12000)).toBeVisible();
    await expect(bar(page,'retained',20000,24000)).toBeVisible();
    await captureReview(page,'mobile-narrow');
    await page.evaluate(() => { window.__REVIEW_TEST__.lostMarkReply = true; });
    await tap('저장 후 이어듣기');
    await tap('저장 다시 시도');
    const attempts = await readState(page,'markCalls');
    expect(attempts).toHaveLength(2);
    expect(attempts[0]).toEqual(attempts[1]);
    expect(attempts[0].ranges).toEqual([{start:20000,end:24000},{start:4000,end:12000}]);
    const queue = page.getByRole('complementary',{name:'교정 대기'});
    await expect(queue.getByRole('listitem')).toHaveCount(2);
    await expect(queue.getByText('오류 2개')).toBeVisible();
    await expect(page.locator('.scan-timeline .timeline-bar.retained')).toHaveCount(2);
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
    await captureReview(page,'mobile-pending');
    await tap('교정 목록 열기');
    await expect(page.getByRole('heading',{name:'수정본 그대로 확인',exact:true})).toBeVisible();
    await finish(page,errors,'phone touch review compares both halves, retains branches, cancels a choice and retries a save');
  }
  {
    const page = await browser.newPage({viewport:{width:1280,height:900}});
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('response', response => { if (!response.ok()) errors.push(`${response.status()} ${response.url()}`); });
    await page.goto(`${url}/download/`);
    await expect(page.getByRole('heading',{level:1})).toHaveText(/녹음부터 교정까지/);
    expect(await page.getByRole('img').evaluate(img => img.naturalWidth)).toBeGreaterThan(0);
    await captureReview(page,'download-desktop');
    await page.getByRole('link',{name:'시험 빌드 보기',exact:true}).click();
    await expect(page).toHaveURL(/#download$/);
    await expect(page.getByRole('link',{name:'GitHub 빌드 목록'})).toHaveAttribute('href','https://github.com/guny524/seamone/actions/workflows/build.yml');
    await expect(page.getByText('기기 안 학습과 모델 변경분 전송은 준비 중입니다.')).toBeVisible();
    await page.setViewportSize({width:390,height:844});
    expect(await page.evaluate(() => document.documentElement.scrollWidth)).toBe(390);
    await finish(page,errors,'download page loads its preview, explains current and planned privacy and links to test builds');
  }
  console.log(`${completed} browser scenarios passed`);
} finally { await browser.close(); await new Promise(done => server.close(done)); }
