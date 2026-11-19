// Exercises the real compiled WASM UI with a test-only IPC service and real HTML audio ended events.
import { chromium, expect } from '@playwright/test';
import { createServer } from 'node:http';
import { createHash } from 'node:crypto';
import { readFile, stat } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(fileURLToPath(new URL('../dist/', import.meta.url)));
const config = JSON.parse(await readFile(new URL('../../src-tauri/tauri.conf.json',import.meta.url),'utf8'));
const index = await readFile(resolve(root,'index.html'),'utf8');
// Tauri adds hashes for bundled inline bootstrap scripts when it serves the app assets.
const hashes = [...index.matchAll(/<script\b[^>]*>([\s\S]*?)<\/script>/gi)]
  .filter(match => match[1].trim()).map(match => "'sha256-"+createHash('sha256').update(match[1]).digest('base64')+"'");
const productionCsp = config.app.security.csp.replace("script-src 'self'","script-src 'self' "+hashes.join(' '));
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, 'http://localhost').pathname;
  if (pathname === '/favicon.ico') { response.writeHead(204).end(); return; }
  const path = resolve(root, '.' + pathname);
  if (path !== root && !path.startsWith(root + '/')) { response.writeHead(403).end(); return; }
  try {
    const file = (await stat(path)).isDirectory() ? resolve(path, 'index.html') : path;
    const types = { '.html': 'text/html', '.js': 'application/javascript', '.wasm': 'application/wasm', '.css': 'text/css' };
    response.setHeader('Content-Type', types[extname(file)] || 'application/octet-stream');
    if (extname(file) === '.html') response.setHeader('Content-Security-Policy',productionCsp);
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
async function fixture({ infoSeen = true, invalidFirst = false } = {}) {
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  page.on('console', message => { if (message.type()==='error') errors.push(message.text()); });
  await page.addInitScript(({ wav, infoSeen, invalidFirst }) => {
    const recording = { id: 'r', name: '시험 녹음.wav', samples: 19200, revision: 1, engine: 'test', model: 'fixture' };
    const segments = ['첫 번째 자동 전사문', '두 번째 자동 전사문', '세 번째 자동 전사문'].map((text, i) => ({
      id: `s${i + 1}`, recording_id: 'r', recording_revision: 1, revision: 1, range: { start: i * 6400, end: (i + 1) * 6400 },
      machine_text: text, text, verdict: null, needs_correction: false, verified: null,
    }));
    if (invalidFirst) segments[0].range.end = 0;
    const proofs = new Map(); const events = new Map();
    const state = { segments, saves: 0, delayNextSave: 0, failNextSave: false, infoSeen, exports: 0 };
    window.__REVIEW_TEST__ = state;
    const copy = value => JSON.parse(JSON.stringify(value));
    window.__TAURI__ = { core: { invoke: async (command, args) => {
      if (args instanceof Map) throw 'IPC arguments must be plain JSON objects';
      if (command === 'get_library') return { recordings: [recording], info_seen: state.infoSeen, pending_training: segments.filter(s => s.verified).length, retry_training:0, inference_available: false };
      if (command === 'get_segments') return { segments: copy(segments), next_offset: null };
      if (command === 'dismiss_info') { state.infoSeen = true; return; }
      if (command === 'export_text') { state.exports++; return segments.map(s => s.text).join('\n'); }
      if (command === 'save_text_file') { state.exports++; return true; }
      if (command === 'copy_saved_text') { state.exports++; return; }
      if (command === 'get_playback') {
        const s = segments.find(s => s.id === args.segmentId);
        if (s.revision !== args.revision) throw '다른 수정본이 저장됐습니다';
        if (s.range.start >= s.range.end) throw '재생 구간이 비었습니다';
        const token = crypto.randomUUID(); proofs.set(token, { id:s.id, revision:s.revision, complete:false });
        return { segment_id:s.id, revision:s.revision, wav_base64:wav, token };
      }
      if (command === 'playback_ended') { proofs.get(args.token).complete = true; return; }
      if (command === 'save_review') {
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
        if (req.action.kind === 'save_draft' && req.action.text === s.text) return copy(s);
        s.revision++; s.verified = null;
        if (req.action.kind === 'judge') {
          s.verdict = req.action.verdict; s.needs_correction = s.verdict === 'no';
          if (s.verdict === 'yes') s.verified = { revision:s.revision,text:s.text,source:'accepted' };
        } else if (req.action.kind === 'save_draft') { s.text = req.action.text; s.needs_correction = true; }
        else if (req.action.kind === 'correct_and_confirm') { s.text=req.action.text; s.verified={ revision:s.revision,text:s.text,source:'corrected' }; }
        else if (req.action.kind === 'confirm_correction') s.verified = { revision:s.revision,text:s.text,source:'corrected' };
        else if (req.action.kind === 'invalidate') { const unchanged=s.verdict==='no' && s.text.trim()===s.machine_text.trim(); s.verdict=null; if (unchanged) { s.text=s.machine_text; s.needs_correction=false; } else s.needs_correction ||= s.text !== s.machine_text; }
        state.saves++; events.set(req.event_id,{ canonical,saved:copy(s) }); return copy(s);
      }
      throw `Unexpected test command: ${command}`;
    } } };
  }, { wav: wav.toString('base64'), infoSeen, invalidFirst });
  await page.goto(url);
  try { await page.getByRole('button', { name: '시험 녹음.wav', exact: true }).click({timeout:10000}); }
  catch (error) { throw new Error([...errors,String(error)].join('\n')); }
  await expect(page.getByText('첫 번째 자동 전사문', { exact: true })).toBeVisible();
  return { page, errors };
}
async function finish(page, errors, name) {
  expect(errors, name).toEqual([]); await page.close(); completed++; console.log(`PASS ${name}`);
}
try {
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
    const { page, errors } = await fixture();
    await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
    await expect(page.locator('textarea')).toHaveCount(0);
    const yes = page.getByRole('button',{ name:'Yes, 그대로 맞음 (1)',exact:true });
    await expect(yes).toBeDisabled(); await page.keyboard.press('1');
    expect(await page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(0);
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled();
    await page.keyboard.down('1'); await page.keyboard.down('1'); await page.keyboard.up('1');
    await expect(page.getByText('두 번째 자동 전사문',{ exact:true })).toBeVisible();
    await expect(yes).toBeDisabled(); expect(await page.evaluate(() => window.__REVIEW_TEST__.saves)).toBe(1);
    await page.keyboard.press('Space'); await expect(yes).toBeEnabled(); await page.keyboard.press('2');
    await expect(page.getByText('세 번째 자동 전사문',{ exact:true })).toBeVisible();
    await page.keyboard.press('Escape');
    const saved = await page.evaluate(() => window.__REVIEW_TEST__.segments);
    expect(saved[0].verified.source).toBe('accepted'); expect(saved[1].verdict).toBe('no'); expect(saved[1].verified).toBeNull(); expect(saved[2].verdict).toBeNull();
    await page.getByRole('button',{ name:'No와 수정본 확인 목록',exact:true }).click();
    await expect(page.getByText('두 번째 자동 전사문',{ exact:true })).toBeVisible();
    await page.getByLabel('수정 초안').fill('사람이 들은 그대로의 수정본');
    await page.getByRole('button',{ name:'초안 저장',exact:true }).click();
    const confirm = page.getByRole('button',{ name:'이대로 교정 확정 (Ctrl/Command+Enter)',exact:true });
    await expect(confirm).toBeDisabled();
    expect((await page.evaluate(() => window.__REVIEW_TEST__.segments))[1].verified).toBeNull();
    await page.getByRole('button',{ name:'구간 재생 (Space)',exact:true }).click(); await expect(confirm).toBeEnabled();
    await page.keyboard.press('Control+Enter'); await expect(page.getByText('이 목록에 남은 조각이 없습니다. 나가서 저장한 글을 사용할 수 있습니다.')).toBeVisible();
    expect((await page.evaluate(() => window.__REVIEW_TEST__.segments))[1].verified.source).toBe('corrected');
    await finish(page,errors,'keyboard Yes/No, held-key guard, separate draft and exact-revision correction confirmation');
  }
  {
    const { page, errors } = await fixture();
    await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
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
    await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
    await expect(page.getByText('두 번째 자동 전사문',{exact:true})).toBeVisible();
    await expect(yes).toBeDisabled();
    await finish(page,errors,'failed write stays on frame; late success persists without reopening the exited pass');
  }
  {
    const { page, errors } = await fixture({ invalidFirst:true });
    await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
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
    await page.getByRole('button',{ name:'자동 전사문 Yes/No 검증',exact:true }).click();
    await page.keyboard.press('Space');
    await expect(page.getByRole('button',{name:'No, 다름 (2)',exact:true})).toBeEnabled();
    await page.keyboard.press('2'); await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'No와 수정본 확인 목록',exact:true}).click();
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
    await page.getByRole('button',{name:'자동 전사문 Yes/No 검증',exact:true}).click();
    await page.keyboard.press('Space'); await expect(page.getByRole('button',{name:'No, 다름 (2)',exact:true})).toBeEnabled();
    await page.keyboard.press('2'); await page.keyboard.press('Escape');
    await page.getByRole('button',{name:'No와 수정본 확인 목록',exact:true}).click();
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
  console.log(`${completed} browser scenarios passed`);
} finally { await browser.close(); await new Promise(done => server.close(done)); }
